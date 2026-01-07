//! Achievement-related database operations

use rusqlite::{Connection, Result};
use overachiever_core::{Achievement, AchievementSchema, GameAchievement, RecentAchievement, SyncAchievement};
use chrono::Utc;

/// Update a game's achievement counts
pub fn update_game_achievements(conn: &Connection, steam_id: &str, appid: u64, achievements: &[Achievement]) -> Result<()> {
    let total = achievements.len() as i32;
    let unlocked = achievements.iter().filter(|a| a.achieved == 1).count() as i32;
    let now = Utc::now().to_rfc3339();
    
    conn.execute(
        "UPDATE games SET achievements_total = ?1, achievements_unlocked = ?2, last_achievement_scrape = ?3 WHERE steam_id = ?4 AND appid = ?5",
        (total, unlocked, &now, steam_id, appid),
    )?;
    Ok(())
}

/// Mark a game as having no achievements
pub fn mark_game_no_achievements(conn: &Connection, steam_id: &str, appid: u64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE games SET achievements_total = 0, achievements_unlocked = 0, last_achievement_scrape = ?1 WHERE steam_id = ?2 AND appid = ?3",
        (&now, steam_id, appid),
    )?;
    Ok(())
}

/// Save achievements for a game (schema + player progress merged)
pub fn save_game_achievements(
    conn: &Connection,
    steam_id: &str,
    appid: u64,
    schema: &[AchievementSchema],
    player_achievements: &[Achievement],
) -> Result<()> {
    // Build a map of player achievements for quick lookup
    let player_map: std::collections::HashMap<&str, &Achievement> = player_achievements
        .iter()
        .map(|a| (a.apiname.as_str(), a))
        .collect();
    
    for ach in schema {
        let player = player_map.get(ach.name.as_str());
        let achieved = player.map(|p| p.achieved == 1).unwrap_or(false);
        let unlocktime = player.and_then(|p| if p.unlocktime > 0 { Some(p.unlocktime as i64) } else { None });
        
        conn.execute(
            "INSERT INTO achievements (steam_id, appid, apiname, name, description, icon, icon_gray, achieved, unlocktime)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(steam_id, appid, apiname) DO UPDATE SET
             name = excluded.name,
             description = excluded.description,
             icon = excluded.icon,
             icon_gray = excluded.icon_gray,
             achieved = excluded.achieved,
             unlocktime = excluded.unlocktime",
            (
                steam_id,
                appid,
                &ach.name,
                &ach.display_name,
                &ach.description,
                &ach.icon,
                &ach.icongray,
                achieved as i32,
                unlocktime,
            ),
        )?;
    }
    
    Ok(())
}

/// Load achievements for a specific game
pub fn get_game_achievements(conn: &Connection, steam_id: &str, appid: u64) -> Result<Vec<GameAchievement>> {
    let mut stmt = conn.prepare(
        "SELECT appid, apiname, name, description, icon, icon_gray, achieved, unlocktime
         FROM achievements WHERE steam_id = ?1 AND appid = ?2 ORDER BY name"
    )?;
    
    let achievements = stmt.query_map([steam_id, &appid.to_string()], |row| {
        let unlocktime_unix: Option<i64> = row.get(7)?;
        let unlocktime = unlocktime_unix.map(|ts| {
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|| Utc::now())
        });
        
        Ok(GameAchievement {
            appid: row.get(0)?,
            apiname: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            icon: row.get(4)?,
            icon_gray: row.get(5)?,
            achieved: row.get::<_, i32>(6)? == 1,
            unlocktime,
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(achievements)
}

/// Get recently unlocked achievements (with game name)
pub fn get_recent_achievements(conn: &Connection, steam_id: &str, limit: i32) -> Result<Vec<RecentAchievement>> {
    let mut stmt = conn.prepare(
        "SELECT a.appid, g.name, a.apiname, a.name, a.unlocktime, a.icon, g.img_icon_url
         FROM achievements a
         JOIN games g ON a.steam_id = g.steam_id AND a.appid = g.appid
         WHERE a.steam_id = ?1 AND a.achieved = 1 AND a.unlocktime IS NOT NULL
         ORDER BY a.unlocktime DESC
         LIMIT ?2"
    )?;
    
    let achievements = stmt.query_map(rusqlite::params![steam_id, limit], |row| {
        let unlocktime_unix: i64 = row.get(4)?;
        let unlocktime = chrono::DateTime::from_timestamp(unlocktime_unix, 0)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| Utc::now());
        
        Ok(RecentAchievement {
            appid: row.get(0)?,
            game_name: row.get(1)?,
            apiname: row.get(2)?,
            achievement_name: row.get(3)?,
            unlocktime,
            achievement_icon: row.get(5)?,
            game_icon_url: row.get(6)?,
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(achievements)
}

/// Get all achievements for export (for cloud sync) - lightweight version without icons
pub fn get_all_achievements_for_export(conn: &Connection, steam_id: &str) -> Result<Vec<SyncAchievement>> {
    let mut stmt = conn.prepare(
        "SELECT appid, apiname, achieved, unlocktime
         FROM achievements WHERE steam_id = ?1 ORDER BY appid, apiname"
    )?;
    
    let achievements = stmt.query_map([steam_id], |row| {
        let unlocktime_unix: Option<i64> = row.get(3)?;
        let unlocktime = unlocktime_unix.map(|ts| {
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|| Utc::now())
        });
        
        Ok(SyncAchievement {
            appid: row.get(0)?,
            apiname: row.get(1)?,
            achieved: row.get::<_, i32>(2)? == 1,
            unlocktime,
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(achievements)
}
