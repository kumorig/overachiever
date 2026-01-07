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
