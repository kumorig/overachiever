//! History tracking - run history and achievement history

use rusqlite::{Connection, Result};
use overachiever_core::{RunHistory, AchievementHistory};
use chrono::Utc;

// ============================================================================
// Run History
// ============================================================================

/// Insert a new run history entry
pub fn insert_run_history(conn: &Connection, steam_id: &str, total_games: i32, unplayed_games_total: i32) -> Result<()> {
    let now = Utc::now();
    conn.execute(
        "INSERT INTO run_history (steam_id, run_at, total_games, unplayed_games, unplayed_games_total) VALUES (?1, ?2, ?3, 0, ?4)",
        (steam_id, now.to_rfc3339(), total_games, unplayed_games_total),
    )?;
    Ok(())
}

/// Get all run history for a user
pub fn get_run_history(conn: &Connection, steam_id: &str) -> Result<Vec<RunHistory>> {
    let mut stmt = conn.prepare(
        "SELECT id, run_at, total_games, COALESCE(unplayed_games, 0), COALESCE(unplayed_games_total, 0) FROM run_history WHERE steam_id = ?1 ORDER BY run_at"
    )?;
    
    let history = stmt.query_map([steam_id], |row| {
        let run_at_str: String = row.get(1)?;
        let run_at = chrono::DateTime::parse_from_rfc3339(&run_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        
        Ok(RunHistory {
            id: row.get(0)?,
            run_at,
            total_games: row.get(2)?,
            unplayed_games: row.get(3)?,
            unplayed_games_total: row.get(4)?,
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(history)
}

/// Update the unplayed_games count for the most recent run_history entry
pub fn update_latest_run_history_unplayed(conn: &Connection, steam_id: &str, unplayed_games: i32) -> Result<()> {
    conn.execute(
        "UPDATE run_history SET unplayed_games = ?1 WHERE steam_id = ?2 AND id = (SELECT MAX(id) FROM run_history WHERE steam_id = ?2)",
        (unplayed_games, steam_id),
    )?;
    Ok(())
}

/// Update the total_games count for the most recent run_history entry
/// Used when recently played games add new games not in GetOwnedGames (e.g., some F2P games)
pub fn update_run_history_total(conn: &Connection, steam_id: &str, total_games: i32) -> Result<()> {
    conn.execute(
        "UPDATE run_history SET total_games = ?1 WHERE steam_id = ?2 AND id = (SELECT MAX(id) FROM run_history WHERE steam_id = ?2)",
        (total_games, steam_id),
    )?;
    Ok(())
}

/// Backfill unplayed_games for run_history entries that still have 0
/// Only updates entries with unplayed_games = 0 (from before this feature was added)
pub fn backfill_run_history_unplayed(conn: &Connection, steam_id: &str, current_unplayed: i32) -> Result<()> {
    conn.execute(
        "UPDATE run_history SET unplayed_games = ?1 WHERE steam_id = ?2 AND unplayed_games = 0",
        (current_unplayed, steam_id),
    )?;
    Ok(())
}

// ============================================================================
// Achievement History
// ============================================================================

/// Insert a new achievement history entry
pub fn insert_achievement_history(conn: &Connection, steam_id: &str, total: i32, unlocked: i32, games_with_ach: i32, avg_pct: f32) -> Result<()> {
    let now = Utc::now();
    conn.execute(
        "INSERT INTO achievement_history (steam_id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (steam_id, now.to_rfc3339(), total, unlocked, games_with_ach, avg_pct),
    )?;
    Ok(())
}

/// Get all achievement history for a user
pub fn get_achievement_history(conn: &Connection, steam_id: &str) -> Result<Vec<AchievementHistory>> {
    let mut stmt = conn.prepare(
        "SELECT id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent FROM achievement_history WHERE steam_id = ?1 ORDER BY recorded_at"
    )?;
    
    let history = stmt.query_map([steam_id], |row| {
        let recorded_at_str: String = row.get(1)?;
        let recorded_at = chrono::DateTime::parse_from_rfc3339(&recorded_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        
        Ok(AchievementHistory {
            id: row.get(0)?,
            recorded_at,
            total_achievements: row.get(2)?,
            unlocked_achievements: row.get(3)?,
            games_with_achievements: row.get(4)?,
            avg_completion_percent: row.get(5)?,
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(history)
}
