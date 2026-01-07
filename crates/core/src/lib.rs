//! Core shared types and traits for Overachiever
//! 
//! This crate contains:
//! - Data models shared between desktop, WASM, and backend
//! - WebSocket message types for client-server communication
//! - Error types
//! - Shared UI components (with `ui` feature)

pub mod constants;
pub mod models;
pub mod messages;
pub mod error;

#[cfg(feature = "ui")]
pub mod ui;

pub use constants::*;
pub use models::*;
pub use messages::*;
pub use error::*;

#[cfg(feature = "ui")]
pub use ui::*;
