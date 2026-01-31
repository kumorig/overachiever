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
    
    /// Request full achievement scan (scrape all games)
    FullScan { force: bool },
    
    /// Refresh achievements for a single game
    RefreshSingleGame { appid: u64 },
    
    /// Request history data
    FetchHistory,
    
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
    
    /// Submit an achievement rating (1-5 stars)
    SubmitAchievementRating {
        appid: u64,
        apiname: String,
        rating: u8,
    },
    
    /// Submit a comment for multiple achievements
    SubmitAchievementComment {
        /// List of (appid, apiname) tuples
        achievements: Vec<(u64, String)>,
        comment: String,
    },
    
    /// Get community ratings for a game
    GetCommunityRatings { appid: u64 },
    
    /// Get community tips for an achievement
    GetCommunityTips { appid: u64, apiname: String },
    
    /// View another user's library by short_id (no authentication required)
    ViewGuestLibrary { short_id: String },
    
    /// Request achievements for a game when viewing as guest
    FetchGuestAchievements { short_id: String, appid: u64 },
    
    /// Request history data when viewing as guest
    FetchGuestHistory { short_id: String },
    
    /// Report user's Time to Beat times
    ReportTtb {
        appid: u64,
        main_seconds: Option<i32>,
        extra_seconds: Option<i32>,
        completionist_seconds: Option<i32>,
    },
    
    /// Mark an achievement as game-finishing
    MarkGameFinishing {
        appid: u64,
        apiname: String,
    },
    
    /// Set hidden status for a game
    SetGameHidden {
        appid: u64,
        hidden: bool,
    },
    
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
    
    /// Achievement rating submitted successfully
    AchievementRatingSubmitted { appid: u64, apiname: String },
    
    /// Achievement comment submitted successfully
    AchievementCommentSubmitted { count: usize },
    
    /// Single game refresh completed
    SingleGameRefreshComplete {
        appid: u64,
        game: Game,
        achievements: Vec<GameAchievement>,
    },
    
    /// History data
    History {
        run_history: Vec<RunHistory>,
        achievement_history: Vec<AchievementHistory>,
        log_entries: Vec<LogEntry>,
    },
    
    /// Guest library view (another user's games)
    GuestLibrary {
        user: UserProfile,
        games: Vec<Game>,
    },
    
    /// Guest library not found (invalid short_id)
    GuestNotFound { short_id: String },
    
    /// TTB report submitted successfully
    TtbReported {
        appid: u64,
        game: Game,
    },
    
    /// Show TTB reporting dialog (triggered on 100% completion)
    ShowTtbDialog {
        appid: u64,
        game_name: String,
        completion_message: Option<String>,
    },
    
    /// Game-finishing achievement marked
    GameFinishingMarked {
        appid: u64,
        achievements: Vec<GameAchievement>,
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
