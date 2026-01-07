//! TTB (Time To Beat) cache functions

use rusqlite::{Connection, Result};
use overachiever_core::TtbTimes;
use chrono::Utc;

/// Cache TTB times for a game locally
pub fn cache_ttb_times(conn: &Connection, times: &TtbTimes) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO ttb_cache (appid, main, main_extra, completionist, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![times.appid, times.main, times.main_extra, times.completionist, now],
    )?;
    Ok(())
}

/// Get cached TTB times for a game
pub fn get_cached_ttb(conn: &Connection, appid: u64) -> Result<Option<TtbTimes>> {
    let result = conn.query_row(
        "SELECT appid, main, main_extra, completionist, cached_at FROM ttb_cache WHERE appid = ?1",
        [appid],
        |row| {
            let cached_at_str: String = row.get(4)?;
            let updated_at = chrono::DateTime::parse_from_rfc3339(&cached_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(TtbTimes {
                appid: row.get(0)?,
                main: row.get(1)?,
                main_extra: row.get(2)?,
                completionist: row.get(3)?,
                updated_at,
            })
        },
    );

    match result {
        Ok(times) => Ok(Some(times)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Get all game appids that don't have cached TTB data
pub fn get_games_without_ttb(conn: &Connection, steam_id: &str) -> Result<Vec<(u64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT g.appid, g.name FROM games g
         LEFT JOIN ttb_cache t ON g.appid = t.appid
         WHERE g.steam_id = ?1 AND t.appid IS NULL
         ORDER BY g.name"
    )?;

    let games = stmt.query_map([steam_id], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?.collect::<Result<Vec<_>>>()?;

    Ok(games)
}
