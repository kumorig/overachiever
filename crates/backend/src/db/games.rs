//! Game-related database operations

use deadpool_postgres::Pool;
use overachiever_core::Game;
use chrono::{DateTime, Utc};
use crate::db::DbError;

/// Get games for a user by their short_id
pub async fn get_user_games_by_short_id(pool: &Pool, short_id: &str) -> Result<Option<Vec<Game>>, DbError> {
    let client = pool.get().await?;
    
    // First get the steam_id for this short_id
    let user_row = client.query_opt(
        "SELECT steam_id FROM users WHERE short_id = $1",
        &[&short_id]
    ).await?;
    
    let steam_id_int: i64 = match user_row {
        Some(row) => row.get("steam_id"),
        None => return Ok(None),
    };
    
    let rows = client.query(
        r#"
        SELECT appid, name, playtime_forever, rtime_last_played, img_icon_url,
               added_at, achievements_total, achievements_unlocked, last_sync,
               avg_user_ttb_main_seconds, avg_user_ttb_extra_seconds, 
               avg_user_ttb_completionist_seconds, user_ttb_report_count,
               my_ttb_main_seconds, my_ttb_extra_seconds, 
               my_ttb_completionist_seconds, my_ttb_reported_at, hidden, steam_hidden
        FROM user_games
        WHERE steam_id = $1
        ORDER BY name
        "#,
        &[&steam_id_int]
    ).await?;
    
    let games = rows.into_iter().map(|row| {
        Game {
            appid: row.get::<_, i64>("appid") as u64,
            name: row.get("name"),
            playtime_forever: row.get::<_, i32>("playtime_forever") as u32,
            rtime_last_played: row.get::<_, Option<i32>>("rtime_last_played").map(|t| t as u32),
            img_icon_url: row.get("img_icon_url"),
            added_at: row.get::<_, Option<DateTime<Utc>>>("added_at").unwrap_or_else(Utc::now),
            achievements_total: row.get("achievements_total"),
            achievements_unlocked: row.get("achievements_unlocked"),
            last_achievement_scrape: row.get("last_sync"),
            avg_user_ttb_main_seconds: row.get("avg_user_ttb_main_seconds"),
            avg_user_ttb_extra_seconds: row.get("avg_user_ttb_extra_seconds"),
            avg_user_ttb_completionist_seconds: row.get("avg_user_ttb_completionist_seconds"),
            user_ttb_report_count: row.get::<_, Option<i32>>("user_ttb_report_count").unwrap_or(0),
            my_ttb_main_seconds: row.get("my_ttb_main_seconds"),
            my_ttb_extra_seconds: row.get("my_ttb_extra_seconds"),
            my_ttb_completionist_seconds: row.get("my_ttb_completionist_seconds"),
            my_ttb_reported_at: row.get("my_ttb_reported_at"),
            hidden: row.get::<_, Option<bool>>("hidden").unwrap_or(false),
            steam_hidden: row.get::<_, Option<bool>>("steam_hidden").unwrap_or(false),
            steam_private: false,  // Not stored in database yet
        }
    }).collect();
    
    Ok(Some(games))
}

/// Get games for a user by steam_id
pub async fn get_user_games(pool: &Pool, steam_id: &str) -> Result<Vec<Game>, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    let rows = client.query(
        r#"
        SELECT appid, name, playtime_forever, rtime_last_played, img_icon_url,
               added_at, achievements_total, achievements_unlocked, last_sync,
               avg_user_ttb_main_seconds, avg_user_ttb_extra_seconds, 
               avg_user_ttb_completionist_seconds, user_ttb_report_count,
               my_ttb_main_seconds, my_ttb_extra_seconds, 
               my_ttb_completionist_seconds, my_ttb_reported_at, hidden, steam_hidden
        FROM user_games
        WHERE steam_id = $1
        ORDER BY name
        "#,
        &[&steam_id_int]
    ).await?;
    
    let games = rows.into_iter().map(|row| {
        Game {
            appid: row.get::<_, i64>("appid") as u64,
            name: row.get("name"),
            playtime_forever: row.get::<_, i32>("playtime_forever") as u32,
            rtime_last_played: row.get::<_, Option<i32>>("rtime_last_played").map(|t| t as u32),
            img_icon_url: row.get("img_icon_url"),
            added_at: row.get::<_, Option<DateTime<Utc>>>("added_at").unwrap_or_else(Utc::now),
            achievements_total: row.get("achievements_total"),
            achievements_unlocked: row.get("achievements_unlocked"),
            last_achievement_scrape: row.get("last_sync"),
            avg_user_ttb_main_seconds: row.get("avg_user_ttb_main_seconds"),
            avg_user_ttb_extra_seconds: row.get("avg_user_ttb_extra_seconds"),
            avg_user_ttb_completionist_seconds: row.get("avg_user_ttb_completionist_seconds"),
            user_ttb_report_count: row.get::<_, Option<i32>>("user_ttb_report_count").unwrap_or(0),
            my_ttb_main_seconds: row.get("my_ttb_main_seconds"),
            my_ttb_extra_seconds: row.get("my_ttb_extra_seconds"),
            my_ttb_completionist_seconds: row.get("my_ttb_completionist_seconds"),
            my_ttb_reported_at: row.get("my_ttb_reported_at"),
            hidden: row.get::<_, Option<bool>>("hidden").unwrap_or(false),
            steam_hidden: row.get::<_, Option<bool>>("steam_hidden").unwrap_or(false),
            steam_private: false,  // Not stored in database yet
        }
    }).collect();
    
    Ok(games)
}

/// Insert or update games for a user
pub async fn upsert_games(
    pool: &Pool,
    steam_id: &str,
    games: &[overachiever_core::SteamGame],
) -> Result<usize, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    
    let mut count = 0;
    for game in games {
        client.execute(
            r#"
            INSERT INTO user_games (steam_id, appid, name, playtime_forever, rtime_last_played, img_icon_url, added_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (steam_id, appid) DO UPDATE SET
                name = EXCLUDED.name,
                playtime_forever = EXCLUDED.playtime_forever,
                rtime_last_played = EXCLUDED.rtime_last_played,
                img_icon_url = EXCLUDED.img_icon_url
            "#,
            &[
                &steam_id_int,
                &(game.appid as i64),
                &game.name,
                &(game.playtime_forever as i32),
                &game.rtime_last_played.map(|t| t as i32),
                &game.img_icon_url,
                &now,
            ]
        ).await?;
        count += 1;
    }
    
    Ok(count)
}

/// Update achievement counts for a game
pub async fn update_game_achievements(
    pool: &Pool,
    steam_id: &str,
    appid: u64,
    total: i32,
    unlocked: i32,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    
    client.execute(
        r#"
        UPDATE user_games
        SET achievements_total = $3, achievements_unlocked = $4, last_sync = $5
        WHERE steam_id = $1 AND appid = $2
        "#,
        &[
            &steam_id_int,
            &(appid as i64),
            &total,
            &unlocked,
            &now,
        ]
    ).await?;
    
    Ok(())
}

/// Update hidden status for a game
pub async fn update_game_hidden(
    pool: &Pool,
    steam_id: &str,
    appid: u64,
    hidden: bool,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    client.execute(
        r#"
        UPDATE user_games
        SET hidden = $3
        WHERE steam_id = $1 AND appid = $2
        "#,
        &[
            &steam_id_int,
            &(appid as i64),
            &hidden,
        ]
    ).await?;
    
    Ok(())
}
