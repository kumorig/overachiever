use rusqlite::{Connection, Result};
use crate::models::{Game, RunHistory, SteamGame, Achievement, AchievementHistory, GameAchievement, AchievementSchema};
use chrono::Utc;

const DB_PATH: &str = "steam_overachiever.db";

pub fn open_connection() -> Result<Connection> {
    let conn = Connection::open(DB_PATH)?;
    init_tables(&conn)?;
    Ok(conn)
}

fn init_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS games (
            appid INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            playtime_forever INTEGER NOT NULL,
            rtime_last_played INTEGER,
            img_icon_url TEXT,
            added_at TEXT NOT NULL,
            achievements_total INTEGER,
            achievements_unlocked INTEGER,
            last_achievement_scrape TEXT
        )",
        [],
    )?;

    // Migration: add rtime_last_played column if it doesn't exist
    let _ = conn.execute("ALTER TABLE games ADD COLUMN rtime_last_played INTEGER", []);

    conn.execute(
        "CREATE TABLE IF NOT EXISTS run_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_at TEXT NOT NULL,
            total_games INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS achievement_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            recorded_at TEXT NOT NULL,
            total_achievements INTEGER NOT NULL,
            unlocked_achievements INTEGER NOT NULL,
            games_with_achievements INTEGER NOT NULL,
            avg_completion_percent REAL NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;

    // Table for storing individual achievements per game
    conn.execute(
        "CREATE TABLE IF NOT EXISTS achievements (
            appid INTEGER NOT NULL,
            apiname TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            icon TEXT NOT NULL,
            icon_gray TEXT NOT NULL,
            achieved INTEGER NOT NULL DEFAULT 0,
            unlocktime INTEGER,
            PRIMARY KEY (appid, apiname)
        )",
        [],
    )?;

    Ok(())
}

pub fn upsert_games(conn: &Connection, games: &[SteamGame]) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    for game in games {
        conn.execute(
            "INSERT INTO games (appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(appid) DO UPDATE SET
             name = excluded.name,
             playtime_forever = excluded.playtime_forever,
             rtime_last_played = excluded.rtime_last_played,
             img_icon_url = excluded.img_icon_url",
            (
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

pub fn get_all_games(conn: &Connection) -> Result<Vec<Game>> {
    let mut stmt = conn.prepare(
        "SELECT appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at,
         achievements_total, achievements_unlocked, last_achievement_scrape 
         FROM games ORDER BY name"
    )?;
    
    let games = stmt.query_map([], |row| {
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
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(games)
}

pub fn update_game_achievements(conn: &Connection, appid: u64, achievements: &[Achievement]) -> Result<()> {
    let total = achievements.len() as i32;
    let unlocked = achievements.iter().filter(|a| a.achieved == 1).count() as i32;
    let now = Utc::now().to_rfc3339();
    
    conn.execute(
        "UPDATE games SET achievements_total = ?1, achievements_unlocked = ?2, last_achievement_scrape = ?3 WHERE appid = ?4",
        (total, unlocked, &now, appid),
    )?;
    Ok(())
}

pub fn mark_game_no_achievements(conn: &Connection, appid: u64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE games SET achievements_total = 0, achievements_unlocked = 0, last_achievement_scrape = ?1 WHERE appid = ?2",
        (&now, appid),
    )?;
    Ok(())
}

pub fn get_games_needing_achievement_scrape(conn: &Connection) -> Result<Vec<Game>> {
    let mut stmt = conn.prepare(
        "SELECT appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at,
         achievements_total, achievements_unlocked, last_achievement_scrape 
         FROM games WHERE last_achievement_scrape IS NULL ORDER BY name"
    )?;
    
    let games = stmt.query_map([], |row| {
        let added_at_str: String = row.get(5)?;
        let added_at = chrono::DateTime::parse_from_rfc3339(&added_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        
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
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(games)
}

pub fn insert_run_history(conn: &Connection, total_games: i32) -> Result<()> {
    let now = Utc::now();
    conn.execute(
        "INSERT INTO run_history (run_at, total_games) VALUES (?1, ?2)",
        (now.to_rfc3339(), total_games),
    )?;
    Ok(())
}

pub fn get_run_history(conn: &Connection) -> Result<Vec<RunHistory>> {
    let mut stmt = conn.prepare(
        "SELECT id, run_at, total_games FROM run_history ORDER BY run_at"
    )?;
    
    let history = stmt.query_map([], |row| {
        let run_at_str: String = row.get(1)?;
        let run_at = chrono::DateTime::parse_from_rfc3339(&run_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        
        Ok(RunHistory {
            id: row.get(0)?,
            run_at,
            total_games: row.get(2)?,
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(history)
}

pub fn insert_achievement_history(conn: &Connection, total: i32, unlocked: i32, games_with_ach: i32, avg_pct: f32) -> Result<()> {
    let now = Utc::now();
    conn.execute(
        "INSERT INTO achievement_history (recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent) VALUES (?1, ?2, ?3, ?4, ?5)",
        (now.to_rfc3339(), total, unlocked, games_with_ach, avg_pct),
    )?;
    Ok(())
}

pub fn get_achievement_history(conn: &Connection) -> Result<Vec<AchievementHistory>> {
    let mut stmt = conn.prepare(
        "SELECT id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent FROM achievement_history ORDER BY recorded_at"
    )?;
    
    let history = stmt.query_map([], |row| {
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

/// Save achievements for a game (schema + player progress merged)
pub fn save_game_achievements(
    conn: &Connection,
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
            "INSERT INTO achievements (appid, apiname, name, description, icon, icon_gray, achieved, unlocktime)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(appid, apiname) DO UPDATE SET
             name = excluded.name,
             description = excluded.description,
             icon = excluded.icon,
             icon_gray = excluded.icon_gray,
             achieved = excluded.achieved,
             unlocktime = excluded.unlocktime",
            (
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
pub fn get_game_achievements(conn: &Connection, appid: u64) -> Result<Vec<GameAchievement>> {
    let mut stmt = conn.prepare(
        "SELECT appid, apiname, name, description, icon, icon_gray, achieved, unlocktime
         FROM achievements WHERE appid = ?1 ORDER BY name"
    )?;
    
    let achievements = stmt.query_map([appid], |row| {
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
