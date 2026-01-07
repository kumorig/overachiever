//! Database module - SQLite storage for desktop app
//!
//! Submodules:
//! - migrations: Schema migrations
//! - games: Game CRUD operations
//! - achievements: Achievement CRUD operations
//! - history: Run and achievement history tracking
//! - first_plays: First play events and log entries
//! - ratings: User achievement ratings
//! - ttb: Time-to-beat cache
//! - sync: Cloud sync import/export
//! - settings: App settings storage

mod migrations;
mod games;
mod achievements;
mod history;
mod first_plays;
mod ratings;
mod ttb;
mod sync;
mod settings;

use rusqlite::{Connection, Result};
use chrono::Utc;

// Re-export all public functions
pub use migrations::finalize_migration;
pub use games::{upsert_games, get_all_games, get_games_needing_achievement_scrape};
pub use achievements::{
    update_game_achievements, mark_game_no_achievements, save_game_achievements,
    get_game_achievements, get_recent_achievements, get_all_achievements_for_export,
};
pub use history::{
    insert_run_history, get_run_history, update_latest_run_history_unplayed,
    update_run_history_total, backfill_run_history_unplayed,
    insert_achievement_history, get_achievement_history,
};
pub use first_plays::{record_first_play, get_recent_first_plays, get_log_entries};
pub use ratings::{set_achievement_rating, get_achievement_rating, get_all_achievement_ratings};
pub use ttb::{cache_ttb_times, get_cached_ttb, get_games_without_ttb};
pub use sync::import_cloud_sync_data;
pub use settings::{record_last_update, get_last_update};

const DB_PATH: &str = "steam_overachiever.db";

/// Open a database connection and initialize tables
pub fn open_connection() -> Result<Connection> {
    let conn = Connection::open(DB_PATH)?;
    init_tables(&conn)?;
    Ok(conn)
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

/// Initialize all database tables
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
    migrations::migrate_games_table(conn)?;

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
    migrations::migrate_add_steam_id(conn, "run_history")?;
    
    // Migration: add unplayed_games column if missing
    migrations::migrate_add_unplayed_games(conn)?;
    
    // Migration: add unplayed_games_total column if missing
    migrations::migrate_add_unplayed_games_total(conn)?;

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
    migrations::migrate_add_steam_id(conn, "achievement_history")?;

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
    migrations::migrate_achievements_table(conn)?;

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
    migrations::migrate_first_plays_table(conn)?;

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
