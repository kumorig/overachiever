//! Cloud sync database operations

use deadpool_postgres::Pool;
use overachiever_core::{CloudSyncData, CloudSyncStatus};
use chrono::Utc;
use crate::db::DbError;

/// Get cloud sync status for a user
pub async fn get_cloud_sync_status(pool: &Pool, steam_id: &str) -> Result<CloudSyncStatus, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    let game_count: i64 = client.query_one(
        "SELECT COUNT(*) FROM user_games WHERE steam_id = $1",
        &[&steam_id_int]
    ).await?.get(0);
    
    let achievement_count: i64 = client.query_one(
        "SELECT COUNT(*) FROM user_achievements WHERE steam_id = $1",
        &[&steam_id_int]
    ).await?.get(0);
    
    let last_sync: Option<chrono::DateTime<Utc>> = client.query_opt(
        "SELECT MAX(run_at) FROM run_history WHERE steam_id = $1",
        &[&steam_id_int]
    ).await?.and_then(|row| row.get(0));
    
    Ok(CloudSyncStatus {
        has_data: game_count > 0,
        game_count: game_count as i32,
        achievement_count: achievement_count as i32,
        last_sync,
    })
}

/// Get all user data for cloud download
pub async fn get_cloud_sync_data(pool: &Pool, steam_id: &str) -> Result<CloudSyncData, DbError> {
    let games = crate::db::games::get_user_games(pool, steam_id).await?;
    let achievements = crate::db::history::get_all_user_achievements(pool, steam_id).await?;
    let run_history = crate::db::history::get_run_history(pool, steam_id).await?;
    let achievement_history = crate::db::history::get_achievement_history(pool, steam_id).await?;
    
    Ok(CloudSyncData {
        steam_id: steam_id.to_string(),
        games,
        achievements,
        run_history,
        achievement_history,
        exported_at: Utc::now(),
    })
}

/// Upload cloud sync data (overwrites all existing data for user)
pub async fn upload_cloud_sync_data(pool: &Pool, data: &CloudSyncData) -> Result<(), DbError> {
    let mut client = pool.get().await?;
    let steam_id_int: i64 = data.steam_id.parse().unwrap_or(0);
    
    // Use transaction for atomicity
    let transaction = client.transaction().await?;
    
    // Ensure user exists
    transaction.execute(
        "INSERT INTO users (steam_id, display_name) VALUES ($1, $2) ON CONFLICT (steam_id) DO NOTHING",
        &[&steam_id_int, &format!("User {}", &data.steam_id[..8.min(data.steam_id.len())])]
    ).await?;
    
    // Delete existing data for this user
    transaction.execute("DELETE FROM user_achievements WHERE steam_id = $1", &[&steam_id_int]).await?;
    transaction.execute("DELETE FROM user_games WHERE steam_id = $1", &[&steam_id_int]).await?;
    transaction.execute("DELETE FROM run_history WHERE steam_id = $1", &[&steam_id_int]).await?;
    transaction.execute("DELETE FROM achievement_history WHERE steam_id = $1", &[&steam_id_int]).await?;
    
    // Insert games
    for game in &data.games {
        transaction.execute(
            r#"
            INSERT INTO user_games (steam_id, appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at, achievements_total, achievements_unlocked, last_sync)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            &[
                &steam_id_int,
                &(game.appid as i64),
                &game.name,
                &(game.playtime_forever as i32),
                &game.rtime_last_played.map(|t| t as i32),
                &game.img_icon_url,
                &game.added_at,
                &game.achievements_total,
                &game.achievements_unlocked,
                &game.last_achievement_scrape,
            ]
        ).await?;
    }
    
    // Insert achievements (lightweight - only sync user progress, not schema)
    for ach in &data.achievements {
        transaction.execute(
            r#"
            INSERT INTO user_achievements (steam_id, appid, apiname, achieved, unlocktime)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (steam_id, appid, apiname) DO UPDATE SET
                achieved = EXCLUDED.achieved,
                unlocktime = EXCLUDED.unlocktime
            "#,
            &[
                &steam_id_int,
                &(ach.appid as i64),
                &ach.apiname,
                &ach.achieved,
                &ach.unlocktime,
            ]
        ).await?;
    }
    
    // Insert run history
    for rh in &data.run_history {
        transaction.execute(
            "INSERT INTO run_history (steam_id, run_at, total_games, unplayed_games, unplayed_games_total) VALUES ($1, $2, $3, $4, $5)",
            &[&steam_id_int, &rh.run_at, &rh.total_games, &rh.unplayed_games, &rh.unplayed_games_total]
        ).await?;
    }
    
    // Insert achievement history
    for ah in &data.achievement_history {
        transaction.execute(
            "INSERT INTO achievement_history (steam_id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent) VALUES ($1, $2, $3, $4, $5, $6)",
            &[&steam_id_int, &ah.recorded_at, &ah.total_achievements, &ah.unlocked_achievements, &ah.games_with_achievements, &(ah.avg_completion_percent as f64)]
        ).await?;
    }
    
    transaction.commit().await?;
    
    Ok(())
}

/// Delete all cloud data for a user
pub async fn delete_cloud_sync_data(pool: &Pool, steam_id: &str) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    // Delete in order to respect foreign keys
    client.execute("DELETE FROM user_achievements WHERE steam_id = $1", &[&steam_id_int]).await?;
    client.execute("DELETE FROM user_games WHERE steam_id = $1", &[&steam_id_int]).await?;
    client.execute("DELETE FROM run_history WHERE steam_id = $1", &[&steam_id_int]).await?;
    client.execute("DELETE FROM achievement_history WHERE steam_id = $1", &[&steam_id_int]).await?;
    client.execute("DELETE FROM achievement_ratings WHERE steam_id = $1", &[&steam_id_int]).await?;
    client.execute("DELETE FROM game_ratings WHERE steam_id = $1", &[&steam_id_int]).await?;
    
    Ok(())
}
