//! Achievement-related route handlers

use axum::{
    extract::{Path, State},
    http::{StatusCode, HeaderMap},
    Json,
};
use std::sync::Arc;
use overachiever_core::GameAchievement;
use crate::AppState;
use super::auth::extract_user;

pub async fn get_achievements(
    State(_state): State<Arc<AppState>>,
    Path(_appid): Path<u64>,
) -> Json<Vec<GameAchievement>> {
    // TODO: Get authenticated user and fetch achievements
    Json(vec![])
}

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
