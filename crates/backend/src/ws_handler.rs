//! WebSocket handler for real-time sync

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use overachiever_core::{ClientMessage, ServerMessage};
use crate::AppState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    
    // Track authenticated user
    let mut authenticated_steam_id: Option<String> = None;
    
    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(data)) => {
                let _ = sender.send(Message::Pong(data)).await;
                continue;
            }
            _ => continue,
        };
        
        // Parse client message
        let client_msg: ClientMessage = match serde_json::from_str(&msg) {
            Ok(m) => m,
            Err(e) => {
                let error = ServerMessage::Error { 
                    message: format!("Invalid message: {}", e) 
                };
                let _ = sender.send(Message::Text(serde_json::to_string(&error).unwrap().into())).await;
                continue;
            }
        };
        
        // Handle message
        let response = match client_msg {
            ClientMessage::Authenticate { token } => {
                match crate::auth::verify_jwt(&token, &state.jwt_secret) {
                    Ok(claims) => {
                        authenticated_steam_id = Some(claims.steam_id.clone());
                        ServerMessage::Authenticated {
                            user: overachiever_core::UserProfile {
                                steam_id: claims.steam_id,
                                display_name: claims.display_name,
                                avatar_url: claims.avatar_url,
                            }
                        }
                    }
                    Err(e) => ServerMessage::AuthError { reason: e.to_string() }
                }
            }
            
            ClientMessage::Ping => ServerMessage::Pong,
            
            ClientMessage::FetchGames => {
                if let Some(ref steam_id) = authenticated_steam_id {
                    match crate::db::get_user_games(&state.db_pool, steam_id).await {
                        Ok(games) => ServerMessage::Games { games },
                        Err(e) => ServerMessage::Error { message: e.to_string() }
                    }
                } else {
                    ServerMessage::AuthError { reason: "Not authenticated".to_string() }
                }
            }
            
            ClientMessage::FetchAchievements { appid } => {
                if let Some(ref steam_id) = authenticated_steam_id {
                    match crate::db::get_game_achievements(&state.db_pool, steam_id, appid).await {
                        Ok(achievements) => ServerMessage::Achievements { appid, achievements },
                        Err(e) => ServerMessage::Error { message: e.to_string() }
                    }
                } else {
                    ServerMessage::AuthError { reason: "Not authenticated".to_string() }
                }
            }
            
            ClientMessage::GetCommunityRatings { appid } => {
                match crate::db::get_community_ratings(&state.db_pool, appid).await {
                    Ok(ratings) => {
                        let rating_count = ratings.len() as i32;
                        let avg_rating = if rating_count > 0 {
                            ratings.iter().map(|r| r.rating as f32).sum::<f32>() / rating_count as f32
                        } else {
                            0.0
                        };
                        ServerMessage::CommunityRatings { appid, avg_rating, rating_count, ratings }
                    }
                    Err(e) => ServerMessage::Error { message: e.to_string() }
                }
            }
            
            ClientMessage::SubmitRating { appid, rating, comment } => {
                if let Some(ref steam_id) = authenticated_steam_id {
                    let game_rating = overachiever_core::GameRating {
                        id: None,
                        steam_id: steam_id.clone(),
                        appid,
                        rating,
                        comment,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    };
                    match crate::db::upsert_rating(&state.db_pool, &game_rating).await {
                        Ok(_) => ServerMessage::RatingSubmitted { appid },
                        Err(e) => ServerMessage::Error { message: e.to_string() }
                    }
                } else {
                    ServerMessage::AuthError { reason: "Not authenticated".to_string() }
                }
            }
            
            ClientMessage::GetCommunityTips { appid, apiname } => {
                match crate::db::get_achievement_tips(&state.db_pool, appid, &apiname).await {
                    Ok(tips) => ServerMessage::CommunityTips { appid, apiname, tips },
                    Err(e) => ServerMessage::Error { message: e.to_string() }
                }
            }
            
            ClientMessage::SyncFromSteam => {
                // TODO: Implement server-side Steam sync
                ServerMessage::Error { message: "Steam sync not yet implemented".to_string() }
            }
            
            ClientMessage::SubmitAchievementTip { .. } => {
                // TODO: Implement tip submission
                ServerMessage::Error { message: "Tip submission not yet implemented".to_string() }
            }
        };
        
        let response_text = serde_json::to_string(&response).unwrap();
        if sender.send(Message::Text(response_text.into())).await.is_err() {
            break;
        }
    }
}
