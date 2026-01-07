//! First play tracking and combined log entries

use rusqlite::{Connection, Result};
use overachiever_core::{FirstPlay, LogEntry};
use chrono::Utc;

use super::achievements::get_recent_achievements;

/// Record a first play event for a game
pub fn record_first_play(conn: &Connection, steam_id: &str, appid: u64, played_at: i64) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO first_plays (steam_id, appid, played_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![steam_id, appid, played_at],
    )?;
    Ok(())
}

/// Get recent first play events
pub fn get_recent_first_plays(conn: &Connection, steam_id: &str, limit: i32) -> Result<Vec<FirstPlay>> {
    let mut stmt = conn.prepare(
        "SELECT f.appid, g.name, f.played_at, g.img_icon_url
         FROM first_plays f
         JOIN games g ON f.steam_id = g.steam_id AND f.appid = g.appid
         WHERE f.steam_id = ?1
         ORDER BY f.played_at DESC
         LIMIT ?2"
    )?;
    
    let first_plays = stmt.query_map(rusqlite::params![steam_id, limit], |row| {
        let played_at_unix: i64 = row.get(2)?;
        let played_at = chrono::DateTime::from_timestamp(played_at_unix, 0)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| Utc::now());
        
        Ok(FirstPlay {
            appid: row.get(0)?,
            game_name: row.get(1)?,
            played_at,
            game_icon_url: row.get(3)?,
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(first_plays)
}

/// Get combined log entries (achievements + first plays), sorted by timestamp descending
pub fn get_log_entries(conn: &Connection, steam_id: &str, limit: i32) -> Result<Vec<LogEntry>> {
    // Get achievements
    let achievements = get_recent_achievements(conn, steam_id, limit)?;
    
    // Get first plays
    let first_plays = get_recent_first_plays(conn, steam_id, limit)?;
    
    // Combine and sort by timestamp
    let mut entries: Vec<LogEntry> = Vec::new();
    
    for ach in achievements {
        entries.push(LogEntry::Achievement {
            appid: ach.appid,
            game_name: ach.game_name,
            apiname: ach.apiname,
            achievement_name: ach.achievement_name,
            timestamp: ach.unlocktime,
            achievement_icon: ach.achievement_icon,
            game_icon_url: ach.game_icon_url,
        });
    }
    
    for fp in first_plays {
        entries.push(LogEntry::FirstPlay {
            appid: fp.appid,
            game_name: fp.game_name,
            timestamp: fp.played_at,
            game_icon_url: fp.game_icon_url,
        });
    }
    
    // Sort by timestamp descending
    entries.sort_by(|a, b| b.timestamp().cmp(&a.timestamp()));
    
    // Limit to requested number
    entries.truncate(limit as usize);
    
    Ok(entries)
}
