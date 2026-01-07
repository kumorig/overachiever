//! App state management - sorting, progress handling, and background operations

use crate::db::{get_run_history, get_achievement_history, get_log_entries, insert_achievement_history, open_connection, get_last_update, update_latest_run_history_unplayed, backfill_run_history_unplayed, get_games_without_ttb, cache_ttb_times, get_cached_ttb};
use crate::steam_api::{FetchProgress, ScrapeProgress, UpdateProgress};
use crate::ui::{AppState, SortColumn, SortOrder, ProgressReceiver, FLASH_DURATION};
use crate::ttb;

use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::{Duration, Instant};
use std::io::Write;

use super::SteamOverachieverApp;

/// Log a message to log.txt for debugging
fn ttb_log(msg: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("ttb_log.txt")
    {
        let _ = writeln!(file, "[{}] {}", chrono::Local::now().format("%H:%M:%S"), msg);
    }
}

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
            SortColumn::TimeToBeat => {
                let cache = &self.ttb_cache;
                self.games.sort_by(|a, b| {
                    let a_ttb = cache.get(&a.appid).and_then(|t| t.main).unwrap_or(-1.0);
                    let b_ttb = cache.get(&b.appid).and_then(|t| t.main).unwrap_or(-1.0);
                    let cmp = a_ttb.partial_cmp(&b_ttb).unwrap_or(std::cmp::Ordering::Equal);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::Votes => {
                // Votes sorting is handled in set_sort in games_table.rs (needs filter_tags context)
                // This is just for the initial sort_games call which won't use Votes
            }
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
    
    // ---- Cloud sync methods ----
    
    /// Start the Steam login flow to link to cloud
    pub(crate) fn start_cloud_link(&mut self) {
        use crate::cloud_sync::{CloudSyncState, start_steam_login};
        
        self.cloud_sync_state = CloudSyncState::Linking;
        
        match start_steam_login() {
            Ok(receiver) => {
                self.auth_receiver = Some(receiver);
            }
            Err(e) => {
                self.cloud_sync_state = CloudSyncState::Error(e);
            }
        }
    }
    
    /// Check for auth callback result (called from update loop)
    pub(crate) fn check_auth_callback(&mut self) {
        use crate::cloud_sync::CloudSyncState;
        
        if let Some(ref receiver) = self.auth_receiver {
            match receiver.try_recv() {
                Ok(Ok(result)) => {
                    // Save token to config
                    self.config.cloud_token = Some(result.token);
                    let _ = self.config.save();
                    self.cloud_sync_state = CloudSyncState::Success("Linked to cloud successfully!".to_string());
                    self.auth_receiver = None;
                }
                Ok(Err(e)) => {
                    self.cloud_sync_state = CloudSyncState::Error(e);
                    self.auth_receiver = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Still waiting
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.cloud_sync_state = CloudSyncState::Error("Login cancelled".to_string());
                    self.auth_receiver = None;
                }
            }
        }
    }
    
    /// Unlink from cloud (remove saved token)
    pub(crate) fn unlink_cloud(&mut self) {
        use crate::cloud_sync::CloudSyncState;
        
        self.config.cloud_token = None;
        let _ = self.config.save();
        self.cloud_status = None;
        self.cloud_sync_state = CloudSyncState::NotLinked;
    }
    
    /// Check for completed cloud operation results
    pub(crate) fn check_cloud_operation(&mut self) {
        use crate::cloud_sync::{CloudSyncState, CloudOpResult};
        use crate::db::import_cloud_sync_data;
        
        if let Some(ref receiver) = self.cloud_op_receiver {
            match receiver.try_recv() {
                Ok(Ok(result)) => {
                    match result {
                        CloudOpResult::UploadProgress(progress) => {
                            // Update progress state, don't clear receiver - more messages coming
                            self.cloud_sync_state = CloudSyncState::Uploading(progress);
                            return;
                        }
                        CloudOpResult::UploadSuccess => {
                            self.cloud_sync_state = CloudSyncState::Success("Data uploaded successfully!".to_string());
                            // Start async status refresh
                            if let Some(token) = &self.config.cloud_token {
                                self.cloud_op_receiver = Some(crate::cloud_sync::start_status_check(token.clone()));
                                return; // Don't clear receiver yet
                            }
                        }
                        CloudOpResult::DownloadSuccess(data) => {
                            // Import into local database
                            let conn = match open_connection() {
                                Ok(c) => c,
                                Err(e) => {
                                    self.cloud_sync_state = CloudSyncState::Error(format!("Failed to open database: {}", e));
                                    self.cloud_op_receiver = None;
                                    return;
                                }
                            };
                            
                            // Update steam_id from downloaded data if different
                            let steam_id = data.steam_id.clone();
                            if self.config.steam_id != steam_id {
                                self.config.steam_id = steam_id.clone();
                                let _ = self.config.save();
                            }
                            
                            let games_count = data.games.len();
                            let achievements_count = data.achievements.len();
                            
                            if let Err(e) = import_cloud_sync_data(&conn, &data) {
                                self.cloud_sync_state = CloudSyncState::Error(format!("Failed to import data: {}", e));
                                self.cloud_op_receiver = None;
                                return;
                            }
                            
                            // Reload data from database
                            self.games = crate::db::get_all_games(&conn, &steam_id).unwrap_or_default();
                            self.run_history = get_run_history(&conn, &steam_id).unwrap_or_default();
                            self.achievement_history = get_achievement_history(&conn, &steam_id).unwrap_or_default();
                            self.log_entries = get_log_entries(&conn, &steam_id, 30).unwrap_or_default();
                            
                            self.sort_games();
                            
                            self.cloud_sync_state = CloudSyncState::Success(format!(
                                "Downloaded {} games, {} achievements!", 
                                games_count, 
                                achievements_count
                            ));
                        }
                        CloudOpResult::DeleteSuccess => {
                            self.cloud_status = None;
                            self.cloud_sync_state = CloudSyncState::Success("Cloud data deleted successfully!".to_string());
                        }
                        CloudOpResult::StatusChecked(status) => {
                            self.cloud_status = Some(status);
                            // Only go to Idle if we weren't showing a success message
                            if !matches!(self.cloud_sync_state, CloudSyncState::Success(_)) {
                                self.cloud_sync_state = CloudSyncState::Idle;
                            }
                        }
                    }
                    self.cloud_op_receiver = None;
                }
                Ok(Err(e)) => {
                    // If 401, token expired - need to re-link
                    if e.contains("401") {
                        self.config.cloud_token = None;
                        let _ = self.config.save();
                        self.cloud_sync_state = CloudSyncState::NotLinked;
                    } else {
                        self.cloud_sync_state = CloudSyncState::Error(e);
                    }
                    self.cloud_op_receiver = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Still waiting
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.cloud_sync_state = CloudSyncState::Error("Operation failed unexpectedly".to_string());
                    self.cloud_op_receiver = None;
                }
            }
        }
    }
    
    #[allow(dead_code)]
    pub(crate) fn check_cloud_status(&mut self) {
        use crate::cloud_sync::CloudSyncState;
        
        let token = match &self.config.cloud_token {
            Some(t) => t.clone(),
            None => {
                self.cloud_sync_state = CloudSyncState::NotLinked;
                return;
            }
        };
        
        self.cloud_sync_state = CloudSyncState::Checking;
        self.cloud_op_receiver = Some(crate::cloud_sync::start_status_check(token));
    }
    
    pub(crate) fn upload_to_cloud(&mut self) {
        use crate::cloud_sync::CloudSyncState;
        use crate::db::get_all_achievements_for_export;
        use crate::steam_library::get_installed_games_with_sizes;
        use overachiever_core::CloudSyncData;
        
        let token = match &self.config.cloud_token {
            Some(t) => t.clone(),
            None => {
                self.cloud_sync_state = CloudSyncState::NotLinked;
                return;
            }
        };
        
        let steam_id = self.config.steam_id.clone();
        
        self.cloud_sync_state = CloudSyncState::Uploading(crate::cloud_sync::UploadProgress::default());
        
        // Gather data from local database (this is fast, so we do it synchronously)
        let conn = match open_connection() {
            Ok(c) => c,
            Err(e) => {
                self.cloud_sync_state = CloudSyncState::Error(format!("Failed to open database: {}", e));
                return;
            }
        };
        
        let achievements = match get_all_achievements_for_export(&conn, &steam_id) {
            Ok(a) => a,
            Err(e) => {
                self.cloud_sync_state = CloudSyncState::Error(format!("Failed to get achievements: {}", e));
                return;
            }
        };
        
        let data = CloudSyncData {
            steam_id: steam_id.clone(),
            games: self.games.clone(),
            achievements,
            run_history: self.run_history.clone(),
            achievement_history: self.achievement_history.clone(),
            exported_at: chrono::Utc::now(),
        };
        
        // Collect install sizes from ACF files (for community database)
        let install_sizes: Vec<(u64, u64)> = get_installed_games_with_sizes()
            .into_iter()
            .filter_map(|info| info.size_on_disk.map(|size| (info.appid, size)))
            .collect();
        
        // Start async upload (includes size submission)
        self.cloud_op_receiver = Some(crate::cloud_sync::start_upload_with_sizes(token, data, install_sizes));
    }
    
    pub(crate) fn download_from_cloud(&mut self) {
        use crate::cloud_sync::CloudSyncState;
        
        let token = match &self.config.cloud_token {
            Some(t) => t.clone(),
            None => {
                self.cloud_sync_state = CloudSyncState::NotLinked;
                return;
            }
        };
        
        self.cloud_sync_state = CloudSyncState::Downloading;
        
        // Start async download
        self.cloud_op_receiver = Some(crate::cloud_sync::start_download(token));
    }
    
    pub(crate) fn delete_from_cloud(&mut self) {
        use crate::cloud_sync::CloudSyncState;

        let token = match &self.config.cloud_token {
            Some(t) => t.clone(),
            None => {
                self.cloud_sync_state = CloudSyncState::NotLinked;
                return;
            }
        };

        self.cloud_sync_state = CloudSyncState::Deleting;

        // Start async delete
        self.cloud_op_receiver = Some(crate::cloud_sync::start_delete(token));
    }

    // ============================================================================
    // TTB (Time To Beat) Scan Functions
    // ============================================================================

    /// Count games that don't have TTB data cached (for admin mode scan button)
    /// Excludes games in the TTB blacklist
    pub(crate) fn games_needing_ttb_admin(&self) -> usize {
        self.games.iter()
            .filter(|g| !self.ttb_cache.contains_key(&g.appid))
            .filter(|g| !self.ttb_blacklist.contains(&g.appid))
            .count()
    }

    /// Start TTB scan for all games without TTB data
    /// Excludes games in the TTB blacklist
    pub(crate) fn start_ttb_scan(&mut self) {
        if !self.ttb_scan_queue.is_empty() {
            return;
        }

        // Get games without TTB from database, filtering out blacklisted games
        if let Ok(conn) = open_connection() {
            if let Ok(games) = get_games_without_ttb(&conn, &self.config.steam_id) {
                // Filter out blacklisted games
                self.ttb_scan_queue = games.into_iter()
                    .filter(|(appid, _)| !self.ttb_blacklist.contains(appid))
                    .collect();

                if !self.ttb_scan_queue.is_empty() {
                    let total = self.ttb_scan_queue.len() as i32;
                    self.state = AppState::TtbScanning { current: 0, total };
                    self.status = format!("TTB Scan: 0 / {} games", total);
                }
            }
        }
    }

    /// Stop the TTB scan
    pub(crate) fn stop_ttb_scan(&mut self) {
        self.ttb_scan_queue.clear();
        self.ttb_fetching = None;
        self.ttb_receiver = None;
        if matches!(self.state, AppState::TtbScanning { .. }) {
            self.state = AppState::Idle;
            self.status = "TTB scan cancelled".to_string();
        }
    }

    /// Process TTB scan queue (called each frame)
    pub(crate) fn ttb_scan_tick(&mut self) {
        // Track if we're in a batch scan (vs single fetch)
        let is_scanning = matches!(self.state, AppState::TtbScanning { .. });

        // First, check if we have a pending result from background fetch
        if let Some(ref receiver) = self.ttb_receiver {
            match receiver.try_recv() {
                Ok(Ok((appid, game_name, times))) => {
                    ttb_log(&format!("Fetched times for appid={}: main={:?}", appid, times.main));

                    // Cache locally
                    if let Ok(conn) = open_connection() {
                        let _ = cache_ttb_times(&conn, &times);
                    }
                    self.ttb_cache.insert(appid, times.clone());

                    // POST to backend (fire and forget)
                    if let Some(token) = &self.config.cloud_token {
                        ttb_log("Have cloud token, posting to backend...");
                        let token = token.clone();
                        let game_name_for_post = game_name.clone();
                        thread::spawn(move || {
                            let _ = post_ttb_to_backend(&token, appid, &game_name_for_post, &times);
                        });
                    } else {
                        ttb_log("No cloud token - skipping backend POST");
                    }

                    self.ttb_fetching = None;
                    self.ttb_receiver = None;

                    // Check if scan is complete (or single fetch finished)
                    if self.ttb_scan_queue.is_empty() {
                        self.state = AppState::Idle;
                        self.status = if is_scanning {
                            "TTB scan complete!".to_string()
                        } else {
                            format!("TTB loaded for {}", game_name)
                        };
                    }
                }
                Ok(Err(e)) => {
                    // Log error but continue scanning
                    ttb_log(&format!("TTB fetch failed: {}", e));
                    self.ttb_fetching = None;
                    self.ttb_receiver = None;

                    // Check if scan is complete
                    if self.ttb_scan_queue.is_empty() {
                        self.state = AppState::Idle;
                        self.status = if is_scanning {
                            "TTB scan complete!".to_string()
                        } else {
                            format!("TTB error: {}", e)
                        };
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Still waiting for result, keep receiver
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // Thread died unexpectedly
                    ttb_log("TTB fetch thread disconnected unexpectedly");
                    self.ttb_fetching = None;
                    self.ttb_receiver = None;
                }
            }
        }

        // If queue is empty, nothing to do
        if self.ttb_scan_queue.is_empty() {
            return;
        }

        // Check if we're already fetching (receiver is set)
        if self.ttb_receiver.is_some() {
            return;
        }

        // Check rate limit between fetches (configurable via ttb_scan_delay_secs)
        if let Some(last) = self.ttb_last_fetch {
            if last.elapsed() < Duration::from_secs(self.config.ttb_scan_delay_secs) {
                return;
            }
        }

        // Pop next game from queue and spawn background fetch
        if let Some((appid, game_name)) = self.ttb_scan_queue.pop() {
            self.ttb_fetching = Some(appid);
            self.ttb_last_fetch = Some(Instant::now());

            let total = self.ttb_scan_queue.len() as i32 + 1;
            let current = total - self.ttb_scan_queue.len() as i32;
            self.state = AppState::TtbScanning { current, total };
            self.status = format!("TTB: {} / {} - {}", current, total, game_name);

            // Spawn background thread for the fetch
            let (tx, rx) = channel();
            self.ttb_receiver = Some(rx);

            // Clean the game name for search (remove dashes, colons, apostrophe+s, etc.)
            let search_query = ttb::clean_game_name_for_search(&game_name);
            thread::spawn(move || {
                let result = ttb::fetch_ttb_times(appid, &search_query);
                let _ = tx.send(result.map(|times| (appid, game_name, times)).map_err(|e| e.to_string()));
            });
        }
    }

    /// Fetch TTB for a single game with a custom search query (async, non-blocking)
    pub(crate) fn fetch_single_ttb_with_query(&mut self, appid: u64, game_name: &str, search_query: &str) {
        // Don't start if already fetching
        if self.ttb_receiver.is_some() {
            return;
        }

        // Always allow fetching - user explicitly requested it via dialog
        ttb_log(&format!("Fetching single game: appid={}, name={}, query={}", appid, game_name, search_query));

        self.ttb_fetching = Some(appid);
        self.status = format!("Fetching TTB for {}...", game_name);

        // Spawn background thread for the fetch
        let (tx, rx) = channel();
        self.ttb_receiver = Some(rx);

        let game_name = game_name.to_string();
        let search_query = search_query.to_string();
        thread::spawn(move || {
            let result = ttb::fetch_ttb_times_with_query(appid, &search_query, &search_query);
            let _ = tx.send(result.map(|times| (appid, game_name, times)).map_err(|e| e.to_string()));
        });
    }

    /// Load TTB cache from local database on startup
    pub(crate) fn load_ttb_cache(&mut self) {
        if let Ok(conn) = open_connection() {
            for game in &self.games {
                if let Ok(Some(times)) = get_cached_ttb(&conn, game.appid) {
                    self.ttb_cache.insert(game.appid, times);
                }
            }
        }
    }

    /// Load TTB blacklist from backend on startup
    pub(crate) fn load_ttb_blacklist(&mut self) {
        match crate::cloud_sync::fetch_ttb_blacklist() {
            Ok(appids) => {
                self.ttb_blacklist = appids.into_iter().collect();
                ttb_log(&format!("Loaded {} games from TTB blacklist", self.ttb_blacklist.len()));
            }
            Err(e) => {
                ttb_log(&format!("Failed to load TTB blacklist: {}", e));
                // Don't fail startup, just use empty blacklist
            }
        }
    }

    /// Add a game to the TTB blacklist (admin only, posts to backend)
    pub(crate) fn add_to_ttb_blacklist(&mut self, appid: u64, game_name: &str) {
        if let Some(token) = &self.config.cloud_token {
            let token = token.clone();
            let game_name = game_name.to_string();
            let appid_copy = appid;

            // Fire and forget - add to local set immediately
            self.ttb_blacklist.insert(appid);

            std::thread::spawn(move || {
                if let Err(e) = crate::cloud_sync::add_to_ttb_blacklist(&token, appid_copy, &game_name, None) {
                    ttb_log(&format!("Failed to add {} to blacklist: {}", appid_copy, e));
                } else {
                    ttb_log(&format!("Added {} to TTB blacklist", appid_copy));
                }
            });
        }
    }

    /// Remove a game from the TTB blacklist (admin only, posts to backend)
    pub(crate) fn remove_from_ttb_blacklist(&mut self, appid: u64) {
        if let Some(token) = &self.config.cloud_token {
            let token = token.clone();

            // Fire and forget - remove from local set immediately
            self.ttb_blacklist.remove(&appid);

            std::thread::spawn(move || {
                if let Err(e) = crate::cloud_sync::remove_from_ttb_blacklist(&token, appid) {
                    ttb_log(&format!("Failed to remove {} from blacklist: {}", appid, e));
                } else {
                    ttb_log(&format!("Removed {} from TTB blacklist", appid));
                }
            });
        }
    }

    // ============================================================================
    // Tags (SteamSpy) Functions
    // ============================================================================

    /// Load available tags from backend on startup
    pub(crate) fn load_available_tags(&mut self) {
        match crate::cloud_sync::fetch_tag_names() {
            Ok(tags) => {
                self.available_tags = tags;
            }
            Err(e) => {
                eprintln!("Failed to load tag names: {}", e);
            }
        }
    }

    /// Load tags for all games from backend
    pub(crate) fn load_tags_for_games(&mut self) {
        let appids: Vec<u64> = self.games.iter().map(|g| g.appid).collect();
        if appids.is_empty() {
            return;
        }

        // Fetch in batches of 500
        for chunk in appids.chunks(500) {
            match crate::cloud_sync::fetch_tags_batch(chunk) {
                Ok(tags) => {
                    // Group tags by appid
                    for tag in tags {
                        self.tags_cache
                            .entry(tag.appid)
                            .or_insert_with(Vec::new)
                            .push((tag.tag_name, tag.vote_count));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load tags batch: {}", e);
                }
            }
        }
    }

    /// Process tag fetch queue (called each frame when admin mode is on)
    pub(crate) fn tags_fetch_tick(&mut self) {
        // Check if we have a pending result
        if let Some(ref receiver) = self.tags_receiver {
            match receiver.try_recv() {
                Ok(Ok((appid, tags))) => {
                    let is_scanning = matches!(self.state, AppState::TagsScanning { .. });

                    // Cache locally
                    self.tags_cache.insert(appid, tags.clone());

                    // Update available_tags with any new tags
                    for (tag_name, _) in &tags {
                        if !self.available_tags.contains(tag_name) {
                            self.available_tags.push(tag_name.clone());
                            self.available_tags.sort();
                        }
                    }

                    // POST to backend (fire and forget)
                    if let Some(token) = &self.config.cloud_token {
                        let token = token.clone();
                        let tags_for_post = tags.clone();
                        std::thread::spawn(move || {
                            let _ = crate::cloud_sync::submit_tags(&token, appid, &tags_for_post);
                        });
                    }

                    self.tags_fetching = None;
                    self.tags_receiver = None;

                    // Check if scan is complete
                    if self.tags_fetch_queue.is_empty() {
                        self.tags_scan_total = 0;
                        if is_scanning {
                            self.state = AppState::Idle;
                            self.status = "Tags scan complete!".to_string();
                        } else {
                            self.status = format!("Tags loaded for appid {}", appid);
                        }
                    }
                }
                Ok(Err(e)) => {
                    let is_scanning = matches!(self.state, AppState::TagsScanning { .. });
                    eprintln!("Tags fetch failed: {}", e);
                    self.tags_fetching = None;
                    self.tags_receiver = None;

                    // Check if scan is complete (even on error, continue)
                    if self.tags_fetch_queue.is_empty() {
                        self.tags_scan_total = 0;
                        if is_scanning {
                            self.state = AppState::Idle;
                            self.status = "Tags scan complete!".to_string();
                        } else {
                            self.status = format!("Tags error: {}", e);
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Still waiting
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.tags_fetching = None;
                    self.tags_receiver = None;
                }
            }
        }

        // If queue is empty, nothing to do
        if self.tags_fetch_queue.is_empty() {
            return;
        }

        // Check if already fetching
        if self.tags_receiver.is_some() {
            return;
        }

        // Check rate limit between fetches (configurable via tags_scan_delay_secs)
        if let Some(last) = self.tags_last_fetch {
            if last.elapsed() < Duration::from_secs(self.config.tags_scan_delay_secs) {
                return;
            }
        }

        // Pop next appid from queue and spawn background fetch
        if let Some(appid) = self.tags_fetch_queue.pop() {
            self.tags_fetching = Some(appid);
            self.tags_last_fetch = Some(Instant::now());

            // Update progress if in scan mode
            if let AppState::TagsScanning { current: _, total } = self.state {
                let new_current = total - self.tags_fetch_queue.len() as i32;
                self.state = AppState::TagsScanning { current: new_current, total };
                self.status = format!("Tags Scan: {} / {} games", new_current, total);
            } else {
                self.status = format!("Fetching tags for appid {}...", appid);
            }

            let (tx, rx) = channel();
            self.tags_receiver = Some(rx);

            thread::spawn(move || {
                let result = crate::steamspy::fetch_tags(appid);
                let _ = tx.send(result.map(|tags| (appid, tags)));
            });
        }
    }

    // ============================================================================
    // Tags Scan Functions (admin mode bulk fetch)
    // ============================================================================

    /// Count games that don't have tags cached (for admin mode scan button)
    pub(crate) fn games_needing_tags(&self) -> usize {
        self.games.iter()
            .filter(|g| !self.tags_cache.contains_key(&g.appid))
            .count()
    }

    /// Start tags scan for all games without cached tags
    pub(crate) fn start_tags_scan(&mut self) {
        if !self.tags_fetch_queue.is_empty() {
            return;
        }

        // Get games without tags in cache
        let games_to_fetch: Vec<u64> = self.games.iter()
            .filter(|g| !self.tags_cache.contains_key(&g.appid))
            .map(|g| g.appid)
            .collect();

        if !games_to_fetch.is_empty() {
            let total = games_to_fetch.len() as i32;
            self.tags_fetch_queue = games_to_fetch;
            self.tags_scan_total = total;
            self.state = AppState::TagsScanning { current: 0, total };
            self.status = format!("Tags Scan: 0 / {} games", total);
        }
    }

    /// Stop the tags scan
    pub(crate) fn stop_tags_scan(&mut self) {
        self.tags_fetch_queue.clear();
        self.tags_fetching = None;
        self.tags_receiver = None;
        self.tags_scan_total = 0;
        if matches!(self.state, AppState::TagsScanning { .. }) {
            self.state = AppState::Idle;
            self.status = "Tags scan cancelled".to_string();
        }
    }
}

/// POST TTB times to backend API
fn post_ttb_to_backend(token: &str, appid: u64, game_name: &str, times: &overachiever_core::TtbTimes) -> Result<(), String> {
    let client = reqwest::blocking::Client::new();

    #[derive(serde::Serialize)]
    struct TtbSubmit {
        appid: u64,
        game_name: String,
        main: Option<f32>,
        main_extra: Option<f32>,
        completionist: Option<f32>,
    }

    let body = TtbSubmit {
        appid,
        game_name: game_name.to_string(),
        main: times.main,
        main_extra: times.main_extra,
        completionist: times.completionist,
    };

    ttb_log(&format!("Posting to backend: appid={}, game={}, main={:?}", appid, game_name, times.main));

    let response = client
        .post("https://overachiever.space/api/ttb")
        .header("Authorization", format!("Bearer {}", token))
        .json(&body)
        .send()
        .map_err(|e| {
            ttb_log(&format!("POST failed: {}", e));
            e.to_string()
        })?;

    if response.status().is_success() {
        ttb_log(&format!("POST success for appid={}", appid));
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        ttb_log(&format!("POST error: {} - {}", status, body));
        Err(format!("Server returned {}", status))
    }
}
