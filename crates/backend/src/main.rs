//! Overachiever Backend Server
//! 
//! Provides:
//! - WebSocket API for real-time sync
//! - REST API for initial data load
//! - Steam API proxy for WASM clients
//! - PostgreSQL storage for user data

mod db;
mod steam_api;
mod ws_handler;
mod auth;
mod routes;

use axum::{
    routing::{get, post, delete},
    Router,
};
use deadpool_postgres::{Config, Runtime, Pool};
use tokio_postgres::NoTls;
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use axum::extract::DefaultBodyLimit;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::sync::Arc;

pub struct AppState {
    pub db_pool: Pool,
    pub jwt_secret: String,
    pub steam_api_key: Option<String>,
}

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenvy::dotenv().ok();
    
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "overachiever_backend=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    // Database connection pool
    let mut cfg = Config::new();
    cfg.host = std::env::var("DB_HOST").ok();
    cfg.port = std::env::var("DB_PORT").ok().and_then(|p| p.parse().ok());
    cfg.dbname = std::env::var("DB_NAME").ok();
    cfg.user = std::env::var("DB_USER").ok();
    cfg.password = std::env::var("DB_PASSWORD").ok();
    
    let db_pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)
        .expect("Failed to create database pool");
    
    // Test connection
    let _ = db_pool.get().await.expect("Failed to connect to database");
    tracing::info!("Connected to database");
    
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev-secret-change-in-production".to_string());
    
    let steam_api_key = std::env::var("STEAM_API_KEY").ok();
    if steam_api_key.is_none() {
        tracing::warn!("STEAM_API_KEY not set - Steam sync will be disabled");
    }
    
    let state = Arc::new(AppState {
        db_pool,
        jwt_secret,
        steam_api_key,
    });
    
    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(|| async { "OK" }))
        // WebSocket endpoint
        .route("/ws", get(ws_handler::ws_handler))
        // REST API
        .route("/api/games", get(routes::get_games))
        .route("/api/games/{appid}/achievements", get(routes::get_achievements))
        .route("/api/community/ratings/{appid}", get(routes::get_ratings))
        .route("/api/community/ratings", post(routes::submit_rating))
        // Achievement rating/comment endpoints
        .route("/api/achievement/rating", post(routes::submit_achievement_rating))
        .route("/api/achievement/ratings", get(routes::get_user_achievement_ratings))
        .route("/api/achievement/comment", post(routes::submit_achievement_comment))
        // Cloud sync endpoints
        .route("/api/sync/status", get(routes::get_sync_status))
        .route("/api/sync/download", get(routes::download_sync_data))
        .route("/api/sync/upload", post(routes::upload_sync_data)
            .layer(DefaultBodyLimit::max(routes::UPLOAD_BODY_LIMIT)))
        .route("/api/sync/data", delete(routes::delete_sync_data))
        // Size on disk endpoints
        .route("/size-on-disk", get(routes::get_size_on_disk))
        .route("/api/size-on-disk", post(routes::submit_size_on_disk))
        // Time to beat (TTB) endpoints
        .route("/api/ttb", post(routes::submit_ttb))
        .route("/api/ttb/{appid}", get(routes::get_ttb))
        .route("/api/ttb/batch", post(routes::get_ttb_batch))
        // TTB blacklist endpoints
        .route("/api/ttb/blacklist", get(routes::get_ttb_blacklist))
        .route("/api/ttb/blacklist", post(routes::add_to_ttb_blacklist))
        .route("/api/ttb/blacklist/{appid}", delete(routes::remove_from_ttb_blacklist))
        // Game tags endpoints (SteamSpy data)
        .route("/api/tags", get(routes::get_all_tag_names))
        .route("/api/tags", post(routes::submit_tags))
        .route("/api/tags/{appid}", get(routes::get_tags_for_game))
        .route("/api/tags/batch", post(routes::get_tags_batch))
        // Auth
        .route("/auth/steam", get(auth::steam_login))
        .route("/auth/steam/callback", get(auth::steam_callback))
        // User list
        .route("/api/users", get(routes::get_all_users))
        .with_state(state)
        .layer(CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any))
        .layer(TraceLayer::new_for_http());
    
    // Start server
    let addr = std::env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    
    tracing::info!("Starting server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
