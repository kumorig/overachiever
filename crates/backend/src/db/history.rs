//! History-related database operations

use deadpool_postgres::Pool;
use overachiever_core::{LogEntry, SyncAchievement};
use chrono::Utc;
use crate::db::DbError;

/// Get history data for a user by short_id (for guest viewing)
pub async fn get_history_by_short_id(
    pool: &Pool,
    short_id: &str,
) -> Result<Option<(Vec<overachiever_core::RunHistory>, Vec<overachiever_core::AchievementHistory>, Vec<overachiever_core::LogEntry>)>, DbError> {
    let client = pool.get().await?;
    
    // First get the steam_id for this short_id
    let user_row = client.query_opt(
        "SELECT steam_id FROM users WHERE short_id = $1",
        &[&short_id]
    ).await?;
    
    let steam_id_int: i64 = match user_row {
        Some(row) => row.get("steam_id"),
        None => return Ok(None),
    };
    
    let steam_id = steam_id_int.to_string();
    
    // Use existing functions to get history data
    let run_history = get_run_history(pool, &steam_id).await?;
    let achievement_history = get_achievement_history(pool, &steam_id).await?;
    let log_entries = get_log_entries(pool, &steam_id, 100).await?;
    
    Ok(Some((run_history, achievement_history, log_entries)))
}

/// Get run history for a user
pub async fn get_run_history(pool: &Pool, steam_id: &str) -> Result<Vec<overachiever_core::RunHistory>, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    let rows = client.query(
        r#"
        SELECT id::bigint as id, run_at, total_games, COALESCE(unplayed_games, 0) as unplayed_games, COALESCE(unplayed_games_total, 0) as unplayed_games_total
        FROM run_history
        WHERE steam_id = $1
        ORDER BY run_at
        "#,
        &[&steam_id_int]
    ).await?;
    
    let history = rows.into_iter().map(|row| {
        overachiever_core::RunHistory {
            id: row.get::<_, i64>("id"),
            run_at: row.get("run_at"),
            total_games: row.get("total_games"),
            unplayed_games: row.get("unplayed_games"),
            unplayed_games_total: row.get("unplayed_games_total"),
        }
    }).collect();
    
    Ok(history)
}

/// Get achievement history for a user  
pub async fn get_achievement_history(pool: &Pool, steam_id: &str) -> Result<Vec<overachiever_core::AchievementHistory>, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    let rows = client.query(
        r#"
        SELECT id::bigint as id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent
        FROM achievement_history
        WHERE steam_id = $1
        ORDER BY recorded_at
        "#,
        &[&steam_id_int]
    ).await?;
    
    let history = rows.into_iter().map(|row| {
        overachiever_core::AchievementHistory {
            id: row.get::<_, i64>("id"),
            recorded_at: row.get("recorded_at"),
            total_achievements: row.get("total_achievements"),
            unlocked_achievements: row.get("unlocked_achievements"),
            games_with_achievements: row.get("games_with_achievements"),
            avg_completion_percent: row.get::<_, f64>("avg_completion_percent") as f32,
        }
    }).collect();
    
    Ok(history)
}

/// Record a run history entry
pub async fn insert_run_history(pool: &Pool, steam_id: &str, total_games: i32, unplayed_games_total: i32) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    
    client.execute(
        r#"
        INSERT INTO run_history (steam_id, run_at, total_games, unplayed_games, unplayed_games_total)
        VALUES ($1, $2, $3, 0, $4)
        "#,
        &[&steam_id_int, &now, &total_games, &unplayed_games_total]
    ).await?;
    
    Ok(())
}

/// Update the unplayed_games count for the most recent run_history entry
pub async fn update_latest_run_history_unplayed(pool: &Pool, steam_id: &str, unplayed_games: i32) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    client.execute(
        r#"
        UPDATE run_history 
        SET unplayed_games = $1 
        WHERE steam_id = $2 AND id = (SELECT MAX(id) FROM run_history WHERE steam_id = $2)
        "#,
        &[&unplayed_games, &steam_id_int]
    ).await?;
    
    Ok(())
}

/// Update the total_games count for the most recent run_history entry
/// Used when recently played games add new games not in GetOwnedGames (e.g., some F2P games)
pub async fn update_run_history_total(pool: &Pool, steam_id: &str, total_games: i32) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    client.execute(
        r#"
        UPDATE run_history 
        SET total_games = $1 
        WHERE steam_id = $2 AND id = (SELECT MAX(id) FROM run_history WHERE steam_id = $2)
        "#,
        &[&total_games, &steam_id_int]
    ).await?;
    
    Ok(())
}

/// Backfill unplayed_games for run_history entries that still have 0
/// Only updates entries with unplayed_games = 0 (from before this feature was added)
pub async fn backfill_run_history_unplayed(pool: &Pool, steam_id: &str, current_unplayed: i32) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    client.execute(
        r#"
        UPDATE run_history 
        SET unplayed_games = $1 
        WHERE steam_id = $2 AND unplayed_games = 0
        "#,
        &[&current_unplayed, &steam_id_int]
    ).await?;
    
    Ok(())
}

/// Record achievement history snapshot
pub async fn insert_achievement_history(
    pool: &Pool,
    steam_id: &str,
    total_achievements: i32,
    unlocked_achievements: i32,
    games_with_achievements: i32,
    avg_completion_percent: f32,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    
    client.execute(
        r#"
        INSERT INTO achievement_history (steam_id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        &[&steam_id_int, &now, &total_achievements, &unlocked_achievements, &games_with_achievements, &(avg_completion_percent as f64)]
    ).await?;
    
    Ok(())
}

/// Get log entries (recently unlocked achievements) for a user
pub async fn get_log_entries(pool: &Pool, steam_id: &str, limit: i32) -> Result<Vec<LogEntry>, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    // Get recently unlocked achievements with game and schema info
    let rows = client.query(
        r#"
        SELECT ua.appid, g.name as game_name, ua.apiname, s.display_name as achievement_name, 
               ua.unlocktime, s.icon as achievement_icon, g.img_icon_url as game_icon_url
        FROM user_achievements ua
        JOIN user_games g ON ua.steam_id = g.steam_id AND ua.appid = g.appid
        LEFT JOIN achievement_schemas s ON ua.appid = s.appid AND ua.apiname = s.apiname
        WHERE ua.steam_id = $1 AND ua.achieved = true AND ua.unlocktime IS NOT NULL
        ORDER BY ua.unlocktime DESC
        LIMIT $2
        "#,
        &[&steam_id_int, &(limit as i64)]
    ).await?;
    
    let entries = rows.into_iter().map(|row| {
        LogEntry::Achievement {
            appid: row.get::<_, i64>("appid") as u64,
            game_name: row.get("game_name"),
            apiname: row.get("apiname"),
            achievement_name: row.get::<_, Option<String>>("achievement_name").unwrap_or_else(|| "Unknown".to_string()),
            timestamp: row.get("unlocktime"),
            achievement_icon: row.get::<_, Option<String>>("achievement_icon").unwrap_or_default(),
            game_icon_url: row.get("game_icon_url"),
        }
    }).collect();
    
    Ok(entries)
}

/// Get all achievements for a user (across all games) - lightweight for sync
pub async fn get_all_user_achievements(pool: &Pool, steam_id: &str) -> Result<Vec<SyncAchievement>, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    let rows = client.query(
        r#"
        SELECT appid, apiname, achieved, unlocktime
        FROM user_achievements
        WHERE steam_id = $1
        ORDER BY appid, apiname
        "#,
        &[&steam_id_int]
    ).await?;
    
    let achievements = rows.into_iter().map(|row| {
        let unlocktime: Option<chrono::DateTime<chrono::Utc>> = row.get("unlocktime");
        SyncAchievement {
            appid: row.get::<_, i64>("appid") as u64,
            apiname: row.get("apiname"),
            achieved: row.get("achieved"),
            unlocktime,
        }
    }).collect();
    
    Ok(achievements)
}
