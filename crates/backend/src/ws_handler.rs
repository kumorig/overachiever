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
            
            ClientMessage::SyncFromSteam => {
                if let Some(ref steam_id) = authenticated_steam_id {
                    if let Some(ref api_key) = state.steam_api_key {
                        tracing::info!("Starting Steam sync for user {}", steam_id);
                        let steam_id_u64: u64 = steam_id.parse().unwrap_or(0);
                        
                        // Step 1: Fetch all owned games
                        let games = match crate::steam_api::fetch_owned_games(api_key, steam_id_u64).await {
                            Ok(g) => g,
                            Err(e) => {
                                tracing::error!("Steam API error for user {}: {:?}", steam_id, e);
                                let _ = sender.send(Message::Text(serde_json::to_string(&ServerMessage::Error { 
                                    message: format!("Steam API error: {}", e) 
                                }).unwrap().into())).await;
                                continue;
                            }
                        };
                        
                        tracing::info!("Fetched {} games from Steam for user {}", games.len(), steam_id);
                        let game_count = games.len() as i32;
                        let unplayed_count = games.iter().filter(|g| g.playtime_forever == 0).count() as i32;
                        
                        match crate::db::upsert_games(&state.db_pool, steam_id, &games).await {
                            Ok(count) => tracing::info!("Saved {} games for user {}", count, steam_id),
                            Err(e) => {
                                let _ = sender.send(Message::Text(serde_json::to_string(&ServerMessage::Error { 
                                    message: format!("Failed to save games: {:?}", e) 
                                }).unwrap().into())).await;
                                continue;
                            }
                        }
                        
                        // Record run history
                        if let Err(e) = crate::db::insert_run_history(&state.db_pool, steam_id, game_count, unplayed_count).await {
                            tracing::error!("Failed to insert run_history: {:?}", e);
                        }
                        
                        // Step 2: Fetch recently played games
                        let recent_appids = crate::steam_api::fetch_recently_played(api_key, steam_id_u64)
                            .await
                            .unwrap_or_default();
                        
                        tracing::info!("Found {} recently played games for user {}", recent_appids.len(), steam_id);
                        
                        if recent_appids.is_empty() {
                            // No recently played games, return sync complete with just the games
                            match crate::db::get_user_games(&state.db_pool, steam_id).await {
                                Ok(user_games) => {
                                    let result = overachiever_core::SyncResult {
                                        games_updated: game_count,
                                        achievements_updated: 0,
                                        new_games: 0,
                                    };
                                    ServerMessage::SyncComplete { result, games: user_games }
                                }
                                Err(e) => ServerMessage::Error { message: format!("Failed to fetch games: {:?}", e) }
                            }
                        } else {
                            // Step 3: Scrape achievements for recently played games
                            let all_games = match crate::db::get_user_games(&state.db_pool, steam_id).await {
                                Ok(g) => g,
                                Err(e) => {
                                    let _ = sender.send(Message::Text(serde_json::to_string(&ServerMessage::Error { 
                                        message: format!("Failed to get games: {:?}", e) 
                                    }).unwrap().into())).await;
                                    continue;
                                }
                            };
                            
                            let games_to_scan: Vec<_> = all_games.iter()
                                .filter(|g| recent_appids.contains(&g.appid))
                                .collect();
                            
                            let total = games_to_scan.len();
                            tracing::info!("Scanning {} recently played games for achievements", total);
                            
                            // Send progress start
                            let _ = sender.send(Message::Text(serde_json::to_string(&ServerMessage::SyncProgress { 
                                state: overachiever_core::SyncState::Starting 
                            }).unwrap().into())).await;
                            
                            let mut total_achievements = 0i32;
                            let mut total_unlocked = 0i32;
                            let mut games_with_ach = 0i32;
                            let mut completion_sum = 0f32;
                            
                            for (i, game) in games_to_scan.iter().enumerate() {
                                // Send progress update
                                let _ = sender.send(Message::Text(serde_json::to_string(&ServerMessage::SyncProgress { 
                                    state: overachiever_core::SyncState::ScrapingAchievements {
                                        current: i as i32 + 1,
                                        total: total as i32,
                                        game_name: game.name.clone(),
                                    }
                                }).unwrap().into())).await;
                                
                                // Fetch achievements and schema
                                let achievements = crate::steam_api::fetch_achievements(api_key, steam_id_u64, game.appid).await.unwrap_or_default();
                                let schema = crate::steam_api::fetch_achievement_schema(api_key, game.appid).await.unwrap_or_default();
                                
                                // Store schema
                                for s in &schema {
                                    let _ = crate::db::upsert_achievement_schema(&state.db_pool, game.appid, s).await;
                                }
                                
                                // Store achievements and count
                                let ach_total = achievements.len() as i32;
                                let mut ach_unlocked = 0i32;
                                
                                for ach in &achievements {
                                    let _ = crate::db::upsert_user_achievement(&state.db_pool, steam_id, game.appid, ach).await;
                                    if ach.achieved == 1 {
                                        ach_unlocked += 1;
                                    }
                                }
                                
                                // Update game achievement counts
                                let _ = crate::db::update_game_achievements(&state.db_pool, steam_id, game.appid, ach_total, ach_unlocked).await;
                                
                                // Track totals
                                if ach_total > 0 {
                                    total_achievements += ach_total;
                                    total_unlocked += ach_unlocked;
                                    games_with_ach += 1;
                                    completion_sum += (ach_unlocked as f32 / ach_total as f32) * 100.0;
                                }
                                
                                // Small delay to avoid rate limiting
                                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                            }
                            
                            // Calculate unplayed games with achievements
                            let user_games = crate::db::get_user_games(&state.db_pool, steam_id).await.unwrap_or_default();
                            let unplayed_with_ach = user_games.iter()
                                .filter(|g| g.achievements_total.map(|t| t > 0).unwrap_or(false))
                                .filter(|g| g.playtime_forever == 0)
                                .count() as i32;
                            
                            // Update unplayed count in run_history and backfill historical data
                            let _ = crate::db::update_latest_run_history_unplayed(&state.db_pool, steam_id, unplayed_with_ach).await;
                            let _ = crate::db::backfill_run_history_unplayed(&state.db_pool, steam_id, unplayed_with_ach).await;
                            
                            // Record achievement history if we scanned any games with achievements
                            if games_with_ach > 0 {
                                let avg_completion = completion_sum / games_with_ach as f32;
                                let _ = crate::db::insert_achievement_history(&state.db_pool, steam_id, total_achievements, total_unlocked, games_with_ach, avg_completion).await;
                            }
                            
                            // Return with updated games
                            let result = overachiever_core::SyncResult {
                                games_updated: total as i32,
                                achievements_updated: total_achievements,
                                new_games: 0,
                            };
                            ServerMessage::SyncComplete { result, games: user_games }
                        }
                    } else {
                        ServerMessage::Error { message: "Steam API key not configured on server".to_string() }
                    }
                } else {
                    ServerMessage::AuthError { reason: "Not authenticated".to_string() }
                }
            }
            
            ClientMessage::FullScan { force } => {
                if let Some(ref steam_id) = authenticated_steam_id {
                    if let Some(ref api_key) = state.steam_api_key {
                        tracing::info!("Starting full achievement scan for user {} (force={})", steam_id, force);
                        let steam_id_u64: u64 = steam_id.parse().unwrap_or(0);
                        
                        // Get games that need scanning
                        let games = match crate::db::get_user_games(&state.db_pool, steam_id).await {
                            Ok(g) => g,
                            Err(e) => {
                                let _ = sender.send(Message::Text(serde_json::to_string(&ServerMessage::Error { 
                                    message: format!("Failed to get games: {:?}", e) 
                                }).unwrap().into())).await;
                                continue;
                            }
                        };
                        
                        let games_to_scan: Vec<_> = if force {
                            games.iter().collect()
                        } else {
                            games.iter().filter(|g| g.achievements_total.is_none()).collect()
                        };
                        
                        let total = games_to_scan.len();
                        tracing::info!("Scanning {} games for achievements", total);
                        
                        // Send progress start
                        let _ = sender.send(Message::Text(serde_json::to_string(&ServerMessage::SyncProgress { 
                            state: overachiever_core::SyncState::Starting 
                        }).unwrap().into())).await;
                        
                        let mut total_achievements = 0i32;
                        let mut total_unlocked = 0i32;
                        let mut games_with_ach = 0i32;
                        let mut completion_sum = 0f32;
                        
                        for (i, game) in games_to_scan.iter().enumerate() {
                            // Send progress update
                            let _ = sender.send(Message::Text(serde_json::to_string(&ServerMessage::SyncProgress { 
                                state: overachiever_core::SyncState::ScrapingAchievements {
                                    current: i as i32 + 1,
                                    total: total as i32,
                                    game_name: game.name.clone(),
                                }
                            }).unwrap().into())).await;
                            
                            // Fetch achievements and schema
                            let achievements = crate::steam_api::fetch_achievements(api_key, steam_id_u64, game.appid).await.unwrap_or_default();
                            let schema = crate::steam_api::fetch_achievement_schema(api_key, game.appid).await.unwrap_or_default();
                            
                            // Store schema
                            for s in &schema {
                                let _ = crate::db::upsert_achievement_schema(&state.db_pool, game.appid, s).await;
                            }
                            
                            // Store achievements and count
                            let ach_total = achievements.len() as i32;
                            let mut ach_unlocked = 0i32;
                            
                            for ach in &achievements {
                                let _ = crate::db::upsert_user_achievement(&state.db_pool, steam_id, game.appid, ach).await;
                                if ach.achieved == 1 {
                                    ach_unlocked += 1;
                                }
                            }
                            
                            // Update game achievement counts
                            let _ = crate::db::update_game_achievements(&state.db_pool, steam_id, game.appid, ach_total, ach_unlocked).await;
                            
                            // Track totals
                            if ach_total > 0 {
                                total_achievements += ach_total;
                                total_unlocked += ach_unlocked;
                                games_with_ach += 1;
                                completion_sum += (ach_unlocked as f32 / ach_total as f32) * 100.0;
                            }
                            
                            // Small delay to avoid rate limiting
                            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                        }
                        
                        // Calculate unplayed games with achievements
                        let user_games = crate::db::get_user_games(&state.db_pool, steam_id).await.unwrap_or_default();
                        let unplayed_with_ach = user_games.iter()
                            .filter(|g| g.achievements_total.map(|t| t > 0).unwrap_or(false))
                            .filter(|g| g.playtime_forever == 0)
                            .count() as i32;
                        
                        // Update unplayed count in run_history and backfill historical data
                        let _ = crate::db::update_latest_run_history_unplayed(&state.db_pool, steam_id, unplayed_with_ach).await;
                        let _ = crate::db::backfill_run_history_unplayed(&state.db_pool, steam_id, unplayed_with_ach).await;
                        
                        // Record achievement history
                        let avg_completion = if games_with_ach > 0 { completion_sum / games_with_ach as f32 } else { 0.0 };
                        let _ = crate::db::insert_achievement_history(&state.db_pool, steam_id, total_achievements, total_unlocked, games_with_ach, avg_completion).await;
                        
                        // Return with updated games
                        let result = overachiever_core::SyncResult {
                            games_updated: total as i32,
                            achievements_updated: total_achievements,
                            new_games: 0,
                        };
                        ServerMessage::SyncComplete { result, games: user_games }
                    } else {
                        ServerMessage::Error { message: "Steam API key not configured on server".to_string() }
                    }
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
        };
        
        let response_text = serde_json::to_string(&response).unwrap();
        if sender.send(Message::Text(response_text.into())).await.is_err() {
            break;
        }
    }
}
