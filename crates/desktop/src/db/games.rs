//! Game-related database operations

use rusqlite::{Connection, Result};
use overachiever_core::{Game, SteamGame};
use chrono::Utc;

use super::first_plays::record_first_play;

/// Upsert games from Steam API into the database
pub fn upsert_games(conn: &Connection, steam_id: &str, games: &[SteamGame]) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    for game in games {
        // Check if this is a first play (game existed with 0 playtime, now has playtime)
        if game.playtime_forever > 0 {
            let old_playtime: Option<u32> = conn.query_row(
                "SELECT playtime_forever FROM games WHERE steam_id = ?1 AND appid = ?2",
                [steam_id, &game.appid.to_string()],
                |row| row.get(0),
            ).ok();
            
            if old_playtime == Some(0) {
                // First time playing! Record it using rtime_last_played as the timestamp
                if let Some(played_at) = game.rtime_last_played {
                    let _ = record_first_play(conn, steam_id, game.appid, played_at as i64);
                }
            }
        }
        
        conn.execute(
            "INSERT INTO games (steam_id, appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(steam_id, appid) DO UPDATE SET
             name = excluded.name,
             playtime_forever = excluded.playtime_forever,
             rtime_last_played = excluded.rtime_last_played,
             img_icon_url = excluded.img_icon_url",
            (
                steam_id,
                game.appid,
                &game.name,
                game.playtime_forever,
                game.rtime_last_played,
                &game.img_icon_url,
                &now,
            ),
        )?;
    }
    Ok(())
}

/// Get all games for a user
pub fn get_all_games(conn: &Connection, steam_id: &str) -> Result<Vec<Game>> {
    let mut stmt = conn.prepare(
        "SELECT appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at,
         achievements_total, achievements_unlocked, last_achievement_scrape,
         avg_user_ttb_main_seconds, avg_user_ttb_extra_seconds, avg_user_ttb_completionist_seconds,
         user_ttb_report_count, my_ttb_main_seconds, my_ttb_extra_seconds,
         my_ttb_completionist_seconds, my_ttb_reported_at, hidden, steam_hidden
         FROM games WHERE steam_id = ?1 ORDER BY name"
    )?;
    
    let games = stmt.query_map([steam_id], |row| {
        let added_at_str: String = row.get(5)?;
        let added_at = chrono::DateTime::parse_from_rfc3339(&added_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        
        let last_scrape_str: Option<String> = row.get(8)?;
        let last_achievement_scrape = last_scrape_str.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });
        
        let my_ttb_reported_at_str: Option<String> = row.get(16)?;
        let my_ttb_reported_at = my_ttb_reported_at_str.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });
        
        Ok(Game {
            appid: row.get(0)?,
            name: row.get(1)?,
            playtime_forever: row.get(2)?,
            rtime_last_played: row.get(3)?,
            img_icon_url: row.get(4)?,
            added_at,
            achievements_total: row.get(6)?,
            achievements_unlocked: row.get(7)?,
            last_achievement_scrape,
            avg_user_ttb_main_seconds: row.get(9)?,
            avg_user_ttb_extra_seconds: row.get(10)?,
            avg_user_ttb_completionist_seconds: row.get(11)?,
            user_ttb_report_count: row.get::<_, Option<i32>>(12)?.unwrap_or(0),
            my_ttb_main_seconds: row.get(13)?,
            my_ttb_extra_seconds: row.get(14)?,
            my_ttb_completionist_seconds: row.get(15)?,
            my_ttb_reported_at,
            hidden: row.get::<_, Option<i32>>(17)?.map(|v| v != 0).unwrap_or(false),
            steam_hidden: row.get::<_, Option<i32>>(18)?.map(|v| v != 0).unwrap_or(false),
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(games)
}

/// Get games that haven't been scraped for achievements yet
pub fn get_games_needing_achievement_scrape(conn: &Connection, steam_id: &str) -> Result<Vec<Game>> {
    let mut stmt = conn.prepare(
        "SELECT appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at,
         achievements_total, achievements_unlocked, last_achievement_scrape,
         avg_user_ttb_main_seconds, avg_user_ttb_extra_seconds, avg_user_ttb_completionist_seconds,
         user_ttb_report_count, my_ttb_main_seconds, my_ttb_extra_seconds,
         my_ttb_completionist_seconds, my_ttb_reported_at, hidden, steam_hidden
         FROM games WHERE steam_id = ?1 AND last_achievement_scrape IS NULL ORDER BY name"
    )?;
    
    let games = stmt.query_map([steam_id], |row| {
        let added_at_str: String = row.get(5)?;
        let added_at = chrono::DateTime::parse_from_rfc3339(&added_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        
        let my_ttb_reported_at_str: Option<String> = row.get(16)?;
        let my_ttb_reported_at = my_ttb_reported_at_str.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });
        
        Ok(Game {
            appid: row.get(0)?,
            name: row.get(1)?,
            playtime_forever: row.get(2)?,
            rtime_last_played: row.get(3)?,
            img_icon_url: row.get(4)?,
            added_at,
            achievements_total: row.get(6)?,
            achievements_unlocked: row.get(7)?,
            last_achievement_scrape: None,
            avg_user_ttb_main_seconds: row.get(9)?,
            avg_user_ttb_extra_seconds: row.get(10)?,
            avg_user_ttb_completionist_seconds: row.get(11)?,
            user_ttb_report_count: row.get::<_, Option<i32>>(12)?.unwrap_or(0),
            my_ttb_main_seconds: row.get(13)?,
            my_ttb_extra_seconds: row.get(14)?,
            my_ttb_completionist_seconds: row.get(15)?,
            my_ttb_reported_at,
            hidden: row.get::<_, Option<i32>>(17)?.map(|v| v != 0).unwrap_or(false),
            steam_hidden: row.get::<_, Option<i32>>(18)?.map(|v| v != 0).unwrap_or(false),
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(games)
}

/// Report TTB times for a game (desktop single-user version)
pub fn report_ttb(
    conn: &Connection,
    steam_id: &str,
    appid: u64,
    main_seconds: Option<i32>,
    extra_seconds: Option<i32>,
    completionist_seconds: Option<i32>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    
    conn.execute(
        "UPDATE games SET 
         my_ttb_main_seconds = ?1,
         my_ttb_extra_seconds = ?2,
         my_ttb_completionist_seconds = ?3,
         my_ttb_reported_at = ?4
         WHERE steam_id = ?5 AND appid = ?6",
        (main_seconds, extra_seconds, completionist_seconds, &now, steam_id, appid as i64),
    )?;
    
    Ok(())
}

/// Update hidden status for a game
pub fn set_game_hidden(
    conn: &Connection,
    steam_id: &str,
    appid: u64,
    hidden: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE games SET hidden = ?1 WHERE steam_id = ?2 AND appid = ?3",
        (if hidden { 1 } else { 0 }, steam_id, appid as i64),
    )?;
    
    Ok(())
}
