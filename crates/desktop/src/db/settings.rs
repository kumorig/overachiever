//! App settings storage

use rusqlite::{Connection, Result};
use chrono::Utc;

/// Record the last time an Update was run
pub fn record_last_update(conn: &Connection) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('last_update', ?1)",
        [&now],
    )?;
    Ok(())
}

/// Check if private games have been synced from Steam before
pub fn has_synced_private_games(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'synced_private_games'",
        [],
        |row| row.get::<_, String>(0),
    ).is_ok()
}

/// Record that private games have been synced from Steam
pub fn record_synced_private_games(conn: &Connection) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('synced_private_games', ?1)",
        [&now],
    )?;
    Ok(())
}

/// Check if the initial scan has been completed (baseline data established)
pub fn has_completed_initial_scan(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'initial_scan_complete'",
        [],
        |row| row.get::<_, String>(0),
    ).is_ok()
}

/// Record that the initial scan has been completed (baseline data established)
pub fn record_initial_scan_complete(conn: &Connection) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('initial_scan_complete', ?1)",
        [&now],
    )?;
    Ok(())
}

/// Get the last time an Update was run
pub fn get_last_update(conn: &Connection) -> Result<Option<chrono::DateTime<Utc>>> {
    let result: std::result::Result<String, _> = conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'last_update'",
        [],
        |row| row.get(0),
    );
    
    match result {
        Ok(s) => Ok(chrono::DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .ok()),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}
