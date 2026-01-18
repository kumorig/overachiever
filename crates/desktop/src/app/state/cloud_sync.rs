//! Cloud sync operations

use crate::cloud_sync::{CloudSyncState, CloudOpResult, start_steam_login};
use crate::db::{
    import_cloud_sync_data, get_all_achievements_for_export, get_all_games, 
    get_run_history, get_achievement_history, get_log_entries, open_connection,
    cache_ttb_times
};
use crate::steam_library::get_installed_games_with_sizes;
use overachiever_core::CloudSyncData;

use crate::app::SteamOverachieverApp;

impl SteamOverachieverApp {
    /// Start the Steam login flow to link to cloud
    pub(crate) fn start_cloud_link(&mut self) {
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
        self.config.cloud_token = None;
        let _ = self.config.save();
        self.cloud_status = None;
        self.cloud_sync_state = CloudSyncState::NotLinked;
    }
    
    /// Check for completed cloud operation results
    pub(crate) fn check_cloud_operation(&mut self) {
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
                            self.games = get_all_games(&conn, &steam_id).unwrap_or_default();
                            self.run_history = get_run_history(&conn, &steam_id).unwrap_or_default();
                            self.achievement_history = get_achievement_history(&conn, &steam_id).unwrap_or_default();
                            self.log_entries = get_log_entries(&conn, &steam_id, 30).unwrap_or_default();
                            
                            self.sort_games();
                            
                            // Reload TTB cache from database (in case user had cached TTB data before)
                            self.load_ttb_cache();
                            
                            // Fetch TTB times from server for all games and cache locally
                            let appids: Vec<u64> = self.games.iter().map(|g| g.appid).collect();
                            if !appids.is_empty() {
                                if let Ok(ttb_times) = crate::cloud_sync::fetch_ttb_batch(&appids) {
                                    // Cache each TTB time locally
                                    for times in ttb_times {
                                        if let Ok(conn) = open_connection() {
                                            let _ = cache_ttb_times(&conn, &times);
                                        }
                                        self.ttb_cache.insert(times.appid, times);
                                    }
                                }
                            }
                            
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
}
