//! REST API routes

use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, HeaderMap},
    Json,
};
use std::sync::Arc;
use overachiever_core::{Game, GameAchievement, GameRating};
use crate::AppState;
use crate::auth::{verify_jwt, Claims};

/// List of admin Steam IDs (can mark games as "not for TTB")
const ADMIN_STEAM_IDS: &[&str] = &[
    "76561197975373553", // Main admin
];

/// Check if a steam_id is an admin
fn is_admin(steam_id: &str) -> bool {
    ADMIN_STEAM_IDS.contains(&steam_id)
}

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
// ============================================================================
// Size on Disk Public API
// ============================================================================

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

// ============================================================================
// Time To Beat (TTB) API
// ============================================================================

use overachiever_core::TtbTimes;

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

// ============================================================================
// TTB Blacklist API (admin only for writes, public for reads)
// ============================================================================

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

// ============================================================================
// Game Tags API (SteamSpy data)
// ============================================================================

use overachiever_core::GameTag;

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