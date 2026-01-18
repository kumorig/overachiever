//! Game tags database operations

use deadpool_postgres::Pool;
use crate::db::DbError;

/// Get all unique tag names (for dropdown filter)
pub async fn get_all_tag_names(pool: &Pool) -> Result<Vec<String>, DbError> {
    let client = pool.get().await?;

    let rows = client.query(
        "SELECT DISTINCT tag_name FROM game_tags ORDER BY tag_name",
        &[]
    ).await?;

    let tags = rows.into_iter()
        .map(|r| r.get::<_, String>("tag_name"))
        .collect();

    Ok(tags)
}

/// Get tags for a list of games
pub async fn get_tags_for_games(pool: &Pool, appids: &[u64]) -> Result<Vec<overachiever_core::GameTag>, DbError> {
    if appids.is_empty() {
        return Ok(vec![]);
    }

    let client = pool.get().await?;
    let appids_i64: Vec<i64> = appids.iter().map(|&id| id as i64).collect();

    let rows = client.query(
        "SELECT appid, tag_name, vote_count FROM game_tags WHERE appid = ANY($1)",
        &[&appids_i64]
    ).await?;

    let tags = rows.into_iter().map(|r| overachiever_core::GameTag {
        appid: r.get::<_, i64>("appid") as u64,
        tag_name: r.get("tag_name"),
        vote_count: r.get::<_, i32>("vote_count") as u32,
    }).collect();

    Ok(tags)
}

/// Get tags for a single game
pub async fn get_tags_for_game(pool: &Pool, appid: u64) -> Result<Vec<overachiever_core::GameTag>, DbError> {
    let client = pool.get().await?;

    let rows = client.query(
        "SELECT appid, tag_name, vote_count FROM game_tags WHERE appid = $1 ORDER BY vote_count DESC",
        &[&(appid as i64)]
    ).await?;

    let tags = rows.into_iter().map(|r| overachiever_core::GameTag {
        appid: r.get::<_, i64>("appid") as u64,
        tag_name: r.get("tag_name"),
        vote_count: r.get::<_, i32>("vote_count") as u32,
    }).collect();

    Ok(tags)
}

/// Upsert tags for a game (from SteamSpy)
pub async fn upsert_game_tags(
    pool: &Pool,
    appid: u64,
    tags: &[(String, u32)], // (tag_name, vote_count)
) -> Result<usize, DbError> {
    if tags.is_empty() {
        return Ok(0);
    }

    let client = pool.get().await?;
    let mut count = 0;

    for (tag_name, vote_count) in tags {
        client.execute(
            r#"
            INSERT INTO game_tags (appid, tag_name, vote_count, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (appid, tag_name) DO UPDATE SET
                vote_count = EXCLUDED.vote_count,
                updated_at = NOW()
            "#,
            &[&(appid as i64), tag_name, &(*vote_count as i32)]
        ).await?;
        count += 1;
    }

    Ok(count)
}
