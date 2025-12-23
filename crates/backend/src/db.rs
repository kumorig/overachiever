//! Database operations for the backend using tokio-postgres

use deadpool_postgres::{Pool, PoolError};
use overachiever_core::{Game, GameAchievement, GameRating, AchievementTip, LogEntry, CloudSyncData, CloudSyncStatus, SyncAchievement};
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub enum DbError {
    Pool(PoolError),
    Postgres(tokio_postgres::Error),
}

impl From<PoolError> for DbError {
    fn from(e: PoolError) -> Self {
        DbError::Pool(e)
    }
}

impl From<tokio_postgres::Error> for DbError {
    fn from(e: tokio_postgres::Error) -> Self {
        DbError::Postgres(e)
    }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Pool(e) => write!(f, "Pool error: {}", e),
            DbError::Postgres(e) => write!(f, "Postgres error: {}", e),
        }
    }
}

pub async fn get_user_games(pool: &Pool, steam_id: &str) -> Result<Vec<Game>, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    let rows = client.query(
        r#"
        SELECT appid, name, playtime_forever, rtime_last_played, img_icon_url,
               added_at, achievements_total, achievements_unlocked, last_sync
        FROM user_games
        WHERE steam_id = $1
        ORDER BY name
        "#,
        &[&steam_id_int]
    ).await?;
    
    let games = rows.into_iter().map(|row| {
        Game {
            appid: row.get::<_, i64>("appid") as u64,
            name: row.get("name"),
            playtime_forever: row.get::<_, i32>("playtime_forever") as u32,
            rtime_last_played: row.get::<_, Option<i32>>("rtime_last_played").map(|t| t as u32),
            img_icon_url: row.get("img_icon_url"),
            added_at: row.get::<_, Option<DateTime<Utc>>>("added_at").unwrap_or_else(Utc::now),
            achievements_total: row.get("achievements_total"),
            achievements_unlocked: row.get("achievements_unlocked"),
            last_achievement_scrape: row.get("last_sync"),
        }
    }).collect();
    
    Ok(games)
}

pub async fn get_game_achievements(
    pool: &Pool,
    steam_id: &str,
    appid: u64,
) -> Result<Vec<GameAchievement>, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    let rows = client.query(
        r#"
        SELECT ua.appid, ua.apiname, s.display_name as name, s.description,
               s.icon, s.icon_gray, ua.achieved, ua.unlocktime
        FROM user_achievements ua
        LEFT JOIN achievement_schemas s ON ua.appid = s.appid AND ua.apiname = s.apiname
        WHERE ua.steam_id = $1 AND ua.appid = $2
        ORDER BY s.display_name
        "#,
        &[&steam_id_int, &(appid as i64)]
    ).await?;
    
    let achievements = rows.into_iter().map(|row| {
        GameAchievement {
            appid: row.get::<_, i64>("appid") as u64,
            apiname: row.get("apiname"),
            name: row.get::<_, Option<String>>("name").unwrap_or_default(),
            description: row.get("description"),
            icon: row.get::<_, Option<String>>("icon").unwrap_or_default(),
            icon_gray: row.get::<_, Option<String>>("icon_gray").unwrap_or_default(),
            achieved: row.get::<_, Option<bool>>("achieved").unwrap_or(false),
            unlocktime: row.get("unlocktime"),
        }
    }).collect();
    
    Ok(achievements)
}

pub async fn get_community_ratings(
    pool: &Pool,
    appid: u64,
) -> Result<Vec<GameRating>, DbError> {
    let client = pool.get().await?;
    
    let rows = client.query(
        r#"
        SELECT id, steam_id, appid, rating, comment, created_at, updated_at
        FROM game_ratings
        WHERE appid = $1
        ORDER BY created_at DESC
        "#,
        &[&(appid as i64)]
    ).await?;
    
    let ratings = rows.into_iter().map(|row| {
        GameRating {
            id: Some(row.get::<_, i64>("id")),
            steam_id: row.get::<_, i64>("steam_id").to_string(),
            appid: row.get::<_, i64>("appid") as u64,
            rating: row.get::<_, i16>("rating") as u8,
            comment: row.get("comment"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }).collect();
    
    Ok(ratings)
}

pub async fn upsert_rating(
    pool: &Pool,
    rating: &GameRating,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = rating.steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    
    client.execute(
        r#"
        INSERT INTO game_ratings (steam_id, appid, rating, comment, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $5)
        ON CONFLICT (steam_id, appid) DO UPDATE SET
            rating = EXCLUDED.rating,
            comment = EXCLUDED.comment,
            updated_at = EXCLUDED.updated_at
        "#,
        &[
            &steam_id_int,
            &(rating.appid as i64),
            &(rating.rating as i16),
            &rating.comment,
            &now,
        ]
    ).await?;
    
    Ok(())
}

pub async fn get_achievement_tips(
    pool: &Pool,
    appid: u64,
    apiname: &str,
) -> Result<Vec<AchievementTip>, DbError> {
    let client = pool.get().await?;
    
    let rows = client.query(
        r#"
        SELECT id, steam_id, appid, apiname, difficulty, tip, created_at
        FROM achievement_tips
        WHERE appid = $1 AND apiname = $2
        ORDER BY created_at DESC
        "#,
        &[&(appid as i64), &apiname]
    ).await?;
    
    let tips = rows.into_iter().map(|row| {
        AchievementTip {
            id: Some(row.get::<_, i64>("id")),
            steam_id: row.get::<_, i64>("steam_id").to_string(),
            appid: row.get::<_, i64>("appid") as u64,
            apiname: row.get("apiname"),
            difficulty: row.get::<_, i16>("difficulty") as u8,
            tip: row.get("tip"),
            created_at: row.get("created_at"),
        }
    }).collect();
    
    Ok(tips)
}

pub async fn get_or_create_user(
    pool: &Pool,
    steam_id: &str,
    display_name: &str,
    avatar_url: Option<&str>,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    let avatar: Option<String> = avatar_url.map(|s| s.to_string());
    
    client.execute(
        r#"
        INSERT INTO users (steam_id, display_name, avatar_url, created_at, last_seen)
        VALUES ($1, $2, $3, $4, $4)
        ON CONFLICT (steam_id) DO UPDATE SET
            display_name = EXCLUDED.display_name,
            avatar_url = EXCLUDED.avatar_url,
            last_seen = EXCLUDED.last_seen
        "#,
        &[&steam_id_int, &display_name, &avatar, &now]
    ).await?;
    
    Ok(())
}

/// Insert or update games for a user
pub async fn upsert_games(
    pool: &Pool,
    steam_id: &str,
    games: &[overachiever_core::SteamGame],
) -> Result<usize, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    
    let mut count = 0;
    for game in games {
        client.execute(
            r#"
            INSERT INTO user_games (steam_id, appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (steam_id, appid) DO UPDATE SET
                name = EXCLUDED.name,
                playtime_forever = EXCLUDED.playtime_forever,
                rtime_last_played = EXCLUDED.rtime_last_played,
                img_icon_url = EXCLUDED.img_icon_url
            "#,
            &[
                &steam_id_int,
                &(game.appid as i64),
                &game.name,
                &(game.playtime_forever as i32),
                &game.rtime_last_played.map(|t| t as i32),
                &game.img_icon_url,
                &now,
            ]
        ).await?;
        count += 1;
    }
    
    Ok(count)
}

/// Update achievement counts for a game
pub async fn update_game_achievements(
    pool: &Pool,
    steam_id: &str,
    appid: u64,
    total: i32,
    unlocked: i32,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    
    client.execute(
        r#"
        UPDATE user_games
        SET achievements_total = $3, achievements_unlocked = $4, last_sync = $5
        WHERE steam_id = $1 AND appid = $2
        "#,
        &[
            &steam_id_int,
            &(appid as i64),
            &total,
            &unlocked,
            &now,
        ]
    ).await?;
    
    Ok(())
}

/// Store achievement schema
pub async fn upsert_achievement_schema(
    pool: &Pool,
    appid: u64,
    schema: &overachiever_core::AchievementSchema,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    
    client.execute(
        r#"
        INSERT INTO achievement_schemas (appid, apiname, display_name, description, icon, icon_gray)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (appid, apiname) DO UPDATE SET
            display_name = EXCLUDED.display_name,
            description = EXCLUDED.description,
            icon = EXCLUDED.icon,
            icon_gray = EXCLUDED.icon_gray
        "#,
        &[
            &(appid as i64),
            &schema.name,
            &schema.display_name,
            &schema.description,
            &schema.icon,
            &schema.icongray,
        ]
    ).await?;
    
    Ok(())
}

/// Store user achievement progress
pub async fn upsert_user_achievement(
    pool: &Pool,
    steam_id: &str,
    appid: u64,
    achievement: &overachiever_core::Achievement,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let achieved = achievement.achieved == 1;
    let unlocktime: Option<DateTime<Utc>> = if achievement.unlocktime > 0 {
        chrono::DateTime::from_timestamp(achievement.unlocktime as i64, 0)
    } else {
        None
    };
    
    client.execute(
        r#"
        INSERT INTO user_achievements (steam_id, appid, apiname, achieved, unlocktime)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (steam_id, appid, apiname) DO UPDATE SET
            achieved = EXCLUDED.achieved,
            unlocktime = COALESCE(EXCLUDED.unlocktime, user_achievements.unlocktime)
        "#,
        &[
            &steam_id_int,
            &(appid as i64),
            &achievement.apiname,
            &achieved,
            &unlocktime,
        ]
    ).await?;
    
    Ok(())
}

/// Get run history for a user
pub async fn get_run_history(pool: &Pool, steam_id: &str) -> Result<Vec<overachiever_core::RunHistory>, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    let rows = client.query(
        r#"
        SELECT id::bigint as id, run_at, total_games, COALESCE(unplayed_games, 0) as unplayed_games
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
pub async fn insert_run_history(pool: &Pool, steam_id: &str, total_games: i32, unplayed_games: i32) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    
    client.execute(
        r#"
        INSERT INTO run_history (steam_id, run_at, total_games, unplayed_games)
        VALUES ($1, $2, $3, $4)
        "#,
        &[&steam_id_int, &now, &total_games, &unplayed_games]
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

/// Backfill unplayed_games for all run_history entries
/// Uses the current unplayed count as a baseline since we don't have historical data
pub async fn backfill_run_history_unplayed(pool: &Pool, steam_id: &str, current_unplayed: i32) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    client.execute(
        r#"
        UPDATE run_history 
        SET unplayed_games = $1 
        WHERE steam_id = $2
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

/// Upsert an achievement rating for a user
pub async fn upsert_achievement_rating(
    pool: &Pool,
    steam_id: &str,
    appid: u64,
    apiname: &str,
    rating: u8,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    client.execute(
        r#"
        INSERT INTO achievement_ratings (steam_id, appid, apiname, rating)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (steam_id, appid, apiname)
        DO UPDATE SET rating = $4, updated_at = NOW()
        "#,
        &[&steam_id_int, &(appid as i64), &apiname, &(rating as i16)]
    ).await?;
    
    Ok(())
}

/// Get all achievement ratings for a user
pub async fn get_user_achievement_ratings(
    pool: &Pool,
    steam_id: &str,
) -> Result<Vec<(u64, String, u8)>, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    let rows = client.query(
        r#"
        SELECT appid, apiname, rating
        FROM achievement_ratings
        WHERE steam_id = $1
        "#,
        &[&steam_id_int]
    ).await?;
    
    let ratings = rows.into_iter().map(|row| {
        (
            row.get::<_, i64>("appid") as u64,
            row.get::<_, String>("apiname"),
            row.get::<_, i16>("rating") as u8,
        )
    }).collect();
    
    Ok(ratings)
}

// ============================================================================
// Cloud Sync Functions
// ============================================================================

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
    let games = get_user_games(pool, steam_id).await?;
    let achievements = get_all_user_achievements(pool, steam_id).await?;
    let run_history = get_run_history(pool, steam_id).await?;
    let achievement_history = get_achievement_history(pool, steam_id).await?;
    
    Ok(CloudSyncData {
        steam_id: steam_id.to_string(),
        games,
        achievements,
        run_history,
        achievement_history,
        exported_at: Utc::now(),
    })
}

/// Get all achievements for a user (across all games) - lightweight for sync
async fn get_all_user_achievements(pool: &Pool, steam_id: &str) -> Result<Vec<SyncAchievement>, DbError> {
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
        let unlocktime: Option<i32> = row.get("unlocktime");
        SyncAchievement {
            appid: row.get::<_, i64>("appid") as u64,
            apiname: row.get("apiname"),
            achieved: row.get("achieved"),
            unlocktime: unlocktime.and_then(|t| {
                if t > 0 {
                    chrono::DateTime::from_timestamp(t as i64, 0)
                } else {
                    None
                }
            }),
        }
    }).collect();
    
    Ok(achievements)
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
            "INSERT INTO run_history (steam_id, run_at, total_games, unplayed_games) VALUES ($1, $2, $3, $4)",
            &[&steam_id_int, &rh.run_at, &rh.total_games, &rh.unplayed_games]
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
