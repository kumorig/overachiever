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
    /// Short ID for shareable profile URLs
    pub short_id: Option<String>,
    pub exp: usize,
}

#[derive(Debug, Deserialize)]
pub struct SteamLoginParams {
    /// For desktop app: localhost callback URL
    pub redirect_uri: Option<String>,
}

pub async fn steam_login(
    Query(params): Query<SteamLoginParams>,
) -> impl IntoResponse {
    // Use custom redirect_uri for desktop, or default for web
    let return_url = if let Some(redirect_uri) = params.redirect_uri {
        // Desktop flow: callback to localhost, but we need to go through our server first
        let base_callback = std::env::var("STEAM_CALLBACK_URL")
            .unwrap_or_else(|_| "http://localhost:8080/auth/steam/callback".to_string());
        format!("{}?redirect_uri={}", base_callback, urlencoding::encode(&redirect_uri))
    } else {
        std::env::var("STEAM_CALLBACK_URL")
            .unwrap_or_else(|_| "http://localhost:8080/auth/steam/callback".to_string())
    };
    
    let realm = return_url.split("/auth").next().unwrap_or(&return_url);
    
    let steam_openid_url = format!(
        "https://steamcommunity.com/openid/login?openid.ns=http://specs.openid.net/auth/2.0&openid.mode=checkid_setup&openid.return_to={}&openid.realm={}&openid.identity=http://specs.openid.net/auth/2.0/identifier_select&openid.claimed_id=http://specs.openid.net/auth/2.0/identifier_select",
        urlencoding::encode(&return_url),
        urlencoding::encode(realm)
    );
    
    Redirect::temporary(&steam_openid_url)
}

#[derive(Debug, Deserialize)]
pub struct SteamCallbackFullParams {
    #[serde(rename = "openid.claimed_id")]
    claimed_id: Option<String>,
    /// For desktop app: where to redirect with the token
    redirect_uri: Option<String>,
}

pub async fn steam_callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SteamCallbackFullParams>,
) -> impl IntoResponse {
    // Extract Steam ID from claimed_id
    let steam_id = params.claimed_id
        .and_then(|id| id.rsplit('/').next().map(String::from))
        .unwrap_or_default();
    
    if steam_id.is_empty() {
        if let Some(redirect_uri) = params.redirect_uri {
            return Redirect::temporary(&format!("{}?error=auth_failed", redirect_uri));
        }
        return Redirect::temporary("/?error=auth_failed");
    }
    
    // TODO: Verify the OpenID response with Steam
    // TODO: Fetch user profile from Steam API
    
    let display_name = format!("User {}", &steam_id[..8.min(steam_id.len())]);
    
    // Create/update user in database and get short_id
    let short_id = match crate::db::get_or_create_user(&state.db_pool, &steam_id, &display_name, None).await {
        Ok(short_id) => short_id,
        Err(e) => {
            tracing::error!("Failed to create user {}: {:?}", steam_id, e);
            let error_str = format!("{:?}", e);
            let error_msg = urlencoding::encode(&error_str);
            if let Some(redirect_uri) = params.redirect_uri {
                return Redirect::temporary(&format!("{}?error=db_error&details={}", redirect_uri, error_msg));
            }
            return Redirect::temporary(&format!("/?error=db_error&details={}", error_msg));
        }
    };
    tracing::info!("User {} created/updated successfully with short_id {}", steam_id, short_id);
    
    // Create JWT token (30 days for desktop, 7 days for web)
    let expiry_days = if params.redirect_uri.is_some() { 30 } else { 7 };
    let claims = Claims {
        steam_id: steam_id.clone(),
        display_name,
        avatar_url: None,
        short_id: Some(short_id),
        exp: (chrono::Utc::now() + chrono::Duration::days(expiry_days)).timestamp() as usize,
    };
    
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    ).unwrap_or_default();
    
    // Redirect to desktop callback or web frontend
    if let Some(redirect_uri) = params.redirect_uri {
        Redirect::temporary(&format!("{}?token={}&steam_id={}", redirect_uri, token, steam_id))
    } else {
        Redirect::temporary(&format!("/?token={}", token))
    }
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
