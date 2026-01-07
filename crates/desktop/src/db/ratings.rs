//! User achievement ratings

use rusqlite::{Connection, Result};
use chrono::Utc;

/// Save or update a user's achievement rating
pub fn set_achievement_rating(conn: &Connection, steam_id: &str, appid: u64, apiname: &str, rating: u8) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO user_achievement_ratings (steam_id, appid, apiname, rating, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(steam_id, appid, apiname) DO UPDATE SET
         rating = excluded.rating,
         updated_at = excluded.updated_at",
        rusqlite::params![steam_id, appid, apiname, rating, now],
    )?;
    Ok(())
}

/// Get a user's rating for a specific achievement
#[allow(dead_code)]
pub fn get_achievement_rating(conn: &Connection, steam_id: &str, appid: u64, apiname: &str) -> Result<Option<u8>> {
    let result = conn.query_row(
        "SELECT rating FROM user_achievement_ratings WHERE steam_id = ?1 AND appid = ?2 AND apiname = ?3",
        rusqlite::params![steam_id, appid, apiname],
        |row| row.get(0),
    );
    
    match result {
        Ok(rating) => Ok(Some(rating)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Get all achievement ratings for a user (for loading into memory)
pub fn get_all_achievement_ratings(conn: &Connection, steam_id: &str) -> Result<Vec<(u64, String, u8)>> {
    let mut stmt = conn.prepare(
        "SELECT appid, apiname, rating FROM user_achievement_ratings WHERE steam_id = ?1"
    )?;

    let ratings = stmt.query_map([steam_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?.collect::<Result<Vec<_>>>()?;

    Ok(ratings)
}
