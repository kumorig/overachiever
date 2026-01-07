//! Main application module

mod state;
mod panels;

use crate::config::Config;
use crate::db::{get_all_games, get_run_history, get_achievement_history, get_log_entries, open_connection, get_last_update, finalize_migration, ensure_user, get_all_achievement_ratings};
use crate::icon_cache::IconCache;
use crate::steam_library::get_installed_games;
use crate::ui::{AppState, SortColumn, SortOrder, TriFilter, ProgressReceiver};
use crate::cloud_sync::{CloudSyncState, AuthResult, CloudOpResult};
use overachiever_core::{Game, RunHistory, AchievementHistory, GameAchievement, LogEntry, SidebarPanel, CloudSyncStatus, TtbTimes};

use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;
use std::time::Instant;

pub struct SteamOverachieverApp {
    pub(crate) config: Config,
    pub(crate) games: Vec<Game>,
    pub(crate) run_history: Vec<RunHistory>,
    pub(crate) achievement_history: Vec<AchievementHistory>,
    pub(crate) log_entries: Vec<LogEntry>,
    pub(crate) status: String,
    pub(crate) state: AppState,
    pub(crate) receiver: Option<ProgressReceiver>,
    pub(crate) sort_column: SortColumn,
    pub(crate) sort_order: SortOrder,
    // Track recently updated games: appid -> time of update
    pub(crate) updated_games: HashMap<u64, Instant>,
    // Track last update time for 2-week warning
    pub(crate) last_update_time: Option<chrono::DateTime<chrono::Utc>>,
    // Force full scan even when all games have been scraped
    pub(crate) force_full_scan: bool,
    // Include unplayed games (0%) in avg completion calculation
    pub(crate) include_unplayed_in_avg: bool,
    // Track which rows are expanded to show achievements
    pub(crate) expanded_rows: HashSet<u64>,
    // Cache loaded achievements for expanded games
    pub(crate) achievements_cache: HashMap<u64, Vec<GameAchievement>>,
    // Icon cache for achievement icons
    pub(crate) icon_cache: IconCache,
    // User achievement ratings: (appid, apiname) -> rating
    pub(crate) user_achievement_ratings: HashMap<(u64, String), u8>,
    // Filters
    pub(crate) filter_name: String,
    pub(crate) filter_achievements: TriFilter,
    pub(crate) filter_playtime: TriFilter,
    // Settings window
    pub(crate) show_settings: bool,
    // GDPR dialog window
    pub(crate) show_gdpr_dialog: bool,
    // Sidebar panel state
    pub(crate) show_stats_panel: bool,
    pub(crate) sidebar_panel: SidebarPanel,
    // Graph tab selections (0 = first option, 1 = second option)
    pub(crate) games_graph_tab: usize,
    pub(crate) achievements_graph_tab: usize,
    // Cloud sync state
    pub(crate) cloud_sync_state: CloudSyncState,
    pub(crate) cloud_status: Option<CloudSyncStatus>,
    // OAuth callback receiver (for Steam login)
    pub(crate) auth_receiver: Option<Receiver<Result<AuthResult, String>>>,
    // Cloud operation receiver (for async upload/download/delete)
    pub(crate) cloud_op_receiver: Option<Receiver<Result<CloudOpResult, String>>>,
    // Pending cloud action (for confirmation dialog)
    pub(crate) pending_cloud_action: Option<CloudAction>,
    // Navigation target for scrolling to an achievement
    pub(crate) navigation_target: Option<(u64, String)>, // (appid, apiname)
    // Whether we need to scroll to the navigation target (one-time scroll)
    pub(crate) needs_scroll_to_target: bool,
    // Last clicked achievement in the log panel (for persistent highlight)
    pub(crate) log_selected_achievement: Option<(u64, String)>, // (appid, apiname)
    // Single game refresh state: appid of game being refreshed
    pub(crate) single_game_refreshing: Option<u64>,
    // Track game launch times for cooldown (disable button for 7s)
    pub(crate) game_launch_times: HashMap<u64, Instant>,
    // Installed games (detected from Steam library folders)
    pub(crate) installed_games: HashSet<u64>,
    // Filter for installed games
    pub(crate) filter_installed: TriFilter,
    // TTB (Time To Beat) cache: appid -> TtbTimes
    pub(crate) ttb_cache: HashMap<u64, TtbTimes>,
    // TTB scan: list of (appid, name) pairs to scan
    pub(crate) ttb_scan_queue: Vec<(u64, String)>,
    // TTB scan: last fetch time (for rate limiting)
    pub(crate) ttb_last_fetch: Option<Instant>,
    // TTB scan: currently fetching single game
    pub(crate) ttb_fetching: Option<u64>,
    // TTB scan: receiver for async fetch result
    pub(crate) ttb_receiver: Option<Receiver<Result<(u64, String, overachiever_core::TtbTimes), String>>>,
    // TTB search dialog: (appid, game_name, editable_search_query)
    pub(crate) ttb_search_pending: Option<(u64, String, String)>,
    // TTB English name fetch: receiver for async result
    pub(crate) english_name_receiver: Option<Receiver<Option<String>>>,
    // Filter for TTB (Time to Beat)
    pub(crate) filter_ttb: TriFilter,
    // Settings tab selection
    pub(crate) settings_tab: SettingsTab,
    // Available system fonts (lazily loaded on first settings open)
    pub(crate) available_fonts: Option<Vec<String>>,
    // Pending font size (before save button is clicked)
    pub(crate) pending_font_size: f32,
    // Flag to trigger font update
    pub(crate) fonts_need_update: bool,
    // Admin mode toggle - enables TTB scanning and per-game TTB fetch
    pub(crate) admin_mode: bool,
    // TTB blacklist - games excluded from TTB scanning (loaded from backend)
    pub(crate) ttb_blacklist: HashSet<u64>,
    // Tag filters - currently selected tags (empty = show all games)
    pub(crate) filter_tags: Vec<String>,
    // Tag search input text for searchable dropdown
    pub(crate) tag_search_input: String,
    // Available tags for dropdown (loaded from backend)
    pub(crate) available_tags: Vec<String>,
    // Tags cache: appid -> Vec<(tag_name, vote_count)>
    pub(crate) tags_cache: HashMap<u64, Vec<(String, u32)>>,
    // Tags fetch queue: list of appids to fetch tags for
    pub(crate) tags_fetch_queue: Vec<u64>,
    // Currently fetching tags for this appid
    pub(crate) tags_fetching: Option<u64>,
    // Receiver for async tag fetch result
    pub(crate) tags_receiver: Option<Receiver<Result<(u64, Vec<(String, u32)>), String>>>,
    // Total count for tags scan progress (0 when not scanning)
    pub(crate) tags_scan_total: i32,
    // Last time we fetched tags (for rate limiting)
    pub(crate) tags_last_fetch: Option<Instant>,
    // Tag search dropdown keyboard navigation - selected index
    pub(crate) tag_search_selected_index: Option<usize>,
    // Tag filter mode: AND (all tags required) or OR (any tag matches)
    pub(crate) tag_filter_mode_and: bool,
    // Selected tag index for vote column display (default 0 = first tag)
    pub(crate) selected_vote_tag_index: Option<usize>,
}

/// Settings tab selection
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Steam,
    Cloud,
    Debug,
}

/// Cloud action pending confirmation
#[derive(Debug, Clone, PartialEq)]
pub enum CloudAction {
    Upload,
    Download,
    Delete,
}

impl SteamOverachieverApp {
    pub fn new() -> Self {
        let config = Config::load();
        let show_settings = !config.is_valid(); // Show settings on first run if not configured
        let steam_id = config.steam_id.as_str();
        let initial_font_size = config.font_size;
        let conn = open_connection().expect("Failed to open database");
        
        // Finalize any pending migrations with the user's steam_id
        if !steam_id.is_empty() {
            let _ = finalize_migration(&conn, steam_id);
            let _ = ensure_user(&conn, steam_id);
        }
        
        let games = get_all_games(&conn, steam_id).unwrap_or_default();
        let run_history = get_run_history(&conn, steam_id).unwrap_or_default();
        let achievement_history = get_achievement_history(&conn, steam_id).unwrap_or_default();
        let log_entries = get_log_entries(&conn, steam_id, 30).unwrap_or_default();
        let last_update_time = get_last_update(&conn).unwrap_or(None);
        let is_cloud_linked = config.cloud_token.is_some();
        
        // Load user achievement ratings - prefer server data if authenticated, fallback to local
        let user_achievement_ratings: HashMap<(u64, String), u8> = if let Some(token) = &config.cloud_token {
            // Try to fetch from server
            match crate::cloud_sync::fetch_user_achievement_ratings(token) {
                Ok(server_ratings) => {
                    // Update local cache with server data
                    for (appid, apiname, rating) in &server_ratings {
                        let _ = crate::db::set_achievement_rating(&conn, steam_id, *appid, apiname, *rating);
                    }
                    server_ratings.into_iter()
                        .map(|(appid, apiname, rating)| ((appid, apiname), rating))
                        .collect()
                }
                Err(_) => {
                    // Fallback to local cache
                    get_all_achievement_ratings(&conn, steam_id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(appid, apiname, rating)| ((appid, apiname), rating))
                        .collect()
                }
            }
        } else {
            // Not authenticated, use local cache only
            get_all_achievement_ratings(&conn, steam_id)
                .unwrap_or_default()
                .into_iter()
                .map(|(appid, apiname, rating)| ((appid, apiname), rating))
                .collect()
        };
        
        // Detect installed Steam games
        let installed_games = get_installed_games();
        
        let mut app = Self {
            config,
            games,
            run_history,
            achievement_history,
            log_entries,
            status: "Ready".to_string(),
            state: AppState::Idle,
            receiver: None,
            sort_column: SortColumn::Name,
            sort_order: SortOrder::Ascending,
            updated_games: HashMap::new(),
            last_update_time,
            force_full_scan: false,
            include_unplayed_in_avg: false,
            expanded_rows: HashSet::new(),
            achievements_cache: HashMap::new(),
            icon_cache: IconCache::new(),
            user_achievement_ratings,
            filter_name: String::new(),
            filter_achievements: TriFilter::All,
            filter_playtime: TriFilter::All,
            show_settings,
            show_gdpr_dialog: false,
            show_stats_panel: true,
            sidebar_panel: SidebarPanel::Stats,
            games_graph_tab: 0,
            achievements_graph_tab: 0,
            cloud_sync_state: if is_cloud_linked { CloudSyncState::Idle } else { CloudSyncState::NotLinked },
            cloud_status: None,
            auth_receiver: None,
            cloud_op_receiver: None,
            pending_cloud_action: None,
            navigation_target: None,
            needs_scroll_to_target: false,
            log_selected_achievement: None,
            single_game_refreshing: None,
            game_launch_times: HashMap::new(),
            installed_games,
            filter_installed: TriFilter::All,
            ttb_cache: HashMap::new(),
            ttb_scan_queue: Vec::new(),
            ttb_last_fetch: None,
            ttb_fetching: None,
            ttb_receiver: None,
            ttb_search_pending: None,
            english_name_receiver: None,
            filter_ttb: TriFilter::All,
            settings_tab: SettingsTab::default(),
            available_fonts: None,
            pending_font_size: initial_font_size,
            fonts_need_update: false,
            admin_mode: false,
            ttb_blacklist: HashSet::new(),
            filter_tags: Vec::new(),
            tag_search_input: String::new(),
            available_tags: Vec::new(),
            tags_cache: HashMap::new(),
            tags_fetch_queue: Vec::new(),
            tags_fetching: None,
            tags_receiver: None,
            tags_scan_total: 0,
            tags_last_fetch: None,
            tag_search_selected_index: None,
            tag_filter_mode_and: true,
            selected_vote_tag_index: None,
        };
        
        // Apply consistent sorting after loading from database
        app.sort_games();

        // Helper to log to ttb_log.txt
        fn init_log(msg: &str) {
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("ttb_log.txt")
            {
                let _ = writeln!(file, "[{}] {}", chrono::Local::now().format("%H:%M:%S"), msg);
            }
        }

        // Load TTB cache from local database
        init_log("Loading TTB cache...");
        app.load_ttb_cache();

        // Load TTB blacklist from backend (games to skip in TTB scan)
        init_log("Loading TTB blacklist...");
        app.load_ttb_blacklist();

        // Load available tags and tags for games from backend
        init_log("Loading available tags...");
        app.load_available_tags();
        init_log(&format!("Loading tags for {} games...", app.games.len()));
        app.load_tags_for_games();
        init_log("Tags loaded, starting update...");

        // Auto-start update on launch
        app.start_update();
        init_log("Update started");
        
        app
    }
}

impl eframe::App for SteamOverachieverApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_progress();
        self.cleanup_expired_flashes();
        self.check_auth_callback();
        self.check_cloud_operation();
        self.ttb_scan_tick(); // Process TTB scan queue
        self.tags_fetch_tick(); // Process tags fetch queue

        let is_busy = self.state.is_busy();
        let has_flashing = !self.updated_games.is_empty();
        let is_linking = self.auth_receiver.is_some();
        let is_cloud_op = self.cloud_op_receiver.is_some();
        let has_launch_cooldowns = !self.game_launch_times.is_empty();
        let is_ttb_scanning = !self.ttb_scan_queue.is_empty();
        let is_ttb_fetching = self.ttb_receiver.is_some();

        // Request repaint while busy or while animations are active
        if is_busy || has_flashing || is_linking || is_cloud_op || has_launch_cooldowns || is_ttb_scanning || is_ttb_fetching {
            ctx.request_repaint();
        }

        // Track window state for persistence (only when not maximized to preserve restore size)
        ctx.input(|i| {
            let maximized = i.viewport().maximized.unwrap_or(false);
            self.config.window_maximized = maximized;

            // Only save position/size when not maximized (to preserve restore dimensions)
            if !maximized {
                if let Some(rect) = i.viewport().inner_rect {
                    self.config.window_x = Some(rect.min.x);
                    // Compensate for title bar offset (inner_rect reports ~30px higher than actual window position)
                    self.config.window_y = Some((rect.min.y - 30.0).max(0.0));
                    self.config.window_width = Some(rect.width());
                    self.config.window_height = Some(rect.height());
                }
            }
        });

        // Clean up expired launch cooldowns
        self.cleanup_expired_launch_cooldowns();

        // Render panels
        self.render_top_panel(ctx);
        self.render_history_panel(ctx);
        self.render_games_table_panel(ctx);

        // Show GDPR modal if needed (for hybrid/remote mode and consent not set)
        self.render_gdpr_modal(ctx);

        // Show TTB search dialog if pending
        self.render_ttb_search_dialog(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Save window state to config on exit
        // Note: We save the last known state before exit
        // The actual window rect is obtained via raw_window_handle integration
        // For simplicity, we save periodically during update and trust that state
        let _ = self.config.save();
    }
}

impl SteamOverachieverApp {
    /// Render the TTB search query dialog
    fn render_ttb_search_dialog(&mut self, ctx: &egui::Context) {
        let pending = match self.ttb_search_pending.take() {
            Some(p) => p,
            None => return,
        };

        let (appid, game_name, mut search_query) = pending;
        let mut confirmed = false;
        let mut cancelled = false;

        // Check if English name fetch completed
        let is_fetching_english = self.english_name_receiver.is_some();
        if let Some(ref rx) = self.english_name_receiver {
            if let Ok(result) = rx.try_recv() {
                if let Some(english_name) = result {
                    // Update search query with cleaned English name
                    search_query = crate::ttb::clean_game_name_for_search(&english_name);
                }
                self.english_name_receiver = None;
            }
        }

        egui::Window::new("Search HowLongToBeat")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([400.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(format!("Searching for: {}", game_name));
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Search query:");
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut search_query)
                            .desired_width(220.0)
                    );
                    // Focus the text field and select all on first show
                    if response.gained_focus() {
                        // Text is already selected by default when focused
                    }
                    // Press Enter to confirm
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        confirmed = true;
                    }
                    // Request focus on first frame (but not while fetching)
                    if !is_fetching_english {
                        response.request_focus();
                    }

                    // English name fetch button
                    if is_fetching_english {
                        ui.spinner();
                    } else if ui.button("EN").on_hover_text("Fetch English name from Steam").clicked() {
                        // Spawn background thread to fetch English name
                        let (tx, rx) = std::sync::mpsc::channel();
                        self.english_name_receiver = Some(rx);
                        std::thread::spawn(move || {
                            let result = crate::ttb::fetch_english_name(appid);
                            let _ = tx.send(result);
                        });
                    }
                });

                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancelled = true;
                    }
                    if ui.button("OK").clicked() {
                        confirmed = true;
                    }
                });

                ui.add_space(4.0);
            });

        if cancelled {
            // Dialog dismissed, don't restore pending state
            self.english_name_receiver = None; // Cancel any pending fetch
        } else if confirmed {
            // Fetch with the (possibly modified) query
            self.fetch_single_ttb_with_query(appid, &game_name, &search_query);
            self.english_name_receiver = None;
        } else {
            // Keep dialog open
            self.ttb_search_pending = Some((appid, game_name, search_query));
        }
    }
}
