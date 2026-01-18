//! Size on disk cache database operations

use deadpool_postgres::Pool;
use crate::db::DbError;

/// App size data for caching install sizes
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSizeInfo {
    pub appid: u64,
    pub size_bytes: u64,
}

/// Upsert app size data (updates if exists, inserts if not)
pub async fn upsert_app_sizes(pool: &Pool, sizes: &[AppSizeInfo]) -> Result<usize, DbError> {
    if sizes.is_empty() {
        return Ok(0);
    }
    
    let client = pool.get().await?;
    let mut count = 0;
    
    for size in sizes {
        let result = client.execute(
            r#"
            INSERT INTO app_size_on_disk (appid, size_bytes, reported_count, first_reported_at, last_reported_at)
            VALUES ($1, $2, 1, NOW(), NOW())
            ON CONFLICT (appid) DO UPDATE SET
                size_bytes = CASE 
                    WHEN app_size_on_disk.size_bytes = EXCLUDED.size_bytes THEN app_size_on_disk.size_bytes
                    ELSE EXCLUDED.size_bytes
                END,
                reported_count = app_size_on_disk.reported_count + 1,
                last_reported_at = NOW()
            "#,
            &[&(size.appid as i64), &(size.size_bytes as i64)]
        ).await?;
        count += result as usize;
    }
    
    Ok(count)
}

/// Get sizes for a list of app IDs
pub async fn get_app_sizes(pool: &Pool, appids: &[u64]) -> Result<Vec<AppSizeInfo>, DbError> {
    if appids.is_empty() {
        return Ok(vec![]);
    }
    
    let client = pool.get().await?;
    
    // Convert to i64 for postgres
    let appids_i64: Vec<i64> = appids.iter().map(|&id| id as i64).collect();
    
    let rows = client.query(
        "SELECT appid, size_bytes FROM app_size_on_disk WHERE appid = ANY($1)",
        &[&appids_i64]
    ).await?;
    
    let sizes = rows.into_iter().map(|row| {
        AppSizeInfo {
            appid: row.get::<_, i64>("appid") as u64,
            size_bytes: row.get::<_, i64>("size_bytes") as u64,
        }
    }).collect();
    
    Ok(sizes)
}
