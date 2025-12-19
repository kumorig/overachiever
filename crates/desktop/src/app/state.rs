//! App state management - sorting, progress handling, and background operations

use crate::db::{get_run_history, get_achievement_history, get_log_entries, insert_achievement_history, open_connection, get_last_update};
use crate::steam_api::{FetchProgress, ScrapeProgress, UpdateProgress};
use crate::ui::{AppState, SortColumn, SortOrder, ProgressReceiver, FLASH_DURATION};

use egui_phosphor::regular;
use std::sync::mpsc::{channel, Sender};
use std::thread;

use super::SteamOverachieverApp;

impl SteamOverachieverApp {
    pub(crate) fn sort_games(&mut self) {
        let order = self.sort_order;
        match self.sort_column {
            SortColumn::Name => {
                self.games.sort_by(|a, b| {
                    let cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::LastPlayed => {
                self.games.sort_by(|a, b| {
                    let cmp = a.rtime_last_played.unwrap_or(0).cmp(&b.rtime_last_played.unwrap_or(0));
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::Playtime => {
                self.games.sort_by(|a, b| {
                    let cmp = a.playtime_forever.cmp(&b.playtime_forever);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::AchievementsTotal => {
                self.games.sort_by(|a, b| {
                    let a_total = a.achievements_total.unwrap_or(-1);
                    let b_total = b.achievements_total.unwrap_or(-1);
                    let cmp = a_total.cmp(&b_total);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::AchievementsPercent => {
                self.games.sort_by(|a, b| {
                    let a_pct = a.completion_percent().unwrap_or(-1.0);
                    let b_pct = b.completion_percent().unwrap_or(-1.0);
                    let cmp = a_pct.partial_cmp(&b_pct).unwrap_or(std::cmp::Ordering::Equal);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
        }
    }
    
    pub(crate) fn set_sort(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.sort_order = self.sort_order.toggle();
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Ascending;
        }
        self.sort_games();
    }
    
    pub(crate) fn sort_indicator(&self, column: SortColumn) -> String {
        if self.sort_column == column {
            match self.sort_order {
                SortOrder::Ascending => format!(" {}", regular::SORT_ASCENDING),
                SortOrder::Descending => format!(" {}", regular::SORT_DESCENDING),
            }
        } else {
            String::new()
        }
    }
    
    #[allow(dead_code)]
    pub(crate) fn start_fetch(&mut self) {
        if self.state.is_busy() {
            return;
        }
        
        self.state = AppState::FetchRequesting;
        self.status = "Starting fetch...".to_string();
        
        let (tx, rx): (Sender<FetchProgress>, _) = channel();
        self.receiver = Some(ProgressReceiver::Fetch(rx));
        
        thread::spawn(move || {
            if let Err(e) = crate::steam_api::fetch_owned_games_with_progress(tx.clone()) {
                let _ = tx.send(FetchProgress::Error(e.to_string()));
            }
        });
    }
    
    pub(crate) fn start_scrape(&mut self) {
        if self.state.is_busy() {
            return;
        }
        
        self.state = AppState::Scraping { current: 0, total: 0 };
        self.status = "Starting achievement scrape...".to_string();
        
        let force = self.force_full_scan;
        let (tx, rx): (Sender<ScrapeProgress>, _) = channel();
        self.receiver = Some(ProgressReceiver::Scrape(rx));
        
        thread::spawn(move || {
            if let Err(e) = crate::steam_api::scrape_achievements_with_progress(tx.clone(), force) {
                let _ = tx.send(ScrapeProgress::Error(e.to_string()));
            }
        });
    }
    
    pub(crate) fn start_update(&mut self) {
        if self.state.is_busy() {
            return;
        }
        
        self.state = AppState::UpdateFetchingGames;
        self.status = "Starting update...".to_string();
        
        let (tx, rx): (Sender<UpdateProgress>, _) = channel();
        self.receiver = Some(ProgressReceiver::Update(rx));
        
        thread::spawn(move || {
            if let Err(e) = crate::steam_api::run_update_with_progress(tx.clone()) {
                let _ = tx.send(UpdateProgress::Error(e.to_string()));
            }
        });
    }
    
    /// Check if the last update was more than 2 weeks ago
    pub(crate) fn is_update_stale(&self) -> bool {
        match self.last_update_time {
            Some(last_update) => {
                let two_weeks_ago = chrono::Utc::now() - chrono::Duration::weeks(2);
                last_update < two_weeks_ago
            }
            None => true, // Never updated, consider it stale
        }
    }
    
    pub(crate) fn check_progress(&mut self) {
        let receiver = match self.receiver.take() {
            Some(r) => r,
            None => return,
        };
        
        match receiver {
            ProgressReceiver::Fetch(rx) => {
                while let Ok(progress) = rx.try_recv() {
                    match progress {
                        FetchProgress::Requesting => {
                            self.state = AppState::FetchRequesting;
                            self.status = "Requesting...".to_string();
                        }
                        FetchProgress::Downloading => {
                            self.state = AppState::FetchDownloading;
                            self.status = "Downloading...".to_string();
                        }
                        FetchProgress::Processing => {
                            self.state = AppState::FetchProcessing;
                            self.status = "Processing...".to_string();
                        }
                        FetchProgress::Saving => {
                            self.state = AppState::FetchSaving;
                            self.status = "Saving to database...".to_string();
                        }
                        FetchProgress::Done { games, total } => {
                            self.games = games;
                            self.sort_games();
                            if let Ok(conn) = open_connection() {
                                self.run_history = get_run_history(&conn, &self.config.steam_id).unwrap_or_default();
                            }
                            self.status = format!("Fetched {} games!", total);
                            self.state = AppState::Idle;
                            return;
                        }
                        FetchProgress::Error(e) => {
                            self.status = format!("Error: {}", e);
                            self.state = AppState::Idle;
                            return;
                        }
                    }
                }
                self.receiver = Some(ProgressReceiver::Fetch(rx));
            }
            ProgressReceiver::Scrape(rx) => {
                while let Ok(progress) = rx.try_recv() {
                    match progress {
                        ScrapeProgress::FetchingGames => {
                            self.state = AppState::FetchRequesting;
                            self.status = "Fetching games...".to_string();
                        }
                        ScrapeProgress::Starting { total } => {
                            self.state = AppState::Scraping { current: 0, total };
                            self.status = format!("Scraping 0 / {} games...", total);
                        }
                        ScrapeProgress::Scraping { current, total, game_name } => {
                            self.state = AppState::Scraping { current, total };
                            self.status = format!("Scraping {} / {}: {}", current, total, game_name);
                        }
                        ScrapeProgress::GameUpdated { appid, unlocked, total } => {
                            // Update the game in our list immediately
                            if let Some(game) = self.games.iter_mut().find(|g| g.appid == appid) {
                                game.achievements_unlocked = Some(unlocked);
                                game.achievements_total = Some(total);
                                game.last_achievement_scrape = Some(chrono::Utc::now());
                            }
                            // Track this game for flash animation
                            self.updated_games.insert(appid, std::time::Instant::now());
                            // Re-sort to place updated row in correct position
                            self.sort_games();
                        }
                        ScrapeProgress::Done { games } => {
                            self.games = games;
                            self.sort_games();
                            
                            // Reload run history since we fetched games as well
                            if let Ok(conn) = open_connection() {
                                self.run_history = get_run_history(&conn, &self.config.steam_id).unwrap_or_default();
                            }
                            
                            // Calculate and save achievement stats
                            self.save_achievement_history();
                            
                            self.status = "Full scan complete!".to_string();
                            self.state = AppState::Idle;
                            return;
                        }
                        ScrapeProgress::Error(e) => {
                            self.status = format!("Error: {}", e);
                            self.state = AppState::Idle;
                            return;
                        }
                    }
                }
                self.receiver = Some(ProgressReceiver::Scrape(rx));
            }
            ProgressReceiver::Update(rx) => {
                while let Ok(progress) = rx.try_recv() {
                    match progress {
                        UpdateProgress::FetchingGames => {
                            self.state = AppState::UpdateFetchingGames;
                            self.status = "Fetching games...".to_string();
                        }
                        UpdateProgress::FetchingRecentlyPlayed => {
                            self.state = AppState::UpdateFetchingRecentlyPlayed;
                            self.status = "Fetching recently played games...".to_string();
                        }
                        UpdateProgress::ScrapingAchievements { current, total, game_name } => {
                            self.state = AppState::UpdateScraping { current, total };
                            self.status = format!("Updating {} / {}: {}", current, total, game_name);
                        }
                        UpdateProgress::GameUpdated { appid, unlocked, total } => {
                            // Update the game in our list immediately
                            if let Some(game) = self.games.iter_mut().find(|g| g.appid == appid) {
                                game.achievements_unlocked = Some(unlocked);
                                game.achievements_total = Some(total);
                                game.last_achievement_scrape = Some(chrono::Utc::now());
                            }
                            // Track this game for flash animation
                            self.updated_games.insert(appid, std::time::Instant::now());
                            // Re-sort to place updated row in correct position
                            self.sort_games();
                        }
                        UpdateProgress::Done { games, updated_count } => {
                            self.games = games;
                            self.sort_games();
                            
                            // Reload run history
                            if let Ok(conn) = open_connection() {
                                self.run_history = get_run_history(&conn, &self.config.steam_id).unwrap_or_default();
                                self.last_update_time = get_last_update(&conn).unwrap_or(None);
                            }
                            
                            // Calculate and save achievement stats
                            self.save_achievement_history();
                            
                            self.status = format!("Update complete! {} games updated.", updated_count);
                            self.state = AppState::Idle;
                            return;
                        }
                        UpdateProgress::Error(e) => {
                            self.status = format!("Error: {}", e);
                            self.state = AppState::Idle;
                            return;
                        }
                    }
                }
                self.receiver = Some(ProgressReceiver::Update(rx));
            }
        }
    }
    
    pub(crate) fn games_needing_scrape(&self) -> usize {
        self.games.iter().filter(|g| g.last_achievement_scrape.is_none()).count()
    }
    
    /// Returns the flash intensity (0.0 to 1.0) for a game, or None if not flashing
    pub(crate) fn get_flash_intensity(&self, appid: u64) -> Option<f32> {
        if let Some(update_time) = self.updated_games.get(&appid) {
            let elapsed = update_time.elapsed().as_secs_f32();
            if elapsed < FLASH_DURATION {
                // Fade from 1.0 to 0.0 over FLASH_DURATION seconds
                Some(1.0 - (elapsed / FLASH_DURATION))
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Clean up expired flash entries
    pub(crate) fn cleanup_expired_flashes(&mut self) {
        self.updated_games.retain(|_, update_time| {
            update_time.elapsed().as_secs_f32() < FLASH_DURATION
        });
    }
    
    /// Calculate and save achievement statistics to history
    pub(crate) fn save_achievement_history(&mut self) {
        // Calculate stats from games with achievements
        let games_with_ach: Vec<_> = self.games.iter()
            .filter(|g| g.achievements_total.map(|t| t > 0).unwrap_or(false))
            .collect();
        
        if games_with_ach.is_empty() {
            return;
        }
        
        let total_achievements: i32 = games_with_ach.iter()
            .filter_map(|g| g.achievements_total)
            .sum();
        
        let unlocked_achievements: i32 = games_with_ach.iter()
            .filter_map(|g| g.achievements_unlocked)
            .sum();
        
        // Only count played games (playtime > 0) for avg completion
        let completion_percents: Vec<f32> = games_with_ach.iter()
            .filter(|g| g.playtime_forever > 0)
            .filter_map(|g| g.completion_percent())
            .collect();
        
        let avg_completion: f32 = if completion_percents.is_empty() {
            0.0
        } else {
            completion_percents.iter().sum::<f32>() / completion_percents.len() as f32
        };
        
        if let Ok(conn) = open_connection() {
            let _ = insert_achievement_history(
                &conn,
                &self.config.steam_id,
                total_achievements,
                unlocked_achievements,
                games_with_ach.len() as i32,
                avg_completion,
            );
            self.achievement_history = get_achievement_history(&conn, &self.config.steam_id).unwrap_or_default();
            self.log_entries = get_log_entries(&conn, &self.config.steam_id, 30).unwrap_or_default();
        }
    }
}
