//! WebSocket sync operations

use axum::extract::ws::Message;
use futures_util::SinkExt;
use std::sync::Arc;
use overachiever_core::ServerMessage;
use crate::AppState;

pub async fn handle_steam_sync(
    sender: &mut futures_util::stream::SplitSink<axum::extract::ws::WebSocket, Message>,
    state: &Arc<AppState>,
    steam_id: &str,
) -> ServerMessage {
    if let Some(ref api_key) = state.steam_api_key {
        tracing::info!("Starting Steam sync for user {}", steam_id);
        let steam_id_u64: u64 = steam_id.parse().unwrap_or(0);
        
        // Step 1: Fetch all owned games
        let games = match crate::steam_api::fetch_owned_games(api_key, steam_id_u64).await {
            Ok(g) => g,
            Err(e) => {
                tracing::error!("Steam API error for user {}: {:?}", steam_id, e);
                return ServerMessage::Error { 
                    message: format!("Steam API error: {}", e) 
                };
            }
        };
        
        tracing::info!("Fetched {} games from Steam for user {}", games.len(), steam_id);
        let game_count = games.len() as i32;
        let unplayed_count = games.iter().filter(|g| g.playtime_forever == 0).count() as i32;
        
        if let Err(_) = crate::db::upsert_games(&state.db_pool, steam_id, &games).await {
            return ServerMessage::Error { 
                message: "Failed to save games".to_string() 
            };
        }
        
        // Record run history
        let _ = crate::db::insert_run_history(&state.db_pool, steam_id, game_count, unplayed_count).await;
        
        // Step 2: Fetch recently played games
        let recent_games = crate::steam_api::fetch_recently_played(api_key, steam_id_u64)
            .await
            .unwrap_or_default();
        
        tracing::info!("Found {} recently played games for user {}", recent_games.len(), steam_id);
        
        if recent_games.is_empty() {
            // No recently played games, return sync complete with just the games
            match crate::db::get_user_games(&state.db_pool, steam_id).await {
                Ok(user_games) => {
                    let result = overachiever_core::SyncResult {
                        games_updated: game_count,
                        achievements_updated: 0,
                        new_games: 0,
                    };
                    return ServerMessage::SyncComplete { result, games: user_games };
                }
                Err(_) => return ServerMessage::Error { message: "Failed to fetch games".to_string() }
            }
        }
        
        // Upsert recently played games
        let _ = crate::db::upsert_games(&state.db_pool, steam_id, &recent_games).await;
        
        // Recalculate total games after adding recently played
        if let Ok(all_games_after) = crate::db::get_user_games(&state.db_pool, steam_id).await {
            let new_total = all_games_after.len() as i32;
            if new_total > game_count {
                let _ = crate::db::update_run_history_total(&state.db_pool, steam_id, new_total).await;
            }
        }
        
        // Get appids for filtering
        let recent_appids: Vec<u64> = recent_games.iter().map(|g| g.appid).collect();
        
        // Step 3: Scrape achievements for recently played games
        scan_achievements(sender, state, steam_id, &recent_appids, false).await
    } else {
        ServerMessage::Error { message: "Steam API key not configured on server".to_string() }
    }
}

pub async fn handle_full_scan(
    sender: &mut futures_util::stream::SplitSink<axum::extract::ws::WebSocket, Message>,
    state: &Arc<AppState>,
    steam_id: &str,
    force: bool,
) -> ServerMessage {
    if let Some(ref api_key) = state.steam_api_key {
        tracing::info!("Starting full achievement scan for user {} (force={})", steam_id, force);
        let steam_id_u64: u64 = steam_id.parse().unwrap_or(0);

        // Step 1: Fetch all owned games from Steam (like SyncFromSteam)
        let owned_games = match crate::steam_api::fetch_owned_games(api_key, steam_id_u64).await {
            Ok(g) => g,
            Err(e) => {
                tracing::error!("Steam API error for user {}: {:?}", steam_id, e);
                return ServerMessage::Error {
                    message: format!("Steam API error: {}", e)
                };
            }
        };

        tracing::info!("Fetched {} owned games from Steam for user {}", owned_games.len(), steam_id);
        let game_count = owned_games.len() as i32;
        let unplayed_count = owned_games.iter().filter(|g| g.playtime_forever == 0).count() as i32;

        if let Err(_) = crate::db::upsert_games(&state.db_pool, steam_id, &owned_games).await {
            return ServerMessage::Error {
                message: "Failed to save games".to_string()
            };
        }

        // Record run history
        let _ = crate::db::insert_run_history(&state.db_pool, steam_id, game_count, unplayed_count).await;

        // Step 2: Fetch recently played games (to capture F2P games not in GetOwnedGames)
        let recent_games = crate::steam_api::fetch_recently_played(api_key, steam_id_u64)
            .await
            .unwrap_or_default();

        if !recent_games.is_empty() {
            tracing::info!("Found {} recently played games for user {}", recent_games.len(), steam_id);
            let _ = crate::db::upsert_games(&state.db_pool, steam_id, &recent_games).await;

            // Update run_history if total increased
            if let Ok(all_games_after) = crate::db::get_user_games(&state.db_pool, steam_id).await {
                let new_total = all_games_after.len() as i32;
                if new_total > game_count {
                    let _ = crate::db::update_run_history_total(&state.db_pool, steam_id, new_total).await;
                }
            }
        }

        // Step 3: Get all appids for scanning
        let games = match crate::db::get_user_games(&state.db_pool, steam_id).await {
            Ok(g) => g,
            Err(_) => {
                return ServerMessage::Error {
                    message: "Failed to get games".to_string()
                };
            }
        };
        
        let appids: Vec<u64> = if force {
            games.iter().map(|g| g.appid).collect()
        } else {
            games.iter()
                .filter(|g| g.achievements_total.is_none())
                .map(|g| g.appid)
                .collect()
        };
        
        scan_achievements(sender, state, steam_id, &appids, false).await
    } else {
        ServerMessage::Error { message: "Steam API key not configured on server".to_string() }
    }
}

pub async fn handle_single_game_refresh(
    state: &Arc<AppState>,
    steam_id: &str,
    appid: u64,
) -> ServerMessage {
    if let Some(ref api_key) = state.steam_api_key {
        tracing::info!("Refreshing single game {} for user {}", appid, steam_id);
        let steam_id_u64: u64 = steam_id.parse().unwrap_or(0);
        
        // Fetch achievements and schema
        let achievements = crate::steam_api::fetch_achievements(api_key, steam_id_u64, appid).await.unwrap_or_default();
        let schema = crate::steam_api::fetch_achievement_schema(api_key, appid).await.unwrap_or_default();
        
        // Store schema
        for s in &schema {
            let _ = crate::db::upsert_achievement_schema(&state.db_pool, appid, s).await;
        }
        
        // Store achievements and count
        let ach_total = achievements.len() as i32;
        let mut ach_unlocked = 0i32;
        
        for ach in &achievements {
            let _ = crate::db::upsert_user_achievement(&state.db_pool, steam_id, appid, ach).await;
            if ach.achieved == 1 {
                ach_unlocked += 1;
            }
        }
        
        // Update game achievement counts
        let _ = crate::db::update_game_achievements(&state.db_pool, steam_id, appid, ach_total, ach_unlocked).await;
        
        // Get the updated game and achievements
        let user_games = crate::db::get_user_games(&state.db_pool, steam_id).await.unwrap_or_default();
        let game = user_games.into_iter().find(|g| g.appid == appid);
        let game_achievements = crate::db::get_game_achievements(&state.db_pool, steam_id, appid).await.unwrap_or_default();
        
        if let Some(game) = game {
            ServerMessage::SingleGameRefreshComplete { appid, game, achievements: game_achievements }
        } else {
            ServerMessage::Error { message: "Game not found after refresh".to_string() }
        }
    } else {
        ServerMessage::Error { message: "Steam API key not configured on server".to_string() }
    }
}

async fn scan_achievements(
    sender: &mut futures_util::stream::SplitSink<axum::extract::ws::WebSocket, Message>,
    state: &Arc<AppState>,
    steam_id: &str,
    appids: &[u64],
    _force: bool,
) -> ServerMessage {
    let api_key = match &state.steam_api_key {
        Some(key) => key,
        None => return ServerMessage::Error { message: "Steam API key not configured".to_string() }
    };
    
    let steam_id_u64: u64 = steam_id.parse().unwrap_or(0);
    
    // Get game names for progress reporting
    let all_games = match crate::db::get_user_games(&state.db_pool, steam_id).await {
        Ok(g) => g,
        Err(_) => return ServerMessage::Error { message: "Failed to get games".to_string() }
    };
    
    let games_to_scan: Vec<_> = all_games.iter()
        .filter(|g| appids.contains(&g.appid))
        .collect();
    
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
