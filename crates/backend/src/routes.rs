//! REST API routes

use axum::{
    extract::{Path, State},
    http::{StatusCode, HeaderMap},
    Json,
};
use std::sync::Arc;
use overachiever_core::{Game, GameAchievement, GameRating};
use crate::AppState;
use crate::auth::{verify_jwt, Claims};

/// Extract authenticated user from Authorization header
fn extract_user(headers: &HeaderMap, jwt_secret: &str) -> Result<Claims, (StatusCode, Json<serde_json::Value>)> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Missing Authorization header"})))
        })?;
    
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Invalid Authorization header format"})))
        })?;
    
    verify_jwt(token, jwt_secret).map_err(|e| {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": format!("Invalid token: {}", e)})))
    })
}

pub async fn get_games(
    State(_state): State<Arc<AppState>>,
) -> Json<Vec<Game>> {
    // TODO: Get authenticated user and fetch their games
    Json(vec![])
}

pub async fn get_achievements(
    State(_state): State<Arc<AppState>>,
    Path(_appid): Path<u64>,
) -> Json<Vec<GameAchievement>> {
    // TODO: Get authenticated user and fetch achievements
    Json(vec![])
}

pub async fn get_ratings(
    State(state): State<Arc<AppState>>,
    Path(appid): Path<u64>,
) -> Json<Vec<GameRating>> {
    match crate::db::get_community_ratings(&state.db_pool, appid).await {
        Ok(ratings) => Json(ratings),
        Err(_) => Json(vec![]),
    }
}

#[derive(serde::Deserialize)]
pub struct SubmitRatingRequest {
    pub appid: u64,
    pub rating: u8,
    pub comment: Option<String>,
}

pub async fn submit_rating(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<SubmitRatingRequest>,
) -> Json<serde_json::Value> {
    // TODO: Get authenticated user and submit rating
    Json(serde_json::json!({"error": "Not implemented"}))
}

// ============================================================================
// Achievement Rating & Comment Endpoints
// ============================================================================

#[derive(serde::Deserialize)]
pub struct AchievementRatingRequest {
    pub appid: u64,
    pub apiname: String,
    pub rating: u8,
}

#[derive(serde::Serialize)]
pub struct AchievementRatingResponse {
    pub success: bool,
    pub appid: u64,
    pub apiname: String,
}

pub async fn submit_achievement_rating(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<AchievementRatingRequest>,
) -> Result<Json<AchievementRatingResponse>, (StatusCode, Json<serde_json::Value>)> {
    let claims = extract_user(&headers, &state.jwt_secret)?;
    
    // Validate rating is 1-5
    if body.rating < 1 || body.rating > 5 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Rating must be between 1 and 5"}))
        ));
    }
    
    tracing::info!(
        steam_id = %claims.steam_id,
        appid = %body.appid,
        apiname = %body.apiname,
        rating = %body.rating,
        "Achievement rating submitted via REST"
    );
    
    // Store rating in database
    if let Err(e) = crate::db::upsert_achievement_rating(
        &state.db_pool,
        &claims.steam_id,
        body.appid,
        &body.apiname,
        body.rating,
    ).await {
        tracing::error!("Failed to store achievement rating: {:?}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Failed to store rating"}))
        ));
    }
    
    Ok(Json(AchievementRatingResponse {
        success: true,
        appid: body.appid,
        apiname: body.apiname,
    }))
}

/// Response format for user's achievement ratings
#[derive(serde::Serialize)]
pub struct UserAchievementRatingsResponse {
    pub ratings: Vec<AchievementRatingEntry>,
}

#[derive(serde::Serialize)]
pub struct AchievementRatingEntry {
    pub appid: u64,
    pub apiname: String,
    pub rating: u8,
}

/// Get all achievement ratings for the authenticated user
pub async fn get_user_achievement_ratings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<UserAchievementRatingsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let claims = extract_user(&headers, &state.jwt_secret)?;
    
    match crate::db::get_user_achievement_ratings(&state.db_pool, &claims.steam_id).await {
        Ok(ratings) => {
            let entries: Vec<AchievementRatingEntry> = ratings
                .into_iter()
                .map(|(appid, apiname, rating)| AchievementRatingEntry { appid, apiname, rating })
                .collect();
            Ok(Json(UserAchievementRatingsResponse { ratings: entries }))
        }
        Err(e) => {
            tracing::error!("Failed to fetch user achievement ratings: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to fetch ratings"}))
            ))
        }
    }
}

#[derive(serde::Deserialize)]
pub struct AchievementCommentRequest {
    /// List of (appid, apiname) pairs
    pub achievements: Vec<(u64, String)>,
    pub comment: String,
}

#[derive(serde::Serialize)]
pub struct AchievementCommentResponse {
    pub success: bool,
    pub count: usize,
}

pub async fn submit_achievement_comment(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<AchievementCommentRequest>,
) -> Result<Json<AchievementCommentResponse>, (StatusCode, Json<serde_json::Value>)> {
    let claims = extract_user(&headers, &state.jwt_secret)?;
    
    if body.achievements.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No achievements specified"}))
        ));
    }
    
    if body.comment.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Comment cannot be empty"}))
        ));
    }
    
    tracing::info!(
        steam_id = %claims.steam_id,
        achievements = ?body.achievements,
        comment = %body.comment,
        "Achievement comment submitted via REST"
    );
    
    // TODO: Store comment in database
    // For now, just log and return success
    
    Ok(Json(AchievementCommentResponse {
        success: true,
        count: body.achievements.len(),
    }))
}

// ============================================================================
// Cloud Sync Endpoints
// ============================================================================

use overachiever_core::{CloudSyncData, CloudSyncStatus};

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
