//! Time to beat (TTB) database operations

use deadpool_postgres::Pool;
use crate::db::DbError;

/// Upsert TTB times for a game (from desktop scraper)
pub async fn upsert_ttb_times(
    pool: &Pool,
    appid: u64,
    game_name: &str,
    main: Option<f32>,
    main_extra: Option<f32>,
    completionist: Option<f32>,
) -> Result<(), DbError> {
    let client = pool.get().await?;

    client.execute(
        r#"
        INSERT INTO ttb_times (appid, game_name, main, main_extra, completionist, reported_count, first_reported_at, last_reported_at)
        VALUES ($1, $2, $3, $4, $5, 1, NOW(), NOW())
        ON CONFLICT (appid) DO UPDATE SET
            game_name = EXCLUDED.game_name,
            main = COALESCE(EXCLUDED.main, ttb_times.main),
            main_extra = COALESCE(EXCLUDED.main_extra, ttb_times.main_extra),
            completionist = COALESCE(EXCLUDED.completionist, ttb_times.completionist),
            reported_count = ttb_times.reported_count + 1,
            last_reported_at = NOW()
        "#,
        &[
            &(appid as i64),
            &game_name,
            &main,
            &main_extra,
            &completionist,
        ]
    ).await?;

    Ok(())
}

/// Get TTB times for a single game
pub async fn get_ttb_times(pool: &Pool, appid: u64) -> Result<Option<overachiever_core::TtbTimes>, DbError> {
    let client = pool.get().await?;

    let row = client.query_opt(
        "SELECT appid, main, main_extra, completionist, last_reported_at FROM ttb_times WHERE appid = $1",
        &[&(appid as i64)]
    ).await?;

    Ok(row.map(|r| overachiever_core::TtbTimes {
        appid: r.get::<_, i64>("appid") as u64,
        main: r.get("main"),
        main_extra: r.get("main_extra"),
        completionist: r.get("completionist"),
        updated_at: r.get("last_reported_at"),
    }))
}

/// Get TTB times for multiple games
pub async fn get_ttb_times_batch(pool: &Pool, appids: &[u64]) -> Result<Vec<overachiever_core::TtbTimes>, DbError> {
    if appids.is_empty() {
        return Ok(vec![]);
    }

    let client = pool.get().await?;
    let appids_i64: Vec<i64> = appids.iter().map(|&id| id as i64).collect();

    let rows = client.query(
        "SELECT appid, main, main_extra, completionist, last_reported_at FROM ttb_times WHERE appid = ANY($1)",
        &[&appids_i64]
    ).await?;

    let times = rows.into_iter().map(|r| overachiever_core::TtbTimes {
        appid: r.get::<_, i64>("appid") as u64,
        main: r.get("main"),
        main_extra: r.get("main_extra"),
        completionist: r.get("completionist"),
        updated_at: r.get("last_reported_at"),
    }).collect();

    Ok(times)
}

/// Add a game to the TTB blacklist
pub async fn add_to_ttb_blacklist(
    pool: &Pool,
    appid: u64,
    game_name: &str,
    reason: Option<&str>,
    added_by_steam_id: &str,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = added_by_steam_id.parse().unwrap_or(0);

    client.execute(
        r#"
        INSERT INTO ttb_blacklist (appid, game_name, reason, added_by_steam_id, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        ON CONFLICT (appid) DO UPDATE SET
            game_name = EXCLUDED.game_name,
            reason = EXCLUDED.reason,
            added_by_steam_id = EXCLUDED.added_by_steam_id,
            created_at = NOW()
        "#,
        &[
            &(appid as i64),
            &game_name,
            &reason,
            &steam_id_int,
        ]
    ).await?;

    Ok(())
}

/// Remove a game from the TTB blacklist
pub async fn remove_from_ttb_blacklist(pool: &Pool, appid: u64) -> Result<bool, DbError> {
    let client = pool.get().await?;

    let rows_affected = client.execute(
        "DELETE FROM ttb_blacklist WHERE appid = $1",
        &[&(appid as i64)]
    ).await?;

    Ok(rows_affected > 0)
}

/// Get all games in the TTB blacklist (returns list of appids)
pub async fn get_ttb_blacklist(pool: &Pool) -> Result<Vec<u64>, DbError> {
    let client = pool.get().await?;

    let rows = client.query(
        "SELECT appid FROM ttb_blacklist ORDER BY created_at DESC",
        &[]
    ).await?;

    let appids = rows.into_iter()
        .map(|r| r.get::<_, i64>("appid") as u64)
        .collect();

    Ok(appids)
}

/// Check if a game is in the TTB blacklist
#[allow(dead_code)]
pub async fn is_in_ttb_blacklist(pool: &Pool, appid: u64) -> Result<bool, DbError> {
    let client = pool.get().await?;

    let row = client.query_opt(
        "SELECT 1 FROM ttb_blacklist WHERE appid = $1",
        &[&(appid as i64)]
    ).await?;

    Ok(row.is_some())
}

/// Report user's TTB times for a game
pub async fn report_ttb(
    pool: &Pool,
    steam_id: &str,
    appid: u64,
    main_seconds: Option<i32>,
    extra_seconds: Option<i32>,
    completionist_seconds: Option<i32>,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    // Insert or update the report
    // The trigger will automatically update averages
    client.execute(
        r#"
        INSERT INTO user_ttb_reports (steam_id, appid, main_seconds, extra_seconds, completionist_seconds, reported_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        ON CONFLICT (steam_id, appid) DO UPDATE SET
            main_seconds = EXCLUDED.main_seconds,
            extra_seconds = EXCLUDED.extra_seconds,
            completionist_seconds = EXCLUDED.completionist_seconds,
            reported_at = NOW()
        "#,
        &[&steam_id_int, &(appid as i64), &main_seconds, &extra_seconds, &completionist_seconds]
    ).await?;
    
    Ok(())
}
