//! Size on disk cache route handlers

use axum::{
    extract::{Query, State},
    http::{StatusCode, HeaderMap},
    Json,
};
use std::sync::Arc;
use crate::AppState;
use super::auth::extract_user;

#[derive(serde::Deserialize)]
pub struct SizeOnDiskQuery {
    #[serde(rename = "appId")]
    pub app_id: String, // comma-separated list of app IDs
}

#[derive(serde::Serialize)]
pub struct SizeOnDiskResponse {
    pub sizes: Vec<crate::db::AppSizeInfo>,
}

/// Public endpoint to query app install sizes
/// GET /size-on-disk?appId=123,456,789
pub async fn get_size_on_disk(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<SizeOnDiskQuery>,
) -> Json<SizeOnDiskResponse> {
    // Parse comma-separated app IDs
    let appids: Vec<u64> = query.app_id
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .take(100) // Limit to 100 IDs per request
        .collect();
    
    // Log the request (fire and forget)
    let client_ip = headers.get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let user_agent = headers.get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let referer = headers.get("referer")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    
    let app_ids_str = query.app_id.clone();
    let pool = state.db_pool.clone();
    
    // Log asynchronously, don't block the response
    tokio::spawn(async move {
        let _ = crate::db::log_api_request(
            &pool,
            "/size-on-disk",
            client_ip.as_deref(),
            user_agent.as_deref(),
            referer.as_deref(),
            None,
            Some(&app_ids_str),
        ).await;
    });
    
    // Get sizes from database
    match crate::db::get_app_sizes(&state.db_pool, &appids).await {
        Ok(sizes) => Json(SizeOnDiskResponse { sizes }),
        Err(e) => {
            tracing::error!("Failed to get app sizes: {:?}", e);
            Json(SizeOnDiskResponse { sizes: vec![] })
        }
    }
}

#[derive(serde::Deserialize)]
pub struct SubmitSizesRequest {
    pub sizes: Vec<crate::db::AppSizeInfo>,
}

#[derive(serde::Serialize)]
pub struct SubmitSizesResponse {
    pub success: bool,
    pub count: usize,
}

/// Desktop clients submit install sizes they've collected from ACF files
/// POST /api/size-on-disk
pub async fn submit_size_on_disk(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<SubmitSizesRequest>,
) -> Result<Json<SubmitSizesResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Require authentication to submit sizes
    let claims = extract_user(&headers, &state.jwt_secret)?;

    // Limit to 1000 sizes per request
    if body.sizes.len() > 1000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Too many sizes in request (max 1000)"}))
        ));
    }

    match crate::db::upsert_app_sizes(&state.db_pool, &body.sizes).await {
        Ok(count) => {
            tracing::info!(
                steam_id = %claims.steam_id,
                count = body.sizes.len(),
                "Size data submitted"
            );
            Ok(Json(SubmitSizesResponse {
                success: true,
                count,
            }))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save sizes: {:?}", e)}))
        ))
    }
}
