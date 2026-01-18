//! User-related database operations

use deadpool_postgres::Pool;
use chrono::Utc;
use rand::Rng;
use crate::db::DbError;

/// Characters used for generating short IDs (URL-safe, case-sensitive)
/// Similar to YouTube's video ID format
const SHORT_ID_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const SHORT_ID_LENGTH: usize = 8;

/// Generate a random short ID (YouTube-style)
pub fn generate_short_id() -> String {
    let mut rng = rand::thread_rng();
    (0..SHORT_ID_LENGTH)
        .map(|_| {
            let idx = rng.gen_range(0..SHORT_ID_CHARS.len());
            SHORT_ID_CHARS[idx] as char
        })
        .collect()
}

/// Generate a unique short_id by checking for collisions
async fn generate_unique_short_id(client: &deadpool_postgres::Client) -> Result<String, DbError> {
    loop {
        let short_id = generate_short_id();
        let exists = client.query_opt(
            "SELECT 1 FROM users WHERE short_id = $1",
            &[&short_id]
        ).await?;
        
        if exists.is_none() {
            return Ok(short_id);
        }
        // Collision detected, try again (extremely rare with 62^8 possibilities)
    }
}

/// Get user info (steam_id, display_name, avatar_url) by short_id
pub async fn get_user_by_short_id(pool: &Pool, short_id: &str) -> Result<Option<overachiever_core::UserProfile>, DbError> {
    let client = pool.get().await?;
    
    let row = client.query_opt(
        r#"
        SELECT steam_id, display_name, avatar_url, short_id
        FROM users
        WHERE short_id = $1
        "#,
        &[&short_id]
    ).await?;
    
    Ok(row.map(|row| {
        overachiever_core::UserProfile {
            steam_id: row.get::<_, i64>("steam_id").to_string(),
            display_name: row.get("display_name"),
            avatar_url: row.get("avatar_url"),
            short_id: row.get("short_id"),
        }
    }))
}

/// Get or create user, returns short_id
pub async fn get_or_create_user(
    pool: &Pool,
    steam_id: &str,
    display_name: &str,
    avatar_url: Option<&str>,
) -> Result<String, DbError> {
    let client = pool.get().await?;
    let steam_id_int: i64 = steam_id.parse().unwrap_or(0);
    let now = Utc::now();
    let avatar: Option<String> = avatar_url.map(|s| s.to_string());
    
    // First, check if user exists and has a short_id
    let existing = client.query_opt(
        "SELECT short_id FROM users WHERE steam_id = $1",
        &[&steam_id_int]
    ).await?;
    
    if let Some(row) = existing {
        // User exists, update last_seen and return existing short_id
        client.execute(
            r#"
            UPDATE users SET
                display_name = $2,
                avatar_url = $3,
                last_seen = $4
            WHERE steam_id = $1
            "#,
            &[&steam_id_int, &display_name, &avatar, &now]
        ).await?;
        
        // Return existing short_id or generate one if missing
        if let Some(short_id) = row.get::<_, Option<String>>("short_id") {
            return Ok(short_id);
        }
        
        // Generate short_id for existing user (migration case)
        let short_id = generate_unique_short_id(&client).await?;
        client.execute(
            "UPDATE users SET short_id = $2 WHERE steam_id = $1",
            &[&steam_id_int, &short_id]
        ).await?;
        return Ok(short_id);
    }
    
    // New user - generate unique short_id
    let short_id = generate_unique_short_id(&client).await?;
    
    client.execute(
        r#"
        INSERT INTO users (steam_id, display_name, avatar_url, short_id, created_at, last_seen)
        VALUES ($1, $2, $3, $4, $5, $5)
        "#,
        &[&steam_id_int, &display_name, &avatar, &short_id, &now]
    ).await?;
    
    Ok(short_id)
}

/// Get all users with their public profiles (short_id, display_name, avatar_url)
pub async fn get_all_users(pool: &Pool) -> Result<Vec<overachiever_core::UserProfile>, DbError> {
    let client = pool.get().await?;
    
    let rows = client.query(
        r#"
        SELECT steam_id, display_name, avatar_url, short_id
        FROM users
        WHERE short_id IS NOT NULL
        ORDER BY display_name
        "#,
        &[]
    ).await?;
    
    Ok(rows.iter().map(|row| {
        overachiever_core::UserProfile {
            steam_id: row.get::<_, i64>("steam_id").to_string(),
            display_name: row.get("display_name"),
            avatar_url: row.get("avatar_url"),
            short_id: row.get("short_id"),
        }
    }).collect())
}
