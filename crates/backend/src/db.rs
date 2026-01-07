//! Database operations for the backend using tokio-postgres

use deadpool_postgres::{Pool, PoolError};
use overachiever_core::{Game, GameAchievement, GameRating, AchievementTip, LogEntry, CloudSyncData, CloudSyncStatus, SyncAchievement};
use chrono::{DateTime, Utc};
use rand::Rng;

/// Characters used for generating short IDs (URL-safe, case-sensitive)
/// Similar to YouTube's video ID format
const SHORT_ID_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const SHORT_ID_LENGTH: usize = 8;

/// Generate a random short ID (YouTube-style)
pub fn generate_short_id() -> String {
    let mut rng = rand::thread_rng();
    (0..SHORT_ID_LENGTH)
        .map(|_| {
            let idx = rng.gen_range(0..SHORT_ID_CHARS.len());
            SHORT_ID_CHARS[idx] as char
        })
        .collect()
}

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

/// Get user info (steam_id, display_name, avatar_url) by short_id
pub async fn get_user_by_short_id(pool: &Pool, short_id: &str) -> Result<Option<overachiever_core::UserProfile>, DbError> {
    let client = pool.get().await?;
    
    let row = client.query_opt(
        r#"
        SELECT steam_id, display_name, avatar_url, short_id
        FROM users
        WHERE short_id = $1
        "#,
        &[&short_id]
    ).await?;
    
    Ok(row.map(|row| {
        overachiever_core::UserProfile {
            steam_id: row.get::<_, i64>("steam_id").to_string(),
            display_name: row.get("display_name"),
            avatar_url: row.get("avatar_url"),
            short_id: row.get("short_id"),
        }
    }))
}

/// Get games for a user by their short_id
pub async fn get_user_games_by_short_id(pool: &Pool, short_id: &str) -> Result<Option<Vec<Game>>, DbError> {
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
    
    Ok(Some(games))
}

/// Get achievements for a game by short_id (for guest viewing)
pub async fn get_game_achievements_by_short_id(
    pool: &Pool,
    short_id: &str,
    appid: u64,
) -> Result<Option<Vec<GameAchievement>>, DbError> {
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
            name: row.get::<_, Option<String>>("name").unwrap_or_else(|| row.get("apiname")),
            description: row.get("description"),
            icon: row.get::<_, Option<String>>("icon").unwrap_or_default(),
            icon_gray: row.get::<_, Option<String>>("icon_gray").unwrap_or_default(),
            achieved: row.get("achieved"),
            unlocktime: row.get("unlocktime"),
        }
    }).collect();
    
    Ok(Some(achievements))
}

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
) -> Result<String, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    let avatar: Option<String> = avatar_url.map(|s| s.to_string());
    
    // First, check if user exists and has a short_id
    let existing = client.query_opt(
        "SELECT short_id FROM users WHERE steam_id = $1",
        &[&steam_id_int]
    ).await?;
    
    if let Some(row) = existing {
        // User exists, update last_seen and return existing short_id
        client.execute(
            r#"
            UPDATE users SET
                display_name = $2,
                avatar_url = $3,
                last_seen = $4
            WHERE steam_id = $1
            "#,
            &[&steam_id_int, &display_name, &avatar, &now]
        ).await?;
        
        // Return existing short_id or generate one if missing
        if let Some(short_id) = row.get::<_, Option<String>>("short_id") {
            return Ok(short_id);
        }
        
        // Generate short_id for existing user (migration case)
        let short_id = generate_unique_short_id(&client).await?;
        client.execute(
            "UPDATE users SET short_id = $2 WHERE steam_id = $1",
            &[&steam_id_int, &short_id]
        ).await?;
        return Ok(short_id);
    }
    
    // New user - generate unique short_id
    let short_id = generate_unique_short_id(&client).await?;
    
    client.execute(
        r#"
        INSERT INTO users (steam_id, display_name, avatar_url, short_id, created_at, last_seen)
        VALUES ($1, $2, $3, $4, $5, $5)
        "#,
        &[&steam_id_int, &display_name, &avatar, &short_id, &now]
    ).await?;
    
    Ok(short_id)
}

/// Generate a unique short_id by checking for collisions
async fn generate_unique_short_id(client: &deadpool_postgres::Client) -> Result<String, DbError> {
    loop {
        let short_id = generate_short_id();
        let exists = client.query_opt(
            "SELECT 1 FROM users WHERE short_id = $1",
            &[&short_id]
        ).await?;
        
        if exists.is_none() {
            return Ok(short_id);
        }
        // Collision detected, try again (extremely rare with 62^8 possibilities)
    }
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

// ============================================================================
// Size on Disk Cache
// ============================================================================

/// App size data for caching install sizes
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSizeInfo {
    pub appid: u64,
    pub size_bytes: u64,
}

/// Upsert app size data (updates if exists, inserts if not)
pub async fn upsert_app_sizes(pool: &Pool, sizes: &[AppSizeInfo]) -> Result<usize, DbError> {
    if sizes.is_empty() {
        return Ok(0);
    }
    
    let client = pool.get().await?;
    let mut count = 0;
    
    for size in sizes {
        let result = client.execute(
            r#"
            INSERT INTO app_size_on_disk (appid, size_bytes, reported_count, first_reported_at, last_reported_at)
            VALUES ($1, $2, 1, NOW(), NOW())
            ON CONFLICT (appid) DO UPDATE SET
                size_bytes = CASE 
                    WHEN app_size_on_disk.size_bytes = EXCLUDED.size_bytes THEN app_size_on_disk.size_bytes
                    ELSE EXCLUDED.size_bytes
                END,
                reported_count = app_size_on_disk.reported_count + 1,
                last_reported_at = NOW()
            "#,
            &[&(size.appid as i64), &(size.size_bytes as i64)]
        ).await?;
        count += result as usize;
    }
    
    Ok(count)
}

/// Get sizes for a list of app IDs
pub async fn get_app_sizes(pool: &Pool, appids: &[u64]) -> Result<Vec<AppSizeInfo>, DbError> {
    if appids.is_empty() {
        return Ok(vec![]);
    }
    
    let client = pool.get().await?;
    
    // Convert to i64 for postgres
    let appids_i64: Vec<i64> = appids.iter().map(|&id| id as i64).collect();
    
    let rows = client.query(
        "SELECT appid, size_bytes FROM app_size_on_disk WHERE appid = ANY($1)",
        &[&appids_i64]
    ).await?;
    
    let sizes = rows.into_iter().map(|row| {
        AppSizeInfo {
            appid: row.get::<_, i64>("appid") as u64,
            size_bytes: row.get::<_, i64>("size_bytes") as u64,
        }
    }).collect();
    
    Ok(sizes)
}

/// Log an API request
pub async fn log_api_request(
    pool: &Pool,
    endpoint: &str,
    client_ip: Option<&str>,
    user_agent: Option<&str>,
    referer: Option<&str>,
    query_params: Option<&str>,
    app_ids: Option<&str>,
) -> Result<(), DbError> {
    let client = pool.get().await?;

    client.execute(
        r#"
        INSERT INTO api_request_log (endpoint, client_ip, user_agent, referer, query_params, app_ids, requested_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        "#,
        &[&endpoint, &client_ip, &user_agent, &referer, &query_params, &app_ids]
    ).await?;

    Ok(())
}

// ============================================================================
// Time To Beat (TTB) Data
// ============================================================================

/// Upsert TTB times for a game (from desktop scraper)
pub async fn upsert_ttb_times(
    pool: &Pool,
    appid: u64,
    game_name: &str,
    main: Option<f32>,
    main_extra: Option<f32>,
    completionist: Option<f32>,
) -> Result<(), DbError> {
    let client = pool.get().await?;

    client.execute(
        r#"
        INSERT INTO ttb_times (appid, game_name, main, main_extra, completionist, reported_count, first_reported_at, last_reported_at)
        VALUES ($1, $2, $3, $4, $5, 1, NOW(), NOW())
        ON CONFLICT (appid) DO UPDATE SET
            game_name = EXCLUDED.game_name,
            main = COALESCE(EXCLUDED.main, ttb_times.main),
            main_extra = COALESCE(EXCLUDED.main_extra, ttb_times.main_extra),
            completionist = COALESCE(EXCLUDED.completionist, ttb_times.completionist),
            reported_count = ttb_times.reported_count + 1,
            last_reported_at = NOW()
        "#,
        &[
            &(appid as i64),
            &game_name,
            &main,
            &main_extra,
            &completionist,
        ]
    ).await?;

    Ok(())
}

/// Get TTB times for a single game
pub async fn get_ttb_times(pool: &Pool, appid: u64) -> Result<Option<overachiever_core::TtbTimes>, DbError> {
    let client = pool.get().await?;

    let row = client.query_opt(
        "SELECT appid, main, main_extra, completionist, last_reported_at FROM ttb_times WHERE appid = $1",
        &[&(appid as i64)]
    ).await?;

    Ok(row.map(|r| overachiever_core::TtbTimes {
        appid: r.get::<_, i64>("appid") as u64,
        main: r.get("main"),
        main_extra: r.get("main_extra"),
        completionist: r.get("completionist"),
        updated_at: r.get("last_reported_at"),
    }))
}

/// Get TTB times for multiple games
pub async fn get_ttb_times_batch(pool: &Pool, appids: &[u64]) -> Result<Vec<overachiever_core::TtbTimes>, DbError> {
    if appids.is_empty() {
        return Ok(vec![]);
    }

    let client = pool.get().await?;
    let appids_i64: Vec<i64> = appids.iter().map(|&id| id as i64).collect();

    let rows = client.query(
        "SELECT appid, main, main_extra, completionist, last_reported_at FROM ttb_times WHERE appid = ANY($1)",
        &[&appids_i64]
    ).await?;

    let times = rows.into_iter().map(|r| overachiever_core::TtbTimes {
        appid: r.get::<_, i64>("appid") as u64,
        main: r.get("main"),
        main_extra: r.get("main_extra"),
        completionist: r.get("completionist"),
        updated_at: r.get("last_reported_at"),
    }).collect();

    Ok(times)
}

// ============================================================================
// TTB Blacklist (games excluded from TTB scanning)
// ============================================================================

/// Add a game to the TTB blacklist
pub async fn add_to_ttb_blacklist(
    pool: &Pool,
    appid: u64,
    game_name: &str,
    reason: Option<&str>,
    added_by_steam_id: &str,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = added_by_steam_id.parse().unwrap_or(0);

    client.execute(
        r#"
        INSERT INTO ttb_blacklist (appid, game_name, reason, added_by_steam_id, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        ON CONFLICT (appid) DO UPDATE SET
            game_name = EXCLUDED.game_name,
            reason = EXCLUDED.reason,
            added_by_steam_id = EXCLUDED.added_by_steam_id,
            created_at = NOW()
        "#,
        &[
            &(appid as i64),
            &game_name,
            &reason,
            &steam_id_int,
        ]
    ).await?;

    Ok(())
}

/// Remove a game from the TTB blacklist
pub async fn remove_from_ttb_blacklist(pool: &Pool, appid: u64) -> Result<bool, DbError> {
    let client = pool.get().await?;

    let rows_affected = client.execute(
        "DELETE FROM ttb_blacklist WHERE appid = $1",
        &[&(appid as i64)]
    ).await?;

    Ok(rows_affected > 0)
}

/// Get all games in the TTB blacklist (returns list of appids)
pub async fn get_ttb_blacklist(pool: &Pool) -> Result<Vec<u64>, DbError> {
    let client = pool.get().await?;

    let rows = client.query(
        "SELECT appid FROM ttb_blacklist ORDER BY created_at DESC",
        &[]
    ).await?;

    let appids = rows.into_iter()
        .map(|r| r.get::<_, i64>("appid") as u64)
        .collect();

    Ok(appids)
}

/// Check if a game is in the TTB blacklist
pub async fn is_in_ttb_blacklist(pool: &Pool, appid: u64) -> Result<bool, DbError> {
    let client = pool.get().await?;

    let row = client.query_opt(
        "SELECT 1 FROM ttb_blacklist WHERE appid = $1",
        &[&(appid as i64)]
    ).await?;

    Ok(row.is_some())
}

// ============================================================================
// Game Tags (from SteamSpy)
// ============================================================================

/// Get all unique tag names (for dropdown filter)
pub async fn get_all_tag_names(pool: &Pool) -> Result<Vec<String>, DbError> {
    let client = pool.get().await?;

    let rows = client.query(
        "SELECT DISTINCT tag_name FROM game_tags ORDER BY tag_name",
        &[]
    ).await?;

    let tags = rows.into_iter()
        .map(|r| r.get::<_, String>("tag_name"))
        .collect();

    Ok(tags)
}

/// Get tags for a list of games
pub async fn get_tags_for_games(pool: &Pool, appids: &[u64]) -> Result<Vec<overachiever_core::GameTag>, DbError> {
    if appids.is_empty() {
        return Ok(vec![]);
    }

    let client = pool.get().await?;
    let appids_i64: Vec<i64> = appids.iter().map(|&id| id as i64).collect();

    let rows = client.query(
        "SELECT appid, tag_name, vote_count FROM game_tags WHERE appid = ANY($1)",
        &[&appids_i64]
    ).await?;

    let tags = rows.into_iter().map(|r| overachiever_core::GameTag {
        appid: r.get::<_, i64>("appid") as u64,
        tag_name: r.get("tag_name"),
        vote_count: r.get::<_, i32>("vote_count") as u32,
    }).collect();

    Ok(tags)
}

/// Get tags for a single game
pub async fn get_tags_for_game(pool: &Pool, appid: u64) -> Result<Vec<overachiever_core::GameTag>, DbError> {
    let client = pool.get().await?;

    let rows = client.query(
        "SELECT appid, tag_name, vote_count FROM game_tags WHERE appid = $1 ORDER BY vote_count DESC",
        &[&(appid as i64)]
    ).await?;

    let tags = rows.into_iter().map(|r| overachiever_core::GameTag {
        appid: r.get::<_, i64>("appid") as u64,
        tag_name: r.get("tag_name"),
        vote_count: r.get::<_, i32>("vote_count") as u32,
    }).collect();

    Ok(tags)
}

/// Upsert tags for a game (from SteamSpy)
pub async fn upsert_game_tags(
    pool: &Pool,
    appid: u64,
    tags: &[(String, u32)], // (tag_name, vote_count)
) -> Result<usize, DbError> {
    if tags.is_empty() {
        return Ok(0);
    }

    let client = pool.get().await?;
    let mut count = 0;

    for (tag_name, vote_count) in tags {
        client.execute(
            r#"
            INSERT INTO game_tags (appid, tag_name, vote_count, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (appid, tag_name) DO UPDATE SET
                vote_count = EXCLUDED.vote_count,
                updated_at = NOW()
            "#,
            &[&(appid as i64), tag_name, &(*vote_count as i32)]
        ).await?;
        count += 1;
    }

    Ok(count)
}
