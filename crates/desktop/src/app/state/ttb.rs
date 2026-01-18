//! TTB (Time To Beat) scanning and management

use std::io::Write;
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use crate::db::{cache_ttb_times, get_cached_ttb, get_games_without_ttb, open_connection};
use crate::ttb;
use overachiever_core::TtbTimes;

use crate::app::SteamOverachieverApp;
use crate::ui::AppState;

/// Helper function for logging TTB operations to a file
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
            let mut count = 0;
            for game in &self.games {
                if let Ok(Some(times)) = get_cached_ttb(&conn, game.appid) {
                    self.ttb_cache.insert(game.appid, times);
                    count += 1;
                }
            }
            ttb_log(&format!("Loaded {} TTB entries from cache", count));
        } else {
            ttb_log("ERROR: Failed to open database connection for TTB cache");
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
}

/// POST TTB times to backend API
fn post_ttb_to_backend(token: &str, appid: u64, game_name: &str, times: &TtbTimes) -> Result<(), String> {
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
