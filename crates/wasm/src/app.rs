//! WASM App state and UI - matches desktop version layout

use eframe::egui;
use overachiever_core::{
    Game, GameAchievement, UserProfile, RunHistory, AchievementHistory, 
    SyncState, LogEntry, GdprConsent, SidebarPanel, SortColumn, SortOrder, TriFilter,
    sort_games,
};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::cell::RefCell;

use crate::ws_client::WsClient;
use crate::storage::{
    get_token_from_url, get_token_from_storage, save_token_to_storage, clear_token_from_storage,
    get_ws_url_from_location, get_gdpr_consent_from_storage,
};
use crate::http_client::BuildInfo;

// ============================================================================
// Types
// ============================================================================

#[derive(Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Authenticated(UserProfile),
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum AppState {
    #[default]
    Idle,
    Syncing,
    Scanning,
}

impl AppState {
    pub fn is_busy(&self) -> bool {
        !matches!(self, AppState::Idle)
    }
}

// ============================================================================
// Main App
// ============================================================================

pub struct WasmApp {
    // Connection
    pub(crate) server_url: String,
    pub(crate) ws_client: Option<WsClient>,
    pub(crate) connection_state: ConnectionState,
    
    // Data
    pub(crate) games: Vec<Game>,
    pub(crate) games_loaded: bool,
    pub(crate) run_history: Vec<RunHistory>,
    pub(crate) achievement_history: Vec<AchievementHistory>,
    pub(crate) log_entries: Vec<LogEntry>,
    
    // UI state
    pub(crate) status: String,
    pub(crate) app_state: AppState,
    pub(crate) scan_progress: Option<(i32, i32, String)>, // (current, total, game_name)
    pub(crate) force_full_scan: bool,
    pub(crate) sort_column: SortColumn,
    pub(crate) sort_order: SortOrder,
    pub(crate) expanded_rows: HashSet<u64>,
    pub(crate) achievements_cache: HashMap<u64, Vec<GameAchievement>>,
    pub(crate) user_achievement_ratings: HashMap<(u64, String), u8>, // (appid, apiname) -> rating
    pub(crate) pending_ratings: Rc<RefCell<Option<Vec<(u64, String, u8)>>>>, // Incoming ratings from server
    pub(crate) filter_name: String,
    pub(crate) filter_achievements: TriFilter,
    pub(crate) filter_playtime: TriFilter,
    pub(crate) show_login: bool,
    pub(crate) include_unplayed_in_avg: bool,
    pub(crate) show_stats_panel: bool,
    pub(crate) sidebar_panel: SidebarPanel,
    pub(crate) games_graph_tab: usize,
    pub(crate) achievements_graph_tab: usize,
    
    // Token from URL or storage
    pub(crate) auth_token: Option<String>,
    
    // GDPR consent status
    pub(crate) gdpr_consent: GdprConsent,
    
    // Build info (fetched from build_info.json)
    pub(crate) build_info: Rc<RefCell<Option<BuildInfo>>>,
    
    // Navigation target for scrolling to an achievement
    pub(crate) navigation_target: Option<(u64, String)>, // (appid, apiname)
    
    // Whether we need to scroll to the navigation target (one-time scroll)
    pub(crate) needs_scroll_to_target: bool,
    
    // Last clicked achievement in log panel (for persistent highlight)
    pub(crate) log_selected_achievement: Option<(u64, String)>, // (appid, apiname)
    
    // Single game refresh state: appid of game being refreshed
    pub(crate) single_game_refreshing: Option<u64>,
}

impl WasmApp {
    pub fn new() -> Self {
        // Try to get token from URL params or localStorage
        let auth_token = get_token_from_url().or_else(|| get_token_from_storage());
        
        // Auto-detect WebSocket URL from current page location
        let server_url = get_ws_url_from_location();
        
        // Check viewport width to decide if stats panel should start open
        let viewport_width = web_sys::window()
            .and_then(|w| w.inner_width().ok())
            .and_then(|v| v.as_f64())
            .unwrap_or(1200.0);
        let show_stats_panel = viewport_width > 800.0;
        
        // Load GDPR consent from localStorage
        let gdpr_consent = get_gdpr_consent_from_storage();
        
        let mut app = Self {
            server_url,
            ws_client: None,
            connection_state: ConnectionState::Disconnected,
            games: Vec::new(),
            games_loaded: false,
            run_history: Vec::new(),
            achievement_history: Vec::new(),
            log_entries: Vec::new(),
            status: "Connecting...".to_string(),
            app_state: AppState::Idle,
            scan_progress: None,
            force_full_scan: false,
            sort_column: SortColumn::Name,
            sort_order: SortOrder::Ascending,
            expanded_rows: HashSet::new(),
            achievements_cache: HashMap::new(),
            user_achievement_ratings: HashMap::new(),
            pending_ratings: Rc::new(RefCell::new(None)),
            filter_name: String::new(),
            filter_achievements: TriFilter::All,
            filter_playtime: TriFilter::All,
            show_login: false,
            include_unplayed_in_avg: false,
            show_stats_panel,
            sidebar_panel: SidebarPanel::Stats,
            games_graph_tab: 0,
            achievements_graph_tab: 0,
            auth_token,
            gdpr_consent,
            build_info: Rc::new(RefCell::new(None)),
            navigation_target: None,
            needs_scroll_to_target: false,
            log_selected_achievement: None,
            single_game_refreshing: None,
        };
        
        // Fetch build info asynchronously
        app.fetch_build_info();
        
        // Auto-connect on startup
        app.connect();
        app
    }
    
    /// Fetch build info from server
    fn fetch_build_info(&self) {
        let build_info = self.build_info.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match crate::http_client::fetch_build_info().await {
                Ok(info) => {
                    *build_info.borrow_mut() = Some(info);
                }
                Err(e) => {
                    web_sys::console::log_1(&format!("Failed to fetch build info: {}", e).into());
                }
            }
        });
    }
    
    // ========================================================================
    // Connection Management
    // ========================================================================
    
    pub(crate) fn connect(&mut self) {
        if self.connection_state != ConnectionState::Disconnected {
            return;
        }
        
        self.connection_state = ConnectionState::Connecting;
        self.status = "Connecting...".to_string();
        
        match WsClient::new(&self.server_url) {
            Ok(client) => {
                self.ws_client = Some(client);
            }
            Err(e) => {
                self.connection_state = ConnectionState::Error(e.clone());
                self.status = format!("Connection failed: {}", e);
            }
        }
    }
    
    pub(crate) fn check_ws_state(&mut self) {
        if let Some(client) = &self.ws_client {
            use crate::ws_client::WsState;
            match client.state() {
                WsState::Open => {
                    if self.connection_state == ConnectionState::Connecting {
                        self.connection_state = ConnectionState::Connected;
                        self.status = "Connected, authenticating...".to_string();
                        
                        if let Some(token) = &self.auth_token.clone() {
                            client.authenticate(token);
                        } else {
                            self.show_login = true;
                            self.status = "Connected - please log in".to_string();
                        }
                    }
                }
                WsState::Error(e) => {
                    self.connection_state = ConnectionState::Error(e.clone());
                    self.status = format!("Connection error: {}", e);
                }
                WsState::Closed => {
                    if !matches!(self.connection_state, ConnectionState::Disconnected | ConnectionState::Error(_)) {
                        self.connection_state = ConnectionState::Disconnected;
                        self.status = "Disconnected".to_string();
                    }
                }
                _ => {}
            }
        }
    }
    
    /// Fetch user's saved achievement ratings from the server (async)
    fn fetch_user_ratings(&self) {
        if let Some(token) = &self.auth_token {
            let token = token.clone();
            let pending = self.pending_ratings.clone();
            
            wasm_bindgen_futures::spawn_local(async move {
                match crate::http_client::fetch_user_achievement_ratings(&token).await {
                    Ok(ratings) => {
                        *pending.borrow_mut() = Some(ratings);
                        web_sys::console::log_1(&format!("Loaded {} achievement ratings from server", 
                            pending.borrow().as_ref().map_or(0, |r| r.len())).into());
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Failed to fetch achievement ratings: {}", e).into());
                    }
                }
            });
        }
    }
    
    /// Process any pending ratings from async fetch
    fn process_pending_ratings(&mut self) {
        if let Some(ratings) = self.pending_ratings.borrow_mut().take() {
            for (appid, apiname, rating) in ratings {
                self.user_achievement_ratings.insert((appid, apiname), rating);
            }
        }
    }
    
    pub(crate) fn check_messages(&mut self) {
        let messages = if let Some(client) = &self.ws_client {
            client.poll_messages()
        } else {
            vec![]
        };
        
        for msg in messages {
            match msg {
                overachiever_core::ServerMessage::Authenticated { user } => {
                    self.connection_state = ConnectionState::Authenticated(user.clone());
                    self.status = format!("Logged in as {}", user.display_name);
                    
                    if let Some(token) = &self.auth_token {
                        save_token_to_storage(token);
                    }
                    
                    // Auto-fetch games and history after auth
                    if let Some(client) = &self.ws_client {
                        client.fetch_games();
                        client.fetch_history();
                    }
                    
                    // Fetch saved achievement ratings
                    self.fetch_user_ratings();
                }
                overachiever_core::ServerMessage::AuthError { reason } => {
                    self.connection_state = ConnectionState::Error(reason.clone());
                    self.status = format!("Auth failed: {}", reason);
                    self.show_login = true;
                    // Clear invalid token
                    self.auth_token = None;
                    clear_token_from_storage();
                }
                overachiever_core::ServerMessage::Games { games } => {
                    self.games = games;
                    self.games_loaded = true;
                    self.app_state = AppState::Idle;
                    self.status = format!("Loaded {} games", self.games.len());
                    sort_games(&mut self.games, self.sort_column, self.sort_order);
                    // Refresh history (run_history may have been recorded on sync)
                    if let Some(client) = &self.ws_client {
                        client.fetch_history();
                    }
                }
                overachiever_core::ServerMessage::Achievements { appid, achievements } => {
                    self.achievements_cache.insert(appid, achievements);
                }
                overachiever_core::ServerMessage::Error { message } => {
                    self.app_state = AppState::Idle;
                    self.scan_progress = None;
                    self.status = format!("Error: {}", message);
                }
                overachiever_core::ServerMessage::SyncProgress { state } => {
                    match state {
                        SyncState::Starting => {
                            self.status = "Starting scan...".to_string();
                        }
                        SyncState::ScrapingAchievements { current, total, game_name } => {
                            self.scan_progress = Some((current, total, game_name.clone()));
                            self.status = format!("Scanning {}/{}: {}", current, total, game_name);
                        }
                        SyncState::Done => {
                            self.app_state = AppState::Idle;
                            self.scan_progress = None;
                            self.status = "Scan complete!".to_string();
                        }
                        SyncState::Error { message } => {
                            self.app_state = AppState::Idle;
                            self.scan_progress = None;
                            self.status = format!("Scan error: {}", message);
                        }
                        _ => {}
                    }
                }
                overachiever_core::ServerMessage::SyncComplete { result, games } => {
                    self.games = games;
                    self.app_state = AppState::Idle;
                    self.scan_progress = None;
                    self.status = format!("Scan complete! Updated {} games, {} achievements", result.games_updated, result.achievements_updated);
                    sort_games(&mut self.games, self.sort_column, self.sort_order);
                    // Refresh history
                    if let Some(client) = &self.ws_client {
                        client.fetch_history();
                    }
                }
                overachiever_core::ServerMessage::SingleGameRefreshComplete { appid, game, achievements } => {
                    // Update the game in our list
                    if let Some(g) = self.games.iter_mut().find(|g| g.appid == appid) {
                        *g = game;
                    }
                    // Update achievements cache
                    self.achievements_cache.insert(appid, achievements);
                    // Clear the refresh state
                    self.single_game_refreshing = None;
                    self.status = "Refresh complete!".to_string();
                    // Re-sort games
                    sort_games(&mut self.games, self.sort_column, self.sort_order);
                }
                overachiever_core::ServerMessage::History { run_history, achievement_history, log_entries } => {
                    web_sys::console::log_1(&format!("Received History: {} run_history, {} achievement_history, {} log_entries", 
                        run_history.len(), achievement_history.len(), log_entries.len()).into());
                    self.run_history = run_history;
                    self.achievement_history = achievement_history;
                    self.log_entries = log_entries;
                }
                _ => {}
            }
        }
    }
    
    // ========================================================================
    // Actions
    // ========================================================================
    
    pub(crate) fn start_sync(&mut self) {
        if let Some(client) = &self.ws_client {
            self.app_state = AppState::Syncing;
            self.status = "Syncing from Steam...".to_string();
            client.sync_from_steam();
        }
    }
    
    pub(crate) fn start_full_scan(&mut self) {
        if let Some(client) = &self.ws_client {
            self.app_state = AppState::Scanning;
            self.status = "Starting full scan...".to_string();
            client.full_scan(self.force_full_scan);
        }
    }
    
    pub(crate) fn games_needing_scrape(&self) -> usize {
        self.games.iter().filter(|g| g.achievements_total.is_none()).count()
    }
}

// ============================================================================
// eframe::App Implementation
// ============================================================================

impl eframe::App for WasmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_ws_state();
        self.check_messages();
        self.process_pending_ratings();
        
        if matches!(self.connection_state, ConnectionState::Disconnected) {
            self.connect();
        }
        
        ctx.request_repaint();
        
        // Render panels (implemented in panels.rs)
        self.render_top_panel(ctx);
        self.render_stats_panel(ctx);
        self.render_games_panel(ctx);
        
        // Show GDPR modal if consent not set (implemented in gdpr.rs)
        self.render_gdpr_modal(ctx);
    }
}
