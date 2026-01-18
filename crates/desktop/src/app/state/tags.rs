//! Tags (SteamSpy) scanning and management

use std::thread;
use std::time::{Duration, Instant};
use std::sync::mpsc::channel;
use crate::{cloud_sync, steamspy};
use crate::app::SteamOverachieverApp;
use crate::ui::AppState;

impl SteamOverachieverApp {
    /// Load available tags from backend on startup
    pub(crate) fn load_available_tags(&mut self) {
        match cloud_sync::fetch_tag_names() {
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
            match cloud_sync::fetch_tags_batch(chunk) {
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
                        thread::spawn(move || {
                            let _ = cloud_sync::submit_tags(&token, appid, &tags_for_post);
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
                let result = steamspy::fetch_tags(appid);
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
