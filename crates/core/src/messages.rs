//! WebSocket message types for client-server communication

use serde::{Deserialize, Serialize};
use crate::models::*;

/// Messages sent from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Authenticate with JWT token
    Authenticate { token: String },
    
    /// Request user's games list
    FetchGames,
    
    /// Request achievements for a specific game
    FetchAchievements { appid: u64 },
    
    /// Request sync from Steam API (server-side)
    SyncFromSteam,
    
    /// Submit a game rating
    SubmitRating { 
        appid: u64, 
        rating: u8, 
        comment: Option<String> 
    },
    
    /// Submit an achievement tip
    SubmitAchievementTip { 
        appid: u64, 
        apiname: String, 
        difficulty: u8, 
        tip: String 
    },
    
    /// Get community ratings for a game
    GetCommunityRatings { appid: u64 },
    
    /// Get community tips for an achievement
    GetCommunityTips { appid: u64, apiname: String },
    
    /// Ping to keep connection alive
    Ping,
}

/// Messages sent from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Authentication successful
    Authenticated { 
        user: UserProfile 
    },
    
    /// Authentication failed
    AuthError { 
        reason: String 
    },
    
    /// User's games list
    Games { 
        games: Vec<Game> 
    },
    
    /// Achievements for a game
    Achievements { 
        appid: u64, 
        achievements: Vec<GameAchievement> 
    },
    
    /// Sync progress update
    SyncProgress { 
        state: SyncState 
    },
    
    /// Sync completed
    SyncComplete { 
        result: SyncResult,
        games: Vec<Game>,
    },
    
    /// Community ratings for a game
    CommunityRatings { 
        appid: u64,
        avg_rating: f32,
        rating_count: i32,
        ratings: Vec<GameRating> 
    },
    
    /// Community tips for an achievement
    CommunityTips { 
        appid: u64,
        apiname: String,
        tips: Vec<AchievementTip> 
    },
    
    /// Rating submitted successfully
    RatingSubmitted { appid: u64 },
    
    /// Tip submitted successfully
    TipSubmitted { appid: u64, apiname: String },
    
    /// History data
    History {
        run_history: Vec<RunHistory>,
        achievement_history: Vec<AchievementHistory>,
        log_entries: Vec<LogEntry>,
    },
    
    /// Generic error
    Error { 
        message: String 
    },
    
    /// Pong response
    Pong,
}

/// Sync state for progress reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum SyncState {
    /// Starting sync
    Starting,
    /// Fetching games from Steam
    FetchingGames,
    /// Fetching recently played
    FetchingRecentlyPlayed,
    /// Scraping achievements
    ScrapingAchievements { 
        current: i32, 
        total: i32, 
        game_name: String 
    },
    /// A game was updated
    GameUpdated { 
        appid: u64, 
        unlocked: i32, 
        total: i32 
    },
    /// Sync completed
    Done,
    /// Sync failed
    Error { message: String },
}
