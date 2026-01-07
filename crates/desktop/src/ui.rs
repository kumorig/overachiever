use crate::steam_api::{FetchProgress, ScrapeProgress, UpdateProgress, SingleGameRefreshProgress};
use std::sync::mpsc::Receiver;
use overachiever_core::TtbTimes;

/// Duration for the flash animation in seconds
pub const FLASH_DURATION: f32 = 2.0;

#[derive(Clone, PartialEq)]
pub enum AppState {
    Idle,
    // Fetch states
    FetchRequesting,
    FetchDownloading,
    FetchProcessing,
    FetchSaving,
    // Scrape states
    Scraping { current: i32, total: i32 },
    // Update states
    UpdateFetchingGames,
    UpdateFetchingRecentlyPlayed,
    UpdateScraping { current: i32, total: i32 },
    // TTB scan states
    TtbScanning { current: i32, total: i32 },
    // Tags scan states
    TagsScanning { current: i32, total: i32 },
}

impl AppState {
    pub fn is_busy(&self) -> bool {
        match self {
            AppState::Idle => false,
            AppState::TtbScanning { .. } => false, // TTB scan runs in background, doesn't block
            AppState::TagsScanning { .. } => false, // Tags scan runs in background, doesn't block
            _ => true,
        }
    }

    pub fn progress(&self) -> f32 {
        match self {
            AppState::Idle => 0.0,
            AppState::FetchRequesting => 0.25,
            AppState::FetchDownloading => 0.50,
            AppState::FetchProcessing => 0.75,
            AppState::FetchSaving => 0.90,
            AppState::Scraping { current, total } => {
                if *total > 0 { *current as f32 / *total as f32 } else { 0.0 }
            }
            AppState::UpdateFetchingGames => 0.10,
            AppState::UpdateFetchingRecentlyPlayed => 0.20,
            AppState::UpdateScraping { current, total } => {
                if *total > 0 { 0.20 + 0.80 * (*current as f32 / *total as f32) } else { 0.20 }
            }
            AppState::TtbScanning { current, total } => {
                if *total > 0 { *current as f32 / *total as f32 } else { 0.0 }
            }
            AppState::TagsScanning { current, total } => {
                if *total > 0 { *current as f32 / *total as f32 } else { 0.0 }
            }
        }
    }
}

// Re-export shared types from core
pub use overachiever_core::{SortColumn, SortOrder, TriFilter};

/// Progress messages for TTB scan (reserved for future async implementation)
#[allow(dead_code)]
pub enum TtbProgress {
    Starting { total: i32 },
    Fetching { current: i32, total: i32, game_name: String },
    GameDone { appid: u64, times: Option<TtbTimes> },
    Done,
    Error(String),
}

#[allow(dead_code)]
pub enum ProgressReceiver {
    Fetch(Receiver<FetchProgress>),
    Scrape(Receiver<ScrapeProgress>),
    Update(Receiver<UpdateProgress>),
    SingleGameRefresh(Receiver<SingleGameRefreshProgress>),
    TtbScan(Receiver<TtbProgress>),
}
