//! Error types for Overachiever

use thiserror::Error;

#[derive(Error, Debug)]
pub enum OverachieverError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Steam API error: {0}")]
    SteamApi(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    #[error("Authentication error: {0}")]
    Auth(String),
    
    #[error("Not authenticated")]
    NotAuthenticated,
    
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

pub type Result<T> = std::result::Result<T, OverachieverError>;
