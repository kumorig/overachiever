use serde::{Deserialize, Serialize};

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

/// Game with our tracked data
#[derive(Debug, Clone)]
pub struct Game {
    pub appid: u64,
    pub name: String,
    pub playtime_forever: u32,
    pub rtime_last_played: Option<u32>,
    pub img_icon_url: Option<String>,
    #[allow(dead_code)]
    pub added_at: chrono::DateTime<chrono::Utc>,
    pub achievements_total: Option<i32>,
    pub achievements_unlocked: Option<i32>,
    pub last_achievement_scrape: Option<chrono::DateTime<chrono::Utc>>,
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
            (Some(unlocked), Some(total)) if total > 0 => Some(unlocked as f32 / total as f32 * 100.0),
            _ => None,
        }
    }
}

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

/// Achievement stored in local database with display info
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GameAchievement {
    pub appid: u64,
    pub apiname: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: String,
    pub icon_gray: String,
    pub achieved: bool,
    pub unlocktime: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct RunHistory {
    #[allow(dead_code)]
    pub id: i64,
    pub run_at: chrono::DateTime<chrono::Utc>,
    pub total_games: i32,
}

/// History of achievement progress over time
#[derive(Debug, Clone)]
pub struct AchievementHistory {
    #[allow(dead_code)]
    pub id: i64,
    #[allow(dead_code)]
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    pub total_achievements: i32,
    pub unlocked_achievements: i32,
    #[allow(dead_code)]
    pub games_with_achievements: i32,
    pub avg_completion_percent: f32,
}
