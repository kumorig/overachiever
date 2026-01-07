//! Cloud sync import/export functions

use rusqlite::{Connection, Result};
use overachiever_core::CloudSyncData;

/// Import cloud sync data into local database (overwrites existing data for this user)
pub fn import_cloud_sync_data(conn: &Connection, data: &CloudSyncData) -> Result<()> {
    let steam_id = &data.steam_id;
    
    // Start transaction
    conn.execute("BEGIN TRANSACTION", [])?;
    
    // Delete existing data for this user
    conn.execute("DELETE FROM games WHERE steam_id = ?1", [steam_id])?;
    conn.execute("DELETE FROM achievements WHERE steam_id = ?1", [steam_id])?;
    conn.execute("DELETE FROM run_history WHERE steam_id = ?1", [steam_id])?;
    conn.execute("DELETE FROM achievement_history WHERE steam_id = ?1", [steam_id])?;
    
    // Import games
    for game in &data.games {
        conn.execute(
            "INSERT INTO games (steam_id, appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at, achievements_total, achievements_unlocked, last_achievement_scrape)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                steam_id,
                game.appid,
                game.name,
                game.playtime_forever,
                game.rtime_last_played,
                game.img_icon_url,
                game.added_at.to_rfc3339(),
                game.achievements_total,
                game.achievements_unlocked,
                game.last_achievement_scrape.as_ref().map(|d| d.to_rfc3339()),
            ],
        )?;
    }
    
    // Import achievements (lightweight - only sync achieved status, not full metadata)
    // The metadata (name, description, icons) will be populated by local scrape
    for ach in &data.achievements {
        // Use INSERT OR REPLACE to update existing or insert new
        conn.execute(
            "INSERT OR REPLACE INTO achievements (steam_id, appid, apiname, name, description, icon, icon_gray, achieved, unlocktime)
             VALUES (?1, ?2, ?3, 
                COALESCE((SELECT name FROM achievements WHERE steam_id = ?1 AND appid = ?2 AND apiname = ?3), ''),
                COALESCE((SELECT description FROM achievements WHERE steam_id = ?1 AND appid = ?2 AND apiname = ?3), NULL),
                COALESCE((SELECT icon FROM achievements WHERE steam_id = ?1 AND appid = ?2 AND apiname = ?3), ''),
                COALESCE((SELECT icon_gray FROM achievements WHERE steam_id = ?1 AND appid = ?2 AND apiname = ?3), ''),
                ?4, ?5)",
            rusqlite::params![
                steam_id,
                ach.appid,
                ach.apiname,
                if ach.achieved { 1 } else { 0 },
                ach.unlocktime.map(|t| t.timestamp()),
            ],
        )?;
    }
    
    // Import run history
    for rh in &data.run_history {
        let played_games = rh.total_games - rh.unplayed_games;
        conn.execute(
            "INSERT INTO run_history (steam_id, recorded_at, total_games, played_games, unplayed_games, unplayed_games_total)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                steam_id,
                rh.run_at.to_rfc3339(),
                rh.total_games,
                played_games,
                rh.unplayed_games,
                rh.unplayed_games_total,
            ],
        )?;
    }
    
    // Import achievement history
    for ah in &data.achievement_history {
        conn.execute(
            "INSERT INTO achievement_history (steam_id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                steam_id,
                ah.recorded_at.to_rfc3339(),
                ah.total_achievements,
                ah.unlocked_achievements,
                ah.games_with_achievements,
                ah.avg_completion_percent,
            ],
        )?;
    }
    
    // Commit transaction
    conn.execute("COMMIT", [])?;
    
    Ok(())
}
