//! Game-related route handlers

use axum::{
    extract::State,
    Json,
};
use std::sync::Arc;
use overachiever_core::Game;
use crate::AppState;

pub async fn get_games(
    State(_state): State<Arc<AppState>>,
) -> Json<Vec<Game>> {
    // TODO: Get authenticated user and fetch their games
    Json(vec![])
}
