use crate::steam_api::{FetchProgress, ScrapeProgress, UpdateProgress, SingleGameRefreshProgress};
use std::sync::mpsc::Receiver;

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
}

impl AppState {
    pub fn is_busy(&self) -> bool {
        !matches!(self, AppState::Idle)
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
        }
    }
}

// Re-export shared types from core
pub use overachiever_core::{SortColumn, SortOrder, TriFilter};

#[allow(dead_code)]
pub enum ProgressReceiver {
    Fetch(Receiver<FetchProgress>),
    Scrape(Receiver<ScrapeProgress>),
    Update(Receiver<UpdateProgress>),
    SingleGameRefresh(Receiver<SingleGameRefreshProgress>),
}
