//! Cloud sync route handlers

use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    Json,
};
use std::sync::Arc;
use overachiever_core::{CloudSyncData, CloudSyncStatus};
use crate::AppState;
use super::auth::extract_user;

/// Body limit for large uploads (100MB)
pub const UPLOAD_BODY_LIMIT: usize = 100 * 1024 * 1024;

/// Check if user has data in the cloud
pub async fn get_sync_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<CloudSyncStatus>, (StatusCode, Json<serde_json::Value>)> {
    let claims = extract_user(&headers, &state.jwt_secret)?;
    
    match crate::db::get_cloud_sync_status(&state.db_pool, &claims.steam_id).await {
        Ok(status) => Ok(Json(status)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to get sync status: {:?}", e)}))
        ))
    }
}

/// Download all user data from cloud
pub async fn download_sync_data(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<CloudSyncData>, (StatusCode, Json<serde_json::Value>)> {
    let claims = extract_user(&headers, &state.jwt_secret)?;
    
    match crate::db::get_cloud_sync_data(&state.db_pool, &claims.steam_id).await {
        Ok(data) => Ok(Json(data)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to download data: {:?}", e)}))
        ))
    }
}

/// Upload all user data to cloud (overwrites existing)
pub async fn upload_sync_data(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(data): Json<CloudSyncData>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let claims = extract_user(&headers, &state.jwt_secret)?;
    
    // Verify the uploaded data belongs to the authenticated user
    if data.steam_id != claims.steam_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Cannot upload data for a different user"}))
        ));
    }
    
    match crate::db::upload_cloud_sync_data(&state.db_pool, &data).await {
        Ok(_) => {
            tracing::info!(
                steam_id = %claims.steam_id,
                games = data.games.len(),
                achievements = data.achievements.len(),
                "Cloud sync data uploaded"
            );
            Ok(Json(serde_json::json!({
                "success": true,
                "games_uploaded": data.games.len(),
                "achievements_uploaded": data.achievements.len()
            })))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to upload data: {:?}", e)}))
        ))
    }
}

/// Delete all user data from cloud
pub async fn delete_sync_data(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let claims = extract_user(&headers, &state.jwt_secret)?;
    
    match crate::db::delete_cloud_sync_data(&state.db_pool, &claims.steam_id).await {
        Ok(_) => {
            tracing::info!(steam_id = %claims.steam_id, "Cloud sync data deleted");
            Ok(Json(serde_json::json!({"success": true})))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to delete data: {:?}", e)}))
        ))
    }
}
