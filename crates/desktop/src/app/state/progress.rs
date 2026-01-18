//! Progress tracking and background operations

use crate::db::{
    backfill_run_history_unplayed, get_achievement_history, get_last_update, get_log_entries,
    get_run_history, insert_achievement_history, open_connection, update_latest_run_history_unplayed,
};
use crate::steam_api::{FetchProgress, ScrapeProgress, UpdateProgress};
use crate::ui::{AppState, ProgressReceiver, FLASH_DURATION};

use std::sync::mpsc::{channel, Sender};
use std::thread;

use crate::app::SteamOverachieverApp;

impl SteamOverachieverApp {
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
    
    /// Start a single game refresh
    pub(crate) fn start_single_game_refresh(&mut self, appid: u64) -> bool {
        if self.state.is_busy() || self.single_game_refreshing.is_some() {
            return false;
        }
        
        self.single_game_refreshing = Some(appid);
        self.state = AppState::Idle; // Keep idle state but track the refresh separately
        self.status = format!("Refreshing game {}...", appid);
        
        let (tx, rx): (Sender<crate::steam_api::SingleGameRefreshProgress>, _) = channel();
        self.receiver = Some(ProgressReceiver::SingleGameRefresh(rx));
        
        thread::spawn(move || {
            if let Err(e) = crate::steam_api::refresh_single_game(tx.clone(), appid) {
                let _ = tx.send(crate::steam_api::SingleGameRefreshProgress::Error(e.to_string()));
            }
        });
        
        true
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
                            self.status = format!("Fetching from Steam Api: 0 / {} games...", total);
                        }
                        ScrapeProgress::Scraping { current, total, game_name } => {
                            self.state = AppState::Scraping { current, total };
                            self.status = format!("Fetching from Steam Api: {} / {}: {}", current, total, game_name);
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
                            
                            // Refresh installed games detection
                            self.refresh_installed_games();
                            
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
                            
                            // Refresh installed games detection
                            self.refresh_installed_games();
                            
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
            ProgressReceiver::SingleGameRefresh(rx) => {
                while let Ok(progress) = rx.try_recv() {
                    match progress {
                        crate::steam_api::SingleGameRefreshProgress::Refreshing { appid } => {
                            self.status = format!("Refreshing game {}...", appid);
                        }
                        crate::steam_api::SingleGameRefreshProgress::Done { appid, game, achievements } => {
                            // Update the game in our list
                            if let Some(g) = self.games.iter_mut().find(|g| g.appid == appid) {
                                *g = game;
                            }
                            // Update achievements cache
                            self.achievements_cache.insert(appid, achievements);
                            // Track this game for flash animation
                            self.updated_games.insert(appid, std::time::Instant::now());
                            // Re-sort to place updated row in correct position
                            self.sort_games();
                            self.single_game_refreshing = None;
                            self.status = "Refresh complete!".to_string();
                            self.state = AppState::Idle;
                            return;
                        }
                        crate::steam_api::SingleGameRefreshProgress::Error(e) => {
                            self.single_game_refreshing = None;
                            self.status = format!("Refresh error: {}", e);
                            self.state = AppState::Idle;
                            return;
                        }
                    }
                }
                self.receiver = Some(ProgressReceiver::SingleGameRefresh(rx));
            }
            ProgressReceiver::TtbScan(_rx) => {
                // TTB scan uses direct tick-based polling instead of channel-based progress
                // This arm exists for exhaustiveness but won't be used
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
    
    /// Clean up expired game launch cooldowns (7 seconds)
    pub(crate) fn cleanup_expired_launch_cooldowns(&mut self) {
        const LAUNCH_COOLDOWN_SECS: f32 = 7.0;
        self.game_launch_times.retain(|_, launch_time| {
            launch_time.elapsed().as_secs_f32() < LAUNCH_COOLDOWN_SECS
        });
    }
    
    /// Check if a game is in launch cooldown (returns remaining fraction 0.0-1.0)
    pub(crate) fn get_launch_cooldown(&self, appid: u64) -> Option<f32> {
        const LAUNCH_COOLDOWN_SECS: f32 = 7.0;
        self.game_launch_times.get(&appid).and_then(|launch_time| {
            let elapsed = launch_time.elapsed().as_secs_f32();
            if elapsed < LAUNCH_COOLDOWN_SECS {
                Some(1.0 - (elapsed / LAUNCH_COOLDOWN_SECS))
            } else {
                None
            }
        })
    }
    
    /// Refresh the list of installed Steam games
    pub(crate) fn refresh_installed_games(&mut self) {
        self.installed_games = crate::steam_library::get_installed_games();
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
        
        // Count unplayed games WITH achievements (playtime == 0)
        let unplayed_with_ach = games_with_ach.iter()
            .filter(|g| g.playtime_forever == 0)
            .count() as i32;
        
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
            // Update the unplayed count in the most recent run_history entry
            let _ = update_latest_run_history_unplayed(&conn, &self.config.steam_id, unplayed_with_ach);
            
            // Backfill historical entries that have 0 unplayed (from before this feature)
            let _ = backfill_run_history_unplayed(&conn, &self.config.steam_id, unplayed_with_ach);
            
            let _ = insert_achievement_history(
                &conn,
                &self.config.steam_id,
                total_achievements,
                unlocked_achievements,
                games_with_ach.len() as i32,
                avg_completion,
            );
            self.run_history = get_run_history(&conn, &self.config.steam_id).unwrap_or_default();
            self.achievement_history = get_achievement_history(&conn, &self.config.steam_id).unwrap_or_default();
            self.log_entries = get_log_entries(&conn, &self.config.steam_id, 30).unwrap_or_default();
        }
    }
}
