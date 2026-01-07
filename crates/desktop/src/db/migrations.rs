//! Database migrations for schema updates

use rusqlite::{Connection, Result};

/// Migrate old games table (without steam_id) to new format
pub fn migrate_games_table(conn: &Connection) -> Result<()> {
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
pub fn migrate_achievements_table(conn: &Connection) -> Result<()> {
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
pub fn migrate_first_plays_table(conn: &Connection) -> Result<()> {
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
pub fn migrate_add_steam_id(conn: &Connection, table_name: &str) -> Result<()> {
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
pub fn migrate_add_unplayed_games(conn: &Connection) -> Result<()> {
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
pub fn migrate_add_unplayed_games_total(conn: &Connection) -> Result<()> {
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
