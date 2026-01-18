//! Shared data models used across all platforms

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Description of how the desktop app handles data
pub const DATA_HANDLING_DESCRIPTION: &str = "\
• Your game data is stored locally on your computer\n\
• Uses Steam API to fetch your games and achievements\n\
• Uses overachiever.space to post/fetch community difficulty ratings and comments";

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
    
    // User-reported TTB data (averaged from community)
    pub avg_user_ttb_main_seconds: Option<i32>,
    pub avg_user_ttb_extra_seconds: Option<i32>,
    pub avg_user_ttb_completionist_seconds: Option<i32>,
    pub user_ttb_report_count: i32,
    
    // Current user's own TTB report (WASM/Remote) or single user's report (Desktop)
    pub my_ttb_main_seconds: Option<i32>,
    pub my_ttb_extra_seconds: Option<i32>,
    pub my_ttb_completionist_seconds: Option<i32>,
    pub my_ttb_reported_at: Option<DateTime<Utc>>,
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
    /// Whether this achievement marks game completion for the user
    #[serde(default)]
    pub is_game_finishing: bool,
}

/// Run history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunHistory {
    pub id: i64,
    pub run_at: DateTime<Utc>,
    pub total_games: i32,
    /// Unplayed games with achievements
    pub unplayed_games: i32,
    /// Unplayed games total (regardless of achievements)
    #[serde(default)]
    pub unplayed_games_total: i32,
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
    /// Short ID for shareable profile URLs (YouTube-style)
    pub short_id: Option<String>,
}

/// Sync result after updating from Steam
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub games_updated: i32,
    pub achievements_updated: i32,
    pub new_games: i32,
}

// ============================================================================
// Cloud Sync Types
// ============================================================================

/// Cloud sync status for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncStatus {
    pub has_data: bool,
    pub game_count: i32,
    pub achievement_count: i32,
    pub last_sync: Option<DateTime<Utc>>,
}

/// Lightweight achievement data for cloud sync (no icons/descriptions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAchievement {
    pub appid: u64,
    pub apiname: String,
    pub achieved: bool,
    pub unlocktime: Option<DateTime<Utc>>,
}

/// Full cloud sync data bundle for upload/download
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncData {
    pub steam_id: String,
    pub games: Vec<Game>,
    pub achievements: Vec<SyncAchievement>,
    pub run_history: Vec<RunHistory>,
    pub achievement_history: Vec<AchievementHistory>,
    pub exported_at: DateTime<Utc>,
}

// ============================================================================
// Time To Beat (HLTB) Data
// ============================================================================

/// Time to beat data from HowLongToBeat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtbTimes {
    pub appid: u64,
    /// Main story completion time in hours
    pub main: Option<f32>,
    /// Main + extras completion time in hours
    pub main_extra: Option<f32>,
    /// Completionist time in hours
    pub completionist: Option<f32>,
    /// When this data was last updated
    pub updated_at: DateTime<Utc>,
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
// Game Tags (from SteamSpy)
// ============================================================================

/// A tag for a game with vote count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTag {
    pub appid: u64,
    pub tag_name: String,
    pub vote_count: u32,
}
