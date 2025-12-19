//! Database operations for the backend using tokio-postgres

use deadpool_postgres::{Pool, PoolError};
use overachiever_core::{Game, GameAchievement, GameRating, AchievementTip};
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

pub async fn insert_tip(
    pool: &Pool,
    tip: &AchievementTip,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = tip.steam_id.parse().unwrap_or(0);
    
    client.execute(
        r#"
        INSERT INTO achievement_tips (steam_id, appid, apiname, difficulty, tip, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        &[
            &steam_id_int,
            &(tip.appid as i64),
            &tip.apiname,
            &(tip.difficulty as i16),
            &tip.tip,
            &Utc::now(),
        ]
    ).await?;
    
    Ok(())
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
    
    client.execute(
        r#"
        INSERT INTO users (steam_id, display_name, avatar_url, created_at, last_login)
        VALUES ($1, $2, $3, $4, $4)
        ON CONFLICT (steam_id) DO UPDATE SET
            display_name = EXCLUDED.display_name,
            avatar_url = EXCLUDED.avatar_url,
            last_login = EXCLUDED.last_login
        "#,
        &[&steam_id_int, &display_name, &avatar_url, &now]
    ).await?;
    
    Ok(())
}
