//! Authentication helpers for routes

use axum::http::{StatusCode, HeaderMap};
use axum::Json;
use crate::auth::{verify_jwt, Claims};

/// List of admin Steam IDs (can mark games as "not for TTB")
const ADMIN_STEAM_IDS: &[&str] = &[
    "76561197975373553", // Main admin
];

/// Check if a steam_id is an admin
pub fn is_admin(steam_id: &str) -> bool {
    ADMIN_STEAM_IDS.contains(&steam_id)
}

/// Extract authenticated user from Authorization header
pub fn extract_user(headers: &HeaderMap, jwt_secret: &str) -> Result<Claims, (StatusCode, Json<serde_json::Value>)> {
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
