//! WebSocket message handlers

use axum::extract::ws::Message;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use overachiever_core::{ClientMessage, ServerMessage};
use crate::AppState;

pub async fn handle_socket(socket: axum::extract::ws::WebSocket, state: Arc<AppState>) {
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
        let response = handle_client_message(client_msg, &mut sender, &state, &mut authenticated_steam_id).await;
        
        let response_text = serde_json::to_string(&response).unwrap();
        if sender.send(Message::Text(response_text.into())).await.is_err() {
            break;
        }
    }
}

async fn handle_client_message(
    msg: ClientMessage,
    sender: &mut futures_util::stream::SplitSink<axum::extract::ws::WebSocket, Message>,
    state: &Arc<AppState>,
    authenticated_steam_id: &mut Option<String>,
) -> ServerMessage {
    match msg {
        ClientMessage::Authenticate { token } => {
            match crate::auth::verify_jwt(&token, &state.jwt_secret) {
                Ok(claims) => {
                    *authenticated_steam_id = Some(claims.steam_id.clone());
                    ServerMessage::Authenticated {
                        user: overachiever_core::UserProfile {
                            steam_id: claims.steam_id,
                            display_name: claims.display_name,
                            avatar_url: claims.avatar_url,
                            short_id: claims.short_id,
                        }
                    }
                }
                Err(e) => ServerMessage::AuthError { reason: e.to_string() }
            }
        }
        
        ClientMessage::Ping => ServerMessage::Pong,
        
        ClientMessage::FetchGames => {
            if let Some(ref steam_id) = authenticated_steam_id {
                tracing::debug!("Fetching games for steam_id: {}", steam_id);
                match crate::db::get_user_games(&state.db_pool, steam_id).await {
                    Ok(games) => {
                        tracing::info!("Returning {} games for steam_id: {}", games.len(), steam_id);
                        ServerMessage::Games { games }
                    },
                    Err(e) => {
                        tracing::error!("Database error fetching games for {}: {:?}", steam_id, e);
                        ServerMessage::Error { message: format!("Database error: {:?}", e) }
                    }
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
        
        ClientMessage::ViewGuestLibrary { short_id } => {
            tracing::info!("Guest viewing library for short_id: {}", short_id);
            match crate::db::get_user_by_short_id(&state.db_pool, &short_id).await {
                Ok(Some(user)) => {
                    match crate::db::get_user_games_by_short_id(&state.db_pool, &short_id).await {
                        Ok(Some(games)) => {
                            tracing::info!("Returning {} games for guest view of {}", games.len(), short_id);
                            ServerMessage::GuestLibrary { user, games }
                        }
                        Ok(None) => ServerMessage::GuestNotFound { short_id },
                        Err(e) => ServerMessage::Error { message: format!("Database error: {:?}", e) }
                    }
                }
                Ok(None) => ServerMessage::GuestNotFound { short_id },
                Err(e) => ServerMessage::Error { message: format!("Database error: {:?}", e) }
            }
        }
        
        ClientMessage::FetchGuestAchievements { short_id, appid } => {
            match crate::db::get_game_achievements_by_short_id(&state.db_pool, &short_id, appid).await {
                Ok(Some(achievements)) => ServerMessage::Achievements { appid, achievements },
                Ok(None) => ServerMessage::GuestNotFound { short_id },
                Err(e) => ServerMessage::Error { message: e.to_string() }
            }
        }
        
        ClientMessage::FetchGuestHistory { short_id } => {
            match crate::db::get_history_by_short_id(&state.db_pool, &short_id).await {
                Ok(Some((run_history, achievement_history, log_entries))) => {
                    ServerMessage::History { run_history, achievement_history, log_entries }
                }
                Ok(None) => ServerMessage::GuestNotFound { short_id },
                Err(e) => ServerMessage::Error { message: e.to_string() }
            }
        }
        
        ClientMessage::SyncFromSteam => {
            if let Some(ref steam_id) = authenticated_steam_id {
                super::sync::handle_steam_sync(sender, state, steam_id).await
            } else {
                ServerMessage::AuthError { reason: "Not authenticated".to_string() }
            }
        }
        
        ClientMessage::FullScan { force } => {
            if let Some(ref steam_id) = authenticated_steam_id {
                super::sync::handle_full_scan(sender, state, steam_id, force).await
            } else {
                ServerMessage::AuthError { reason: "Not authenticated".to_string() }
            }
        }
        
        ClientMessage::FetchHistory => {
            if let Some(ref steam_id) = authenticated_steam_id {
                let run_history = crate::db::get_run_history(&state.db_pool, steam_id).await.unwrap_or_default();
                let achievement_history = crate::db::get_achievement_history(&state.db_pool, steam_id).await.unwrap_or_default();
                let log_entries = crate::db::get_log_entries(&state.db_pool, steam_id, 50).await.unwrap_or_default();
                ServerMessage::History {
                    run_history,
                    achievement_history,
                    log_entries,
                }
            } else {
                ServerMessage::AuthError { reason: "Not authenticated".to_string() }
            }
        }
        
        ClientMessage::RefreshSingleGame { appid } => {
            if let Some(ref steam_id) = authenticated_steam_id {
                super::sync::handle_single_game_refresh(state, steam_id, appid).await
            } else {
                ServerMessage::AuthError { reason: "Not authenticated".to_string() }
            }
        }
        
        ClientMessage::SubmitAchievementTip { .. } => {
            // TODO: Implement tip submission
            ServerMessage::Error { message: "Tip submission not yet implemented".to_string() }
        }
        
        ClientMessage::SubmitAchievementRating { appid, apiname, rating } => {
            if let Some(ref steam_id) = authenticated_steam_id {
                tracing::info!(steam_id = %steam_id, appid = %appid, apiname = %apiname, rating = %rating, "Achievement rating submitted");
                // TODO: Store rating in database
                ServerMessage::AchievementRatingSubmitted { appid, apiname }
            } else {
                ServerMessage::AuthError { reason: "Not authenticated".to_string() }
            }
        }
        
        ClientMessage::SubmitAchievementComment { achievements, comment } => {
            if let Some(ref steam_id) = authenticated_steam_id {
                tracing::info!(steam_id = %steam_id, achievements = ?achievements, comment = %comment, "Achievement comment submitted");
                // TODO: Store comment in database
                ServerMessage::AchievementCommentSubmitted { count: achievements.len() }
            } else {
                ServerMessage::AuthError { reason: "Not authenticated".to_string() }
            }
        }
        
        ClientMessage::ReportTtb { appid, main_seconds, extra_seconds, completionist_seconds } => {
            if let Some(ref steam_id) = authenticated_steam_id {
                tracing::info!(steam_id = %steam_id, appid = %appid, "TTB report submitted");
                match crate::db::report_ttb(&state.db_pool, steam_id, appid, main_seconds, extra_seconds, completionist_seconds).await {
                    Ok(()) => {
                        // Fetch updated game data to return
                        match crate::db::get_user_games(&state.db_pool, steam_id).await {
                            Ok(games) => {
                                if let Some(game) = games.into_iter().find(|g| g.appid == appid) {
                                    ServerMessage::TtbReported { appid, game }
                                } else {
                                    ServerMessage::Error { message: "Game not found after TTB report".to_string() }
                                }
                            }
                            Err(e) => ServerMessage::Error { message: format!("Error fetching updated game: {}", e) }
                        }
                    }
                    Err(e) => ServerMessage::Error { message: format!("Error reporting TTB: {}", e) }
                }
            } else {
                ServerMessage::AuthError { reason: "Not authenticated".to_string() }
            }
        }
        
        ClientMessage::MarkGameFinishing { appid, apiname } => {
            if let Some(ref steam_id) = authenticated_steam_id {
                tracing::info!(steam_id = %steam_id, appid = %appid, apiname = %apiname, "Marking game-finishing achievement");
                match crate::db::mark_game_finishing(&state.db_pool, steam_id, appid, &apiname).await {
                    Ok(()) => {
                        // Fetch updated achievements for the game
                        match crate::db::get_game_achievements(&state.db_pool, steam_id, appid).await {
                            Ok(achievements) => ServerMessage::GameFinishingMarked { appid, achievements },
                            Err(e) => ServerMessage::Error { message: format!("Error fetching achievements: {}", e) }
                        }
                    }
                    Err(e) => ServerMessage::Error { message: format!("Error marking game-finishing: {}", e) }
                }
            } else {
                ServerMessage::AuthError { reason: "Not authenticated".to_string() }
            }
        }
    }
}
