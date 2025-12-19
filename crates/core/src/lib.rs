//! Core shared types and traits for Overachiever
//! 
//! This crate contains:
//! - Data models shared between desktop, WASM, and backend
//! - WebSocket message types for client-server communication
//! - Error types
//! - Data provider trait abstraction

pub mod models;
pub mod messages;
pub mod error;

pub use models::*;
pub use messages::*;
pub use error::*;
