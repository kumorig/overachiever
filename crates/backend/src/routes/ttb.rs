//! Time to beat route handlers

use axum::{
    extract::{Path, State},
    http::{StatusCode, HeaderMap},
    Json,
};
use std::sync::Arc;
use overachiever_core::TtbTimes;
use crate::AppState;
use super::auth::{extract_user, is_admin};

#[derive(serde::Deserialize)]
pub struct SubmitTtbRequest {
    pub appid: u64,
    pub game_name: String,
    pub main: Option<f32>,
    pub main_extra: Option<f32>,
    pub completionist: Option<f32>,
}

#[derive(serde::Serialize)]
pub struct TtbResponse {
    pub success: bool,
}

/// Desktop clients submit TTB times scraped from HLTB
/// POST /api/ttb
pub async fn submit_ttb(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<SubmitTtbRequest>,
) -> Result<Json<TtbResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Require authentication to submit
    let claims = extract_user(&headers, &state.jwt_secret)?;

    tracing::info!(
        steam_id = %claims.steam_id,
        appid = %body.appid,
        game_name = %body.game_name,
        "TTB times submitted"
    );

    match crate::db::upsert_ttb_times(
        &state.db_pool,
        body.appid,
        &body.game_name,
        body.main,
        body.main_extra,
        body.completionist,
    ).await {
        Ok(_) => Ok(Json(TtbResponse { success: true })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save TTB times: {:?}", e)}))
        ))
    }
}

/// Get TTB times for a single game
/// GET /api/ttb/{appid}
pub async fn get_ttb(
    State(state): State<Arc<AppState>>,
    Path(appid): Path<u64>,
) -> Json<Option<TtbTimes>> {
    match crate::db::get_ttb_times(&state.db_pool, appid).await {
        Ok(times) => Json(times),
        Err(e) => {
            tracing::error!("Failed to get TTB times: {:?}", e);
            Json(None)
        }
    }
}

#[derive(serde::Deserialize)]
pub struct TtbBatchRequest {
    pub appids: Vec<u64>,
}

/// Get TTB times for multiple games
/// POST /api/ttb/batch
pub async fn get_ttb_batch(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TtbBatchRequest>,
) -> Json<Vec<TtbTimes>> {
    // Limit to 500 IDs per request
    let appids: Vec<u64> = body.appids.into_iter().take(500).collect();

    match crate::db::get_ttb_times_batch(&state.db_pool, &appids).await {
        Ok(times) => Json(times),
        Err(e) => {
            tracing::error!("Failed to get TTB times batch: {:?}", e);
            Json(vec![])
        }
    }
}

#[derive(serde::Deserialize)]
pub struct TtbBlacklistRequest {
    pub appid: u64,
    pub game_name: String,
    pub reason: Option<String>,
}

#[derive(serde::Serialize)]
pub struct TtbBlacklistResponse {
    pub success: bool,
    pub appid: u64,
}

/// Add a game to the TTB blacklist (admin only)
/// POST /api/ttb/blacklist
pub async fn add_to_ttb_blacklist(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<TtbBlacklistRequest>,
) -> Result<Json<TtbBlacklistResponse>, (StatusCode, Json<serde_json::Value>)> {
    let claims = extract_user(&headers, &state.jwt_secret)?;

    // Check if user is admin
    if !is_admin(&claims.steam_id) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Admin access required"}))
        ));
    }

    tracing::info!(
        steam_id = %claims.steam_id,
        appid = %body.appid,
        game_name = %body.game_name,
        reason = ?body.reason,
        "Admin adding game to TTB blacklist"
    );

    match crate::db::add_to_ttb_blacklist(
        &state.db_pool,
        body.appid,
        &body.game_name,
        body.reason.as_deref(),
        &claims.steam_id,
    ).await {
        Ok(_) => Ok(Json(TtbBlacklistResponse {
            success: true,
            appid: body.appid,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to add to blacklist: {:?}", e)}))
        ))
    }
}

/// Remove a game from the TTB blacklist (admin only)
/// DELETE /api/ttb/blacklist/{appid}
pub async fn remove_from_ttb_blacklist(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(appid): Path<u64>,
) -> Result<Json<TtbBlacklistResponse>, (StatusCode, Json<serde_json::Value>)> {
    let claims = extract_user(&headers, &state.jwt_secret)?;

    // Check if user is admin
    if !is_admin(&claims.steam_id) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Admin access required"}))
        ));
    }

    tracing::info!(
        steam_id = %claims.steam_id,
        appid = %appid,
        "Admin removing game from TTB blacklist"
    );

    match crate::db::remove_from_ttb_blacklist(&state.db_pool, appid).await {
        Ok(removed) => {
            if removed {
                Ok(Json(TtbBlacklistResponse {
                    success: true,
                    appid,
                }))
            } else {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": "Game not in blacklist"}))
                ))
            }
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to remove from blacklist: {:?}", e)}))
        ))
    }
}

#[derive(serde::Serialize)]
pub struct TtbBlacklistListResponse {
    pub appids: Vec<u64>,
}

/// Get the full TTB blacklist (public, no auth required)
/// GET /api/ttb/blacklist
pub async fn get_ttb_blacklist(
    State(state): State<Arc<AppState>>,
) -> Json<TtbBlacklistListResponse> {
    match crate::db::get_ttb_blacklist(&state.db_pool).await {
        Ok(appids) => Json(TtbBlacklistListResponse { appids }),
        Err(e) => {
            tracing::error!("Failed to get TTB blacklist: {:?}", e);
            Json(TtbBlacklistListResponse { appids: vec![] })
        }
    }
}
