//! Game rating and achievement tip database operations

use deadpool_postgres::Pool;
use overachiever_core::{GameRating, AchievementTip};
use chrono::Utc;
use crate::db::DbError;

/// Get community ratings for a game
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

/// Upsert a game rating
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

/// Get achievement tips
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
