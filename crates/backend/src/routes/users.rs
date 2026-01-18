//! User list route handler

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use crate::AppState;

/// Get all users with public profiles
pub async fn get_all_users(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<overachiever_core::UserProfile>>, (StatusCode, Json<serde_json::Value>)> {
    match crate::db::get_all_users(&state.db_pool).await {
        Ok(users) => Ok(Json(users)),
        Err(e) => {
            tracing::error!("Failed to fetch users: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to fetch users"}))
            ))
        }
    }
}
