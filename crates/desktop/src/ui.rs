use crate::steam_api::{FetchProgress, ScrapeProgress, UpdateProgress};
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

#[derive(Clone, Copy, PartialEq)]
pub enum SortColumn {
    Name,
    LastPlayed,
    Playtime,
    AchievementsTotal,
    AchievementsPercent,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn toggle(&self) -> Self {
        match self {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        }
    }
}

/// Tri-state filter: All, Only With, Only Without
#[derive(Clone, Copy, PartialEq, Default)]
pub enum TriFilter {
    #[default]
    All,
    With,
    Without,
}

impl TriFilter {
    pub fn cycle(&self) -> Self {
        match self {
            TriFilter::All => TriFilter::With,
            TriFilter::With => TriFilter::Without,
            TriFilter::Without => TriFilter::All,
        }
    }
    
    pub fn label(&self, with_text: &str, without_text: &str) -> String {
        match self {
            TriFilter::All => "All".to_string(),
            TriFilter::With => with_text.to_string(),
            TriFilter::Without => without_text.to_string(),
        }
    }
}

#[allow(dead_code)]
pub enum ProgressReceiver {
    Fetch(Receiver<FetchProgress>),
    Scrape(Receiver<ScrapeProgress>),
    Update(Receiver<UpdateProgress>),
}
