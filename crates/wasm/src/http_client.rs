//! HTTP client for REST API calls (ratings, comments)
//!
//! Uses gloo-net for browser fetch API

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

/// Submit an achievement rating via REST API
pub async fn submit_achievement_rating(
    token: &str,
    appid: u64,
    apiname: &str,
    rating: u8,
) -> Result<AchievementRatingResponse, String> {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    
    let url = format!("{}/api/achievement/rating", origin);
    
    let body = AchievementRatingRequest {
        appid,
        apiname: apiname.to_string(),
        rating,
    };
    
    let response = Request::post(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&body)
        .map_err(|e| format!("Failed to serialize request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;
    
    if !response.ok() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Request failed with status {}: {}", status, text));
    }
    
    response
        .json::<AchievementRatingResponse>()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Submit an achievement comment via REST API
#[allow(dead_code)]
pub async fn submit_achievement_comment(
    token: &str,
    achievements: Vec<(u64, String)>,
    comment: &str,
) -> Result<AchievementCommentResponse, String> {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    
    let url = format!("{}/api/achievement/comment", origin);
    
    let body = AchievementCommentRequest {
        achievements,
        comment: comment.to_string(),
    };
    
    let response = Request::post(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&body)
        .map_err(|e| format!("Failed to serialize request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;
    
    if !response.ok() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Request failed with status {}: {}", status, text));
    }
    
    response
        .json::<AchievementCommentResponse>()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Fetch all achievement ratings for the current user
pub async fn fetch_user_achievement_ratings(
    token: &str,
) -> Result<Vec<(u64, String, u8)>, String> {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    
    let url = format!("{}/api/achievement/ratings", origin);
    
    let response = Request::get(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;
    
    if !response.ok() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Request failed with status {}: {}", status, text));
    }
    
    let result = response
        .json::<UserAchievementRatingsResponse>()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    Ok(result.ratings.into_iter().map(|r| (r.appid, r.apiname, r.rating)).collect())
}

// Request/Response types (matching backend)

#[derive(Serialize)]
struct AchievementRatingRequest {
    appid: u64,
    apiname: String,
    rating: u8,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct AchievementRatingResponse {
    pub success: bool,
    pub appid: u64,
    pub apiname: String,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct AchievementCommentRequest {
    achievements: Vec<(u64, String)>,
    comment: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct AchievementCommentResponse {
    pub success: bool,
    pub count: usize,
}

#[derive(Deserialize)]
struct UserAchievementRatingsResponse {
    ratings: Vec<AchievementRatingEntry>,
}

#[derive(Deserialize)]
struct AchievementRatingEntry {
    appid: u64,
    apiname: String,
    rating: u8,
}

// ============================================================================
// Build Info
// ============================================================================

#[derive(Clone, Debug, Deserialize)]
pub struct BuildInfo {
    pub build_number: u32,
    pub build_datetime: String,
}

/// Fetch build info from build_info.json
pub async fn fetch_build_info() -> Result<BuildInfo, String> {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    
    let url = format!("{}/build_info.json", origin);
    
    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch build info: {}", e))?;
    
    if !response.ok() {
        return Err(format!("Build info not found (status {})", response.status()));
    }
    
    response
        .json::<BuildInfo>()
        .await
        .map_err(|e| format!("Failed to parse build info: {}", e))
}
