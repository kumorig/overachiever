//! Game tags route handlers

use axum::{
    extract::{Path, State},
    http::{StatusCode, HeaderMap},
    Json,
};
use std::sync::Arc;
use overachiever_core::GameTag;
use crate::AppState;
use super::auth::extract_user;

#[derive(serde::Serialize)]
pub struct TagNamesResponse {
    pub tags: Vec<String>,
}

/// Get all unique tag names (for dropdown filter)
/// GET /api/tags
pub async fn get_all_tag_names(
    State(state): State<Arc<AppState>>,
) -> Json<TagNamesResponse> {
    match crate::db::get_all_tag_names(&state.db_pool).await {
        Ok(tags) => Json(TagNamesResponse { tags }),
        Err(e) => {
            tracing::error!("Failed to get tag names: {:?}", e);
            Json(TagNamesResponse { tags: vec![] })
        }
    }
}

#[derive(serde::Deserialize)]
pub struct TagsBatchRequest {
    pub appids: Vec<u64>,
}

#[derive(serde::Serialize)]
pub struct TagsBatchResponse {
    pub tags: Vec<GameTag>,
}

/// Get tags for multiple games
/// POST /api/tags/batch
pub async fn get_tags_batch(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TagsBatchRequest>,
) -> Json<TagsBatchResponse> {
    // Limit to 500 IDs per request
    let appids: Vec<u64> = body.appids.into_iter().take(500).collect();

    match crate::db::get_tags_for_games(&state.db_pool, &appids).await {
        Ok(tags) => Json(TagsBatchResponse { tags }),
        Err(e) => {
            tracing::error!("Failed to get tags batch: {:?}", e);
            Json(TagsBatchResponse { tags: vec![] })
        }
    }
}

/// Get tags for a single game
/// GET /api/tags/{appid}
pub async fn get_tags_for_game(
    State(state): State<Arc<AppState>>,
    Path(appid): Path<u64>,
) -> Json<Vec<GameTag>> {
    match crate::db::get_tags_for_game(&state.db_pool, appid).await {
        Ok(tags) => Json(tags),
        Err(e) => {
            tracing::error!("Failed to get tags for game {}: {:?}", appid, e);
            Json(vec![])
        }
    }
}

#[derive(serde::Deserialize)]
pub struct SubmitTagsRequest {
    pub appid: u64,
    pub tags: Vec<(String, u32)>, // (tag_name, vote_count)
}

#[derive(serde::Serialize)]
pub struct SubmitTagsResponse {
    pub success: bool,
    pub count: usize,
}

/// Submit tags for a game (admin only, from SteamSpy)
/// POST /api/tags
pub async fn submit_tags(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<SubmitTagsRequest>,
) -> Result<Json<SubmitTagsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Require authentication
    let claims = extract_user(&headers, &state.jwt_secret)?;

    tracing::info!(
        steam_id = %claims.steam_id,
        appid = %body.appid,
        tag_count = %body.tags.len(),
        "Tags submitted"
    );

    match crate::db::upsert_game_tags(&state.db_pool, body.appid, &body.tags).await {
        Ok(count) => Ok(Json(SubmitTagsResponse { success: true, count })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save tags: {:?}", e)}))
        ))
    }
}
