//! Achievement-related database operations

use deadpool_postgres::Pool;
use overachiever_core::GameAchievement;
use crate::db::DbError;

/// Get achievements for a game by short_id (for guest viewing)
pub async fn get_game_achievements_by_short_id(
    pool: &Pool,
    short_id: &str,
    appid: u64,
) -> Result<Option<Vec<GameAchievement>>, DbError> {
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
        SELECT ua.appid, ua.apiname, s.display_name as name, s.description,
               s.icon, s.icon_gray, ua.achieved, ua.unlocktime, ua.is_game_finishing
        FROM user_achievements ua
        LEFT JOIN achievement_schemas s ON ua.appid = s.appid AND ua.apiname = s.apiname
        WHERE ua.steam_id = $1 AND ua.appid = $2
        ORDER BY s.display_name
        "#,
        &[&steam_id_int, &(appid as i64)]
    ).await?;
    
    let achievements = rows.into_iter().map(|row| {
        GameAchievement {
            appid: row.get::<_, i64>("appid") as u64,
            apiname: row.get("apiname"),
            name: row.get::<_, Option<String>>("name").unwrap_or_else(|| row.get("apiname")),
            description: row.get("description"),
            icon: row.get::<_, Option<String>>("icon").unwrap_or_default(),
            icon_gray: row.get::<_, Option<String>>("icon_gray").unwrap_or_default(),
            achieved: row.get("achieved"),
            unlocktime: row.get("unlocktime"),
            is_game_finishing: row.get::<_, Option<bool>>("is_game_finishing").unwrap_or(false),
        }
    }).collect();
    
    Ok(Some(achievements))
}

/// Get achievements for a game by steam_id
pub async fn get_game_achievements(
    pool: &Pool,
    steam_id: &str,
    appid: u64,
) -> Result<Vec<GameAchievement>, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    let rows = client.query(
        r#"
        SELECT ua.appid, ua.apiname, s.display_name as name, s.description,
               s.icon, s.icon_gray, ua.achieved, ua.unlocktime, ua.is_game_finishing
        FROM user_achievements ua
        LEFT JOIN achievement_schemas s ON ua.appid = s.appid AND ua.apiname = s.apiname
        WHERE ua.steam_id = $1 AND ua.appid = $2
        ORDER BY s.display_name
        "#,
        &[&steam_id_int, &(appid as i64)]
    ).await?;
    
    let achievements = rows.into_iter().map(|row| {
        GameAchievement {
            appid: row.get::<_, i64>("appid") as u64,
            apiname: row.get("apiname"),
            name: row.get::<_, Option<String>>("name").unwrap_or_default(),
            description: row.get("description"),
            icon: row.get::<_, Option<String>>("icon").unwrap_or_default(),
            icon_gray: row.get::<_, Option<String>>("icon_gray").unwrap_or_default(),
            achieved: row.get::<_, Option<bool>>("achieved").unwrap_or(false),
            unlocktime: row.get("unlocktime"),
            is_game_finishing: row.get::<_, Option<bool>>("is_game_finishing").unwrap_or(false),
        }
    }).collect();
    
    Ok(achievements)
}

/// Store achievement schema
pub async fn upsert_achievement_schema(
    pool: &Pool,
    appid: u64,
    schema: &overachiever_core::AchievementSchema,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    
    client.execute(
        r#"
        INSERT INTO achievement_schemas (appid, apiname, display_name, description, icon, icon_gray)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (appid, apiname) DO UPDATE SET
            display_name = EXCLUDED.display_name,
            description = EXCLUDED.description,
            icon = EXCLUDED.icon,
            icon_gray = EXCLUDED.icon_gray
        "#,
        &[
            &(appid as i64),
            &schema.name,
            &schema.display_name,
            &schema.description,
            &schema.icon,
            &schema.icongray,
        ]
    ).await?;
    
    Ok(())
}

/// Store user achievement progress
pub async fn upsert_user_achievement(
    pool: &Pool,
    steam_id: &str,
    appid: u64,
    achievement: &overachiever_core::Achievement,
) -> Result<(), DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let achieved = achievement.achieved == 1;
    let unlocktime: Option<chrono::DateTime<chrono::Utc>> = if achievement.unlocktime > 0 {
        chrono::DateTime::from_timestamp(achievement.unlocktime as i64, 0)
    } else {
        None
    };
    
    client.execute(
        r#"
        INSERT INTO user_achievements (steam_id, appid, apiname, achieved, unlocktime)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (steam_id, appid, apiname) DO UPDATE SET
            achieved = EXCLUDED.achieved,
            unlocktime = COALESCE(EXCLUDED.unlocktime, user_achievements.unlocktime)
        "#,
        &[
            &steam_id_int,
            &(appid as i64),
            &achievement.apiname,
            &achieved,
            &unlocktime,
        ]
    ).await?;
    
    Ok(())
}

/// Mark an achievement as game-finishing for a user
/// Automatically unmarks any previously marked achievement for the same game
pub async fn mark_game_finishing(
    pool: &Pool,
    steam_id: &str,
    appid: u64,
    apiname: &str,
) -> Result<(), DbError> {
    let mut client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    
    // Start a transaction to ensure atomicity
    let tx = client.transaction().await?;
    
    // First, unmark all achievements for this game
    tx.execute(
        r#"
        UPDATE user_achievements
        SET is_game_finishing = FALSE
        WHERE steam_id = $1 AND appid = $2
        "#,
        &[&steam_id_int, &(appid as i64)]
    ).await?;
    
    // Then mark the specified achievement
    tx.execute(
        r#"
        UPDATE user_achievements
        SET is_game_finishing = TRUE
        WHERE steam_id = $1 AND appid = $2 AND apiname = $3
        "#,
        &[&steam_id_int, &(appid as i64), &apiname]
    ).await?;
    
    tx.commit().await?;
    
    Ok(())
}
