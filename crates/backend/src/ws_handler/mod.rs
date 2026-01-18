//! WebSocket handler for real-time sync

mod handlers;
mod sync;

use axum::{
    extract::{
        ws::WebSocketUpgrade,
        State,
    },
    response::IntoResponse,
};
use std::sync::Arc;
use crate::AppState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handlers::handle_socket(socket, state))
}
