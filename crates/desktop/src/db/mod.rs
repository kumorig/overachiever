use rusqlite::{Connection, Result};
use overachiever_core::{
    Game, RunHistory, SteamGame, Achievement, AchievementHistory,
    GameAchievement, AchievementSchema, RecentAchievement, FirstPlay, LogEntry,
    CloudSyncData, SyncAchievement, TtbTimes
};
use chrono::Utc;
use std::path::PathBuf;

// Helper functions for u64 <-> i64 conversion for SQLite
// rusqlite 0.38+ removed ToSql/FromSql for u64
#[inline]
fn appid_to_sql(appid: u64) -> i64 {
    appid as i64
}

#[inline]
fn appid_from_sql(val: i64) -> u64 {
    val as u64
}

/// Get the path to the database file in the app's data directory
fn get_db_path() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "Overachiever") {
        let data_dir = proj_dirs.data_dir();
        // Create the directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(data_dir) {
            eprintln!("Failed to create data directory: {}", e);
            // Fall back to current directory
            return PathBuf::from("steam_overachiever.db");
        }
        data_dir.join("steam_overachiever.db")
    } else {
        // Fallback to current directory if we can't get the app data dir
        PathBuf::from("steam_overachiever.db")
    }
}

pub fn open_connection() -> Result<Connection> {
    let db_path = get_db_path();
    let conn = Connection::open(db_path)?;
    init_tables(&conn)?;
    Ok(conn)
}

fn init_tables(conn: &Connection) -> Result<()> {
    // Users table (to track multiple steam accounts)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            steam_id TEXT PRIMARY KEY,
            display_name TEXT,
            avatar_url TEXT,
            created_at TEXT NOT NULL,
            last_seen TEXT NOT NULL
        )",
        [],
    )?;

    // Games table with steam_id for multi-user support
    conn.execute(
        "CREATE TABLE IF NOT EXISTS games (
            steam_id TEXT NOT NULL,
            appid INTEGER NOT NULL,
            name TEXT NOT NULL,
            playtime_forever INTEGER NOT NULL,
            rtime_last_played INTEGER,
            img_icon_url TEXT,
            added_at TEXT NOT NULL,
            achievements_total INTEGER,
            achievements_unlocked INTEGER,
            last_achievement_scrape TEXT,
            PRIMARY KEY (steam_id, appid)
        )",
        [],
    )?;

    // Migration: Check if old games table exists without steam_id and migrate
    migrate_games_table(conn)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS run_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            steam_id TEXT NOT NULL,
            run_at TEXT NOT NULL,
            total_games INTEGER NOT NULL
        )",
        [],
    )?;

    // Migration: add steam_id to run_history if missing
    migrate_add_steam_id(conn, "run_history")?;
    
    // Migration: add unplayed_games column if missing
    migrate_add_unplayed_games(conn)?;
    
    // Migration: add unplayed_games_total column if missing
    migrate_add_unplayed_games_total(conn)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS achievement_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            steam_id TEXT NOT NULL,
            recorded_at TEXT NOT NULL,
            total_achievements INTEGER NOT NULL,
            unlocked_achievements INTEGER NOT NULL,
            games_with_achievements INTEGER NOT NULL,
            avg_completion_percent REAL NOT NULL
        )",
        [],
    )?;

    // Migration: add steam_id to achievement_history if missing
    migrate_add_steam_id(conn, "achievement_history")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;

    // Achievements table with steam_id for multi-user support
    conn.execute(
        "CREATE TABLE IF NOT EXISTS achievements (
            steam_id TEXT NOT NULL,
            appid INTEGER NOT NULL,
            apiname TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            icon TEXT NOT NULL,
            icon_gray TEXT NOT NULL,
            achieved INTEGER NOT NULL DEFAULT 0,
            unlocktime INTEGER,
            PRIMARY KEY (steam_id, appid, apiname)
        )",
        [],
    )?;

    // Migration: migrate old achievements table
    migrate_achievements_table(conn)?;
    
    // Migration: add user TTB fields to games table
    migrate_add_user_ttb_fields(conn)?;
    
    // Migration: add is_game_finishing to achievements table
    migrate_add_game_finishing(conn)?;
    
    // Migration: add hidden to games table
    migrate_add_hidden(conn)?;
    
    // Migration: add steam_hidden to games table
    migrate_add_steam_hidden(conn)?;
    
    // Migration: add steam_private to games table
    migrate_add_steam_private(conn)?;

    // First plays table with steam_id
    conn.execute(
        "CREATE TABLE IF NOT EXISTS first_plays (
            steam_id TEXT NOT NULL,
            appid INTEGER NOT NULL,
            played_at INTEGER NOT NULL,
            PRIMARY KEY (steam_id, appid)
        )",
        [],
    )?;

    // Migration: migrate old first_plays table
    migrate_first_plays_table(conn)?;

    // User achievement ratings table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_achievement_ratings (
            steam_id TEXT NOT NULL,
            appid INTEGER NOT NULL,
            apiname TEXT NOT NULL,
            rating INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (steam_id, appid, apiname)
        )",
        [],
    )?;

    // TTB (Time To Beat) cache table - game metadata, not user-specific
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ttb_cache (
            appid INTEGER PRIMARY KEY,
            main REAL,
            main_extra REAL,
            completionist REAL,
            cached_at TEXT NOT NULL
        )",
        [],
    )?;

    // Create indexes for common queries
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_games_steam_id ON games(steam_id)", []);
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_achievements_steam_id ON achievements(steam_id)", []);
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_run_history_steam_id ON run_history(steam_id)", []);
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_achievement_history_steam_id ON achievement_history(steam_id)", []);
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_user_achievement_ratings_steam_id ON user_achievement_ratings(steam_id)", []);

    Ok(())
}

/// Migrate old games table (without steam_id) to new format
fn migrate_games_table(conn: &Connection) -> Result<()> {
    // Check if the old table structure exists (appid as PRIMARY KEY without steam_id)
    let has_steam_id: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('games') WHERE name = 'steam_id'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(true);

    if !has_steam_id {
        // Old table exists, need to migrate
        // Create new table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS games_new (
                steam_id TEXT NOT NULL,
                appid INTEGER NOT NULL,
                name TEXT NOT NULL,
                playtime_forever INTEGER NOT NULL,
                rtime_last_played INTEGER,
                img_icon_url TEXT,
                added_at TEXT NOT NULL,
                achievements_total INTEGER,
                achievements_unlocked INTEGER,
                last_achievement_scrape TEXT,
                PRIMARY KEY (steam_id, appid)
            )",
            [],
        )?;

        // Copy data with a placeholder steam_id (will be updated when config is loaded)
        conn.execute(
            "INSERT INTO games_new SELECT 'migrate_pending', appid, name, playtime_forever, 
             rtime_last_played, img_icon_url, added_at, achievements_total, 
             achievements_unlocked, last_achievement_scrape FROM games",
            [],
        )?;

        // Drop old table and rename new one
        conn.execute("DROP TABLE games", [])?;
        conn.execute("ALTER TABLE games_new RENAME TO games", [])?;
    }

    Ok(())
}

/// Migrate old achievements table to include steam_id
fn migrate_achievements_table(conn: &Connection) -> Result<()> {
    let has_steam_id: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('achievements') WHERE name = 'steam_id'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(true);

    if !has_steam_id {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS achievements_new (
                steam_id TEXT NOT NULL,
                appid INTEGER NOT NULL,
                apiname TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                icon TEXT NOT NULL,
                icon_gray TEXT NOT NULL,
                achieved INTEGER NOT NULL DEFAULT 0,
                unlocktime INTEGER,
                PRIMARY KEY (steam_id, appid, apiname)
            )",
            [],
        )?;

        conn.execute(
            "INSERT INTO achievements_new SELECT 'migrate_pending', appid, apiname, name, 
             description, icon, icon_gray, achieved, unlocktime FROM achievements",
            [],
        )?;

        conn.execute("DROP TABLE achievements", [])?;
        conn.execute("ALTER TABLE achievements_new RENAME TO achievements", [])?;
    }

    Ok(())
}

/// Migrate old first_plays table to include steam_id
fn migrate_first_plays_table(conn: &Connection) -> Result<()> {
    let has_steam_id: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('first_plays') WHERE name = 'steam_id'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(true);

    if !has_steam_id {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS first_plays_new (
                steam_id TEXT NOT NULL,
                appid INTEGER NOT NULL,
                played_at INTEGER NOT NULL,
                PRIMARY KEY (steam_id, appid)
            )",
            [],
        )?;

        conn.execute(
            "INSERT INTO first_plays_new SELECT 'migrate_pending', appid, played_at FROM first_plays",
            [],
        )?;

        conn.execute("DROP TABLE first_plays", [])?;
        conn.execute("ALTER TABLE first_plays_new RENAME TO first_plays", [])?;
    }

    Ok(())
}

/// Add steam_id column to a table if it doesn't exist
fn migrate_add_steam_id(conn: &Connection, table_name: &str) -> Result<()> {
    let has_steam_id: bool = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name = 'steam_id'", table_name),
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(true);

    if !has_steam_id {
        let _ = conn.execute(
            &format!("ALTER TABLE {} ADD COLUMN steam_id TEXT NOT NULL DEFAULT 'migrate_pending'", table_name),
            [],
        );
    }

    Ok(())
}

/// Add unplayed_games column to run_history if it doesn't exist
fn migrate_add_unplayed_games(conn: &Connection) -> Result<()> {
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('run_history') WHERE name = 'unplayed_games'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(true);

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE run_history ADD COLUMN unplayed_games INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    Ok(())
}

/// Add unplayed_games_total column to run_history if it doesn't exist
fn migrate_add_unplayed_games_total(conn: &Connection) -> Result<()> {
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('run_history') WHERE name = 'unplayed_games_total'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(true);

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE run_history ADD COLUMN unplayed_games_total INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    Ok(())
}

/// Add user TTB columns to games table
fn migrate_add_user_ttb_fields(conn: &Connection) -> Result<()> {
    // Check if my_ttb_main_seconds column exists
    let has_ttb: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('games') WHERE name = 'my_ttb_main_seconds'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false);

    if !has_ttb {
        // Add user TTB report columns (for desktop, these are the single user's reports)
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN my_ttb_main_seconds INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN my_ttb_extra_seconds INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN my_ttb_completionist_seconds INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN my_ttb_reported_at TEXT",
            [],
        );
        
        // Also add average fields for consistency with backend (will be populated from cloud sync)
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN avg_user_ttb_main_seconds INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN avg_user_ttb_extra_seconds INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN avg_user_ttb_completionist_seconds INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN user_ttb_report_count INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    Ok(())
}

/// Add is_game_finishing column to achievements table
fn migrate_add_game_finishing(conn: &Connection) -> Result<()> {
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('achievements') WHERE name = 'is_game_finishing'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false);

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE achievements ADD COLUMN is_game_finishing INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    Ok(())
}

/// Add hidden column to games table
fn migrate_add_hidden(conn: &Connection) -> Result<()> {
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('games') WHERE name = 'hidden'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false);

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    Ok(())
}

/// Add steam_hidden column to games table (separate from manual hidden)
fn migrate_add_steam_hidden(conn: &Connection) -> Result<()> {
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('games') WHERE name = 'steam_hidden'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false);

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN steam_hidden INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    Ok(())
}

fn migrate_add_steam_private(conn: &Connection) -> Result<()> {
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('games') WHERE name = 'steam_private'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false);

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE games ADD COLUMN steam_private INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    Ok(())
}

/// Update migrated data with the actual steam_id
pub fn finalize_migration(conn: &Connection, steam_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE games SET steam_id = ?1 WHERE steam_id = 'migrate_pending'",
        [steam_id],
    )?;
    conn.execute(
        "UPDATE achievements SET steam_id = ?1 WHERE steam_id = 'migrate_pending'",
        [steam_id],
    )?;
    conn.execute(
        "UPDATE first_plays SET steam_id = ?1 WHERE steam_id = 'migrate_pending'",
        [steam_id],
    )?;
    conn.execute(
        "UPDATE run_history SET steam_id = ?1 WHERE steam_id = 'migrate_pending'",
        [steam_id],
    )?;
    conn.execute(
        "UPDATE achievement_history SET steam_id = ?1 WHERE steam_id = 'migrate_pending'",
        [steam_id],
    )?;
    Ok(())
}

/// Ensure a user exists in the users table
pub fn ensure_user(conn: &Connection, steam_id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO users (steam_id, created_at, last_seen) VALUES (?1, ?2, ?2)
         ON CONFLICT(steam_id) DO UPDATE SET last_seen = excluded.last_seen",
        [steam_id, &now],
    )?;
    Ok(())
}

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
             rtime_last_played = COALESCE(excluded.rtime_last_played, games.rtime_last_played),
             img_icon_url = excluded.img_icon_url",
            (
                steam_id,
                appid_to_sql(game.appid),
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

pub fn get_all_games(conn: &Connection, steam_id: &str) -> Result<Vec<Game>> {
    let mut stmt = conn.prepare(
        "SELECT appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at,
         achievements_total, achievements_unlocked, last_achievement_scrape, hidden, steam_hidden, steam_private
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
        
        Ok(Game {
            appid: appid_from_sql(row.get(0)?),
            name: row.get(1)?,
            playtime_forever: row.get(2)?,
            rtime_last_played: row.get(3)?,
            img_icon_url: row.get(4)?,
            added_at,
            achievements_total: row.get(6)?,
            achievements_unlocked: row.get(7)?,
            last_achievement_scrape,
            avg_user_ttb_main_seconds: None,
            avg_user_ttb_extra_seconds: None,
            avg_user_ttb_completionist_seconds: None,
            user_ttb_report_count: 0,
            my_ttb_main_seconds: None,
            my_ttb_extra_seconds: None,
            my_ttb_completionist_seconds: None,
            my_ttb_reported_at: None,
            hidden: row.get::<_, Option<i32>>(9)?.map(|v| v != 0).unwrap_or(false),
            steam_hidden: row.get::<_, Option<i32>>(10)?.map(|v| v != 0).unwrap_or(false),
            steam_private: row.get::<_, Option<i32>>(11)?.map(|v| v != 0).unwrap_or(false),
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(games)
}

pub fn update_game_achievements(conn: &Connection, steam_id: &str, appid: u64, achievements: &[Achievement]) -> Result<()> {
    let total = achievements.len() as i32;
    let unlocked = achievements.iter().filter(|a| a.achieved == 1).count() as i32;
    let now = Utc::now().to_rfc3339();
    
    conn.execute(
        "UPDATE games SET achievements_total = ?1, achievements_unlocked = ?2, last_achievement_scrape = ?3 WHERE steam_id = ?4 AND appid = ?5",
        (total, unlocked, &now, steam_id, appid_to_sql(appid)),
    )?;
    Ok(())
}

pub fn mark_game_no_achievements(conn: &Connection, steam_id: &str, appid: u64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE games SET achievements_total = 0, achievements_unlocked = 0, last_achievement_scrape = ?1 WHERE steam_id = ?2 AND appid = ?3",
        (&now, steam_id, appid_to_sql(appid)),
    )?;
    Ok(())
}

pub fn get_games_needing_achievement_scrape(conn: &Connection, steam_id: &str) -> Result<Vec<Game>> {
    let mut stmt = conn.prepare(
        "SELECT appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at,
         achievements_total, achievements_unlocked, last_achievement_scrape, hidden, steam_hidden, steam_private
         FROM games WHERE steam_id = ?1 AND last_achievement_scrape IS NULL ORDER BY name"
    )?;
    
    let games = stmt.query_map([steam_id], |row| {
        let added_at_str: String = row.get(5)?;
        let added_at = chrono::DateTime::parse_from_rfc3339(&added_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        
        Ok(Game {
            appid: appid_from_sql(row.get(0)?),
            name: row.get(1)?,
            playtime_forever: row.get(2)?,
            rtime_last_played: row.get(3)?,
            img_icon_url: row.get(4)?,
            added_at,
            achievements_total: row.get(6)?,
            achievements_unlocked: row.get(7)?,
            last_achievement_scrape: None,
            avg_user_ttb_main_seconds: None,
            avg_user_ttb_extra_seconds: None,
            avg_user_ttb_completionist_seconds: None,
            user_ttb_report_count: 0,
            my_ttb_main_seconds: None,
            my_ttb_extra_seconds: None,
            my_ttb_completionist_seconds: None,
            my_ttb_reported_at: None,
            hidden: row.get::<_, Option<i32>>(9)?.map(|v| v != 0).unwrap_or(false),
            steam_hidden: row.get::<_, Option<i32>>(10)?.map(|v| v != 0).unwrap_or(false),
            steam_private: row.get::<_, Option<i32>>(11)?.map(|v| v != 0).unwrap_or(false),
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(games)
}

pub fn insert_run_history(conn: &Connection, steam_id: &str, total_games: i32, unplayed_games_total: i32) -> Result<()> {
    let now = Utc::now();
    conn.execute(
        "INSERT INTO run_history (steam_id, run_at, total_games, unplayed_games, unplayed_games_total) VALUES (?1, ?2, ?3, 0, ?4)",
        (steam_id, now.to_rfc3339(), total_games, unplayed_games_total),
    )?;
    Ok(())
}

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

pub fn insert_achievement_history(conn: &Connection, steam_id: &str, total: i32, unlocked: i32, games_with_ach: i32, avg_pct: f32) -> Result<()> {
    let now = Utc::now();
    conn.execute(
        "INSERT INTO achievement_history (steam_id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (steam_id, now.to_rfc3339(), total, unlocked, games_with_ach, avg_pct),
    )?;
    Ok(())
}

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
                appid_to_sql(appid),
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
    
    let achievements = stmt.query_map(rusqlite::params![steam_id, appid_to_sql(appid)], |row| {
        let unlocktime_unix: Option<i64> = row.get(7)?;
        let unlocktime = unlocktime_unix.map(|ts| {
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|| Utc::now())
        });
        
        Ok(GameAchievement {
            appid: appid_from_sql(row.get(0)?),
            apiname: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            icon: row.get(4)?,
            icon_gray: row.get(5)?,
            achieved: row.get::<_, i32>(6)? == 1,
            unlocktime,
            is_game_finishing: false,
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
            appid: appid_from_sql(row.get(0)?),
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

/// Record a first play event for a game
pub fn record_first_play(conn: &Connection, steam_id: &str, appid: u64, played_at: i64) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO first_plays (steam_id, appid, played_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![steam_id, appid_to_sql(appid), played_at],
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
            appid: appid_from_sql(row.get(0)?),
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
            appid: appid_from_sql(row.get(0)?),
            apiname: row.get(1)?,
            achieved: row.get::<_, i32>(2)? == 1,
            unlocktime,
        })
    })?.collect::<Result<Vec<_>>>()?;
    
    Ok(achievements)
}

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
            "INSERT INTO games (steam_id, appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at, achievements_total, achievements_unlocked, last_achievement_scrape, avg_user_ttb_main_seconds, avg_user_ttb_extra_seconds, avg_user_ttb_completionist_seconds, user_ttb_report_count, my_ttb_main_seconds, my_ttb_extra_seconds, my_ttb_completionist_seconds, my_ttb_reported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            rusqlite::params![
                steam_id,
                appid_to_sql(game.appid),
                game.name,
                game.playtime_forever,
                game.rtime_last_played,
                game.img_icon_url,
                game.added_at.to_rfc3339(),
                game.achievements_total,
                game.achievements_unlocked,
                game.last_achievement_scrape.as_ref().map(|d| d.to_rfc3339()),
                game.avg_user_ttb_main_seconds,
                game.avg_user_ttb_extra_seconds,
                game.avg_user_ttb_completionist_seconds,
                game.user_ttb_report_count,
                game.my_ttb_main_seconds,
                game.my_ttb_extra_seconds,
                game.my_ttb_completionist_seconds,
                game.my_ttb_reported_at.as_ref().map(|d| d.to_rfc3339()),
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
                appid_to_sql(ach.appid),
                ach.apiname,
                if ach.achieved { 1 } else { 0 },
                ach.unlocktime.map(|t| t.timestamp()),
            ],
        )?;
    }
    
    // Clear last_achievement_scrape for games that have achievements
    // This will trigger a re-scrape to populate icon URLs and metadata
    conn.execute(
        "UPDATE games SET last_achievement_scrape = NULL 
         WHERE steam_id = ?1 AND appid IN (SELECT DISTINCT appid FROM achievements WHERE steam_id = ?1)",
        [steam_id],
    )?;
    
    // Import run history
    for rh in &data.run_history {
        conn.execute(
            "INSERT INTO run_history (steam_id, run_at, total_games, unplayed_games, unplayed_games_total)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                steam_id,
                rh.run_at.to_rfc3339(),
                rh.total_games,
                rh.unplayed_games,
                rh.unplayed_games_total,
            ],
        )?;
    }
    
    // Import achievement history
    for ah in &data.achievement_history {
        conn.execute(
            "INSERT INTO achievement_history (steam_id, recorded_at, total_achievements, unlocked_achievements, games_with_achievements, avg_completion_percent)
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

/// Save or update a user's achievement rating
pub fn set_achievement_rating(conn: &Connection, steam_id: &str, appid: u64, apiname: &str, rating: u8) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO user_achievement_ratings (steam_id, appid, apiname, rating, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(steam_id, appid, apiname) DO UPDATE SET
         rating = excluded.rating,
         updated_at = excluded.updated_at",
        rusqlite::params![steam_id, appid_to_sql(appid), apiname, rating, now],
    )?;
    Ok(())
}

/// Get a user's rating for a specific achievement
#[allow(dead_code)]
pub fn get_achievement_rating(conn: &Connection, steam_id: &str, appid: u64, apiname: &str) -> Result<Option<u8>> {
    let result = conn.query_row(
        "SELECT rating FROM user_achievement_ratings WHERE steam_id = ?1 AND appid = ?2 AND apiname = ?3",
        rusqlite::params![steam_id, appid_to_sql(appid), apiname],
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
        Ok((appid_from_sql(row.get(0)?), row.get(1)?, row.get(2)?))
    })?.collect::<Result<Vec<_>>>()?;

    Ok(ratings)
}

// ============================================================================
// TTB (Time To Beat) Cache Functions
// ============================================================================

/// Cache TTB times for a game locally
pub fn cache_ttb_times(conn: &Connection, times: &TtbTimes) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO ttb_cache (appid, main, main_extra, completionist, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![appid_to_sql(times.appid), times.main, times.main_extra, times.completionist, now],
    )?;
    Ok(())
}

/// Get cached TTB times for a game
pub fn get_cached_ttb(conn: &Connection, appid: u64) -> Result<Option<TtbTimes>> {
    let result = conn.query_row(
        "SELECT appid, main, main_extra, completionist, cached_at FROM ttb_cache WHERE appid = ?1",
        [appid_to_sql(appid)],
        |row| {
            let cached_at_str: String = row.get(4)?;
            let updated_at = chrono::DateTime::parse_from_rfc3339(&cached_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(TtbTimes {
                appid: appid_from_sql(row.get(0)?),
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
        Ok((appid_from_sql(row.get(0)?), row.get(1)?))
    })?.collect::<Result<Vec<_>>>()?;

    Ok(games)
}

