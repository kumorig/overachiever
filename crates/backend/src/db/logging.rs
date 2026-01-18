//! API request logging

use deadpool_postgres::Pool;
use crate::db::DbError;

/// Log an API request
pub async fn log_api_request(
    pool: &Pool,
    endpoint: &str,
    client_ip: Option<&str>,
    user_agent: Option<&str>,
    referer: Option<&str>,
    query_params: Option<&str>,
    app_ids: Option<&str>,
) -> Result<(), DbError> {
    let client = pool.get().await?;

    client.execute(
        r#"
        INSERT INTO api_request_log (endpoint, client_ip, user_agent, referer, query_params, app_ids, requested_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        "#,
        &[&endpoint, &client_ip, &user_agent, &referer, &query_params, &app_ids]
    ).await?;

    Ok(())
}
