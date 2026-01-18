//! Game rating route handlers

use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use overachiever_core::GameRating;
use crate::AppState;

pub async fn get_ratings(
    State(state): State<Arc<AppState>>,
    Path(appid): Path<u64>,
) -> Json<Vec<GameRating>> {
    match crate::db::get_community_ratings(&state.db_pool, appid).await {
        Ok(ratings) => Json(ratings),
        Err(_) => Json(vec![]),
    }
}

#[allow(dead_code)]
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
