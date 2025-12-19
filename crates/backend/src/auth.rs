//! Authentication - Steam OpenID and JWT

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub steam_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub exp: usize,
}

#[derive(Debug, Deserialize)]
pub struct SteamCallbackParams {
    #[serde(rename = "openid.claimed_id")]
    claimed_id: Option<String>,
    // Add other OpenID params as needed
}

pub async fn steam_login() -> impl IntoResponse {
    // Redirect to Steam OpenID
    let return_url = std::env::var("STEAM_CALLBACK_URL")
        .unwrap_or_else(|_| "http://localhost:8080/auth/steam/callback".to_string());
    
    let steam_openid_url = format!(
        "https://steamcommunity.com/openid/login?openid.ns=http://specs.openid.net/auth/2.0&openid.mode=checkid_setup&openid.return_to={}&openid.realm={}&openid.identity=http://specs.openid.net/auth/2.0/identifier_select&openid.claimed_id=http://specs.openid.net/auth/2.0/identifier_select",
        urlencoding::encode(&return_url),
        urlencoding::encode(&return_url.replace("/auth/steam/callback", ""))
    );
    
    Redirect::temporary(&steam_openid_url)
}

pub async fn steam_callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SteamCallbackParams>,
) -> impl IntoResponse {
    // Extract Steam ID from claimed_id
    let steam_id = params.claimed_id
        .and_then(|id| id.rsplit('/').next().map(String::from))
        .unwrap_or_default();
    
    if steam_id.is_empty() {
        return Redirect::temporary("/?error=auth_failed");
    }
    
    // TODO: Verify the OpenID response with Steam
    // TODO: Fetch user profile from Steam API
    
    // Create JWT token
    let claims = Claims {
        steam_id: steam_id.clone(),
        display_name: format!("User {}", &steam_id[..8.min(steam_id.len())]),
        avatar_url: None,
        exp: (chrono::Utc::now() + chrono::Duration::days(7)).timestamp() as usize,
    };
    
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    ).unwrap_or_default();
    
    // Redirect to frontend with token
    Redirect::temporary(&format!("/?token={}", token))
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

pub fn create_jwt(claims: &Claims, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}
