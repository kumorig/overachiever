//! Database operations for the backend using tokio-postgres

use deadpool_postgres::{Pool, PoolError};
use overachiever_core::{Game, GameAchievement, GameRating, AchievementTip, LogEntry};
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
        SELECT id, run_at, total_games
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
        SELECT id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent
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
pub async fn insert_run_history(pool: &Pool, steam_id: &str, total_games: i32) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    
    client.execute(
        r#"
        INSERT INTO run_history (steam_id, run_at, total_games)
        VALUES ($1, $2, $3)
        "#,
        &[&steam_id_int, &now, &total_games]
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
        SELECT ua.appid, g.name as game_name, s.display_name as achievement_name, 
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
            achievement_name: row.get::<_, Option<String>>("achievement_name").unwrap_or_else(|| "Unknown".to_string()),
            timestamp: row.get("unlocktime"),
            achievement_icon: row.get::<_, Option<String>>("achievement_icon").unwrap_or_default(),
            game_icon_url: row.get("game_icon_url"),
        }
    }).collect();
    
    Ok(entries)
}
