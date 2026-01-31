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

/// Fetch list of all users from the backend
pub async fn fetch_all_users(server_url: &str) -> Result<Vec<overachiever_core::UserProfile>, String> {
    // Convert WebSocket URL to HTTP
    let http_url = server_url.replace("ws://", "http://").replace("wss://", "https://");
    let base_url = http_url.trim_end_matches("/ws");
    let url = format!("{}/api/users", base_url);
    
    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch users: {}", e))?;
    
    if !response.ok() {
        return Err(format!("Failed to fetch users (status {})", response.status()));
    }
    
    response
        .json::<Vec<overachiever_core::UserProfile>>()
        .await
        .map_err(|e| format!("Failed to parse users: {}", e))
}

/// Fetch TTB times for multiple games from the backend
pub async fn fetch_ttb_batch(appids: &[u64]) -> Result<Vec<overachiever_core::TtbTimes>, String> {
    if appids.is_empty() {
        return Ok(vec![]);
    }
    
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    
    let url = format!("{}/api/ttb/batch", origin);
    
    let body = serde_json::json!({ "appids": appids });
    
    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .map_err(|e| format!("Failed to serialize request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;
    
    if !response.ok() {
        return Err(format!("Failed to fetch TTB times (status {})", response.status()));
    }
    
    response
        .json::<Vec<overachiever_core::TtbTimes>>()
        .await
        .map_err(|e| format!("Failed to parse TTB times: {}", e))
}

/// Fetch all available tag names from the backend
pub async fn fetch_all_tag_names() -> Result<Vec<String>, String> {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    
    let url = format!("{}/api/tags", origin);
    
    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch tag names: {}", e))?;
    
    if !response.ok() {
        return Err(format!("Failed to fetch tag names (status {})", response.status()));
    }
    
    #[derive(Deserialize)]
    struct TagNamesResponse {
        tags: Vec<String>,
    }
    
    let result = response
        .json::<TagNamesResponse>()
        .await
        .map_err(|e| format!("Failed to parse tag names: {}", e))?;
    
    Ok(result.tags)
}

/// Fetch tags for multiple games from the backend
pub async fn fetch_tags_batch(appids: &[u64]) -> Result<Vec<overachiever_core::GameTag>, String> {
    if appids.is_empty() {
        return Ok(vec![]);
    }
    
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    
    let url = format!("{}/api/tags/batch", origin);
    
    let body = serde_json::json!({ "appids": appids });
    
    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .map_err(|e| format!("Failed to serialize request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;
    
    if !response.ok() {
        return Err(format!("Failed to fetch tags (status {})", response.status()));
    }
    
    #[derive(Deserialize)]
    struct TagsBatchResponse {
        tags: Vec<overachiever_core::GameTag>,
    }
    
    let result = response
        .json::<TagsBatchResponse>()
        .await
        .map_err(|e| format!("Failed to parse tags: {}", e))?;
    
    Ok(result.tags)
}

