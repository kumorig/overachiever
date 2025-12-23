//! Shared data models used across all platforms

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Data mode for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DataMode {
    /// All data stored locally in SQLite (desktop only)
    #[default]
    Local,
    /// Personal data local, community ratings/tips synced to server
    Hybrid,
    /// All data synced to remote server (works in WASM)
    Remote,
}

impl DataMode {
    pub fn requires_api_key(&self) -> bool {
        matches!(self, DataMode::Local | DataMode::Hybrid)
    }

    pub fn requires_server(&self) -> bool {
        matches!(self, DataMode::Hybrid | DataMode::Remote)
    }

    pub fn label(&self) -> &'static str {
        match self {
            DataMode::Local => "Local Only",
            DataMode::Hybrid => "Hybrid (Local + Community)",
            DataMode::Remote => "Cloud (Full Remote)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            DataMode::Local => "Essentially air-gapped: No remote communication or storage except Steam API. Requires Steam API key.",
            DataMode::Hybrid => "Personal data local, Accesses community ratings/tips. Shares steam_id if you post comments/ratings. Requires Steam API key.",
            DataMode::Remote => "All data synced to server. Login with Steam, no API key needed. Your game list is stored remotely, assossiated with your steam_id.",
        }
    }
}

/// Raw game data from Steam API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamGame {
    pub appid: u64,
    pub name: String,
    pub playtime_forever: u32,
    pub playtime_windows_forever: Option<u32>,
    pub playtime_mac_forever: Option<u32>,
    pub playtime_linux_forever: Option<u32>,
    pub playtime_deck_forever: Option<u32>,
    pub rtime_last_played: Option<u32>,
    pub img_icon_url: Option<String>,
}

/// Game with tracked data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub appid: u64,
    pub name: String,
    pub playtime_forever: u32,
    pub rtime_last_played: Option<u32>,
    pub img_icon_url: Option<String>,
    pub added_at: DateTime<Utc>,
    pub achievements_total: Option<i32>,
    pub achievements_unlocked: Option<i32>,
    pub last_achievement_scrape: Option<DateTime<Utc>>,
}

impl Game {
    pub fn achievements_display(&self) -> String {
        match (self.achievements_unlocked, self.achievements_total) {
            (Some(unlocked), Some(total)) if total > 0 => format!("{} / {}", unlocked, total),
            (Some(_), Some(0)) => "N/A".to_string(),
            _ => "—".to_string(),
        }
    }

    pub fn completion_percent(&self) -> Option<f32> {
        match (self.achievements_unlocked, self.achievements_total) {
            (Some(unlocked), Some(total)) if total > 0 => {
                Some(unlocked as f32 / total as f32 * 100.0)
            }
            _ => None,
        }
    }
}

/// Achievement progress from Steam API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub apiname: String,
    pub achieved: u8,
    pub unlocktime: u32,
}

/// Achievement definition from Steam schema API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementSchema {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub icon: String,
    pub icongray: String,
}

/// Achievement stored in database with display info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAchievement {
    pub appid: u64,
    pub apiname: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: String,
    pub icon_gray: String,
    pub achieved: bool,
    pub unlocktime: Option<DateTime<Utc>>,
}

/// Run history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunHistory {
    pub id: i64,
    pub run_at: DateTime<Utc>,
    pub total_games: i32,
    pub unplayed_games: i32,
}

/// History of achievement progress over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementHistory {
    pub id: i64,
    pub recorded_at: DateTime<Utc>,
    pub total_achievements: i32,
    pub unlocked_achievements: i32,
    pub games_with_achievements: i32,
    pub avg_completion_percent: f32,
}

/// A recently unlocked achievement with game info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentAchievement {
    pub appid: u64,
    pub game_name: String,
    pub apiname: String,
    pub achievement_name: String,
    pub unlocktime: DateTime<Utc>,
    pub achievement_icon: String,
    pub game_icon_url: Option<String>,
}

/// First play event for a game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstPlay {
    pub appid: u64,
    pub game_name: String,
    pub played_at: DateTime<Utc>,
    pub game_icon_url: Option<String>,
}

/// A log entry that can be either an achievement or first play
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogEntry {
    Achievement {
        appid: u64,
        apiname: String,
        game_name: String,
        achievement_name: String,
        timestamp: DateTime<Utc>,
        achievement_icon: String,
        game_icon_url: Option<String>,
    },
    FirstPlay {
        appid: u64,
        game_name: String,
        timestamp: DateTime<Utc>,
        game_icon_url: Option<String>,
    },
}

impl LogEntry {
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            LogEntry::Achievement { timestamp, .. } => *timestamp,
            LogEntry::FirstPlay { timestamp, .. } => *timestamp,
        }
    }
}

// ============================================================================
// Community features (for Hybrid and Remote modes)
// ============================================================================

/// Game rating submitted by a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRating {
    pub id: Option<i64>,
    pub steam_id: String,
    pub appid: u64,
    pub rating: u8, // 1-5 stars
    pub comment: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Achievement tip/guide submitted by a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementTip {
    pub id: Option<i64>,
    pub steam_id: String,
    pub appid: u64,
    pub apiname: String,
    pub difficulty: u8, // 1-5
    pub tip: String,
    pub created_at: DateTime<Utc>,
}

/// Achievement rating submitted by a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementRating {
    pub id: Option<i64>,
    pub steam_id: String,
    pub appid: u64,
    pub apiname: String,
    pub rating: u8, // 1-5 stars
    pub created_at: DateTime<Utc>,
}

/// Achievement comment that can tag multiple achievements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementComment {
    pub id: Option<i64>,
    pub steam_id: String,
    /// List of (appid, apiname) tuples for tagged achievements
    pub achievements: Vec<(u64, String)>,
    pub comment: String,
    pub created_at: DateTime<Utc>,
}

/// Aggregated community rating for a game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityGameRating {
    pub appid: u64,
    pub avg_rating: f32,
    pub rating_count: i32,
    pub ratings: Vec<GameRating>,
}

/// User profile from Steam
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserProfile {
    pub steam_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

/// Sync result after updating from Steam
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub games_updated: i32,
    pub achievements_updated: i32,
    pub new_games: i32,
}

/// GDPR consent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GdprConsent {
    /// User has not yet responded to GDPR dialog
    #[default]
    Unset,
    /// User accepted data processing
    Accepted,
    /// User declined data processing
    Declined,
}

impl GdprConsent {
    /// Returns true if user has made a choice (accepted or declined)
    pub fn is_set(&self) -> bool {
        !matches!(self, GdprConsent::Unset)
    }
    
    /// Returns true if user has accepted
    pub fn is_accepted(&self) -> bool {
        matches!(self, GdprConsent::Accepted)
    }
}

// ============================================================================
// Cloud Sync Types
// ============================================================================

/// Lightweight achievement data for cloud sync (no icon URLs - saves bandwidth)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAchievement {
    pub appid: u64,
    pub apiname: String,
    pub achieved: bool,
    pub unlocktime: Option<DateTime<Utc>>,
}

impl From<&GameAchievement> for SyncAchievement {
    fn from(a: &GameAchievement) -> Self {
        Self {
            appid: a.appid,
            apiname: a.apiname.clone(),
            achieved: a.achieved,
            unlocktime: a.unlocktime,
        }
    }
}

/// Full user data for cloud sync (upload/download)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncData {
    pub steam_id: String,
    pub games: Vec<Game>,
    pub achievements: Vec<SyncAchievement>,
    pub run_history: Vec<RunHistory>,
    pub achievement_history: Vec<AchievementHistory>,
    pub exported_at: DateTime<Utc>,
}

/// Cloud sync status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncStatus {
    pub has_data: bool,
    pub game_count: i32,
    pub achievement_count: i32,
    pub last_sync: Option<DateTime<Utc>>,
}
