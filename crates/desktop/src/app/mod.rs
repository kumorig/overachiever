//! Main application module

mod state;
mod panels;

use crate::config::Config;
use crate::db::{get_all_games, get_run_history, get_achievement_history, get_log_entries, open_connection, get_last_update, finalize_migration, ensure_user, get_all_achievement_ratings};
use crate::icon_cache::IconCache;
use crate::steam_library::get_installed_games;
use crate::ui::{AppState, SortColumn, SortOrder, TriFilter, ProgressReceiver};
use crate::cloud_sync::{CloudSyncState, AuthResult, CloudOpResult};
use overachiever_core::{Game, RunHistory, AchievementHistory, GameAchievement, LogEntry, SidebarPanel, CloudSyncStatus};

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
        };
        
        // Apply consistent sorting after loading from database
        app.sort_games();
        
        // Auto-start update on launch
        app.start_update();
        
        app
    }
}

impl eframe::App for SteamOverachieverApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_progress();
        self.cleanup_expired_flashes();
        self.check_auth_callback();
        self.check_cloud_operation();
        
        let is_busy = self.state.is_busy();
        let has_flashing = !self.updated_games.is_empty();
        let is_linking = self.auth_receiver.is_some();
        let is_cloud_op = self.cloud_op_receiver.is_some();
        let has_launch_cooldowns = !self.game_launch_times.is_empty();
        
        // Request repaint while busy or while animations are active
        if is_busy || has_flashing || is_linking || is_cloud_op || has_launch_cooldowns {
            ctx.request_repaint();
        }
        
        // Clean up expired launch cooldowns
        self.cleanup_expired_launch_cooldowns();
        
        // Render panels
        self.render_top_panel(ctx);
        self.render_history_panel(ctx);
        self.render_games_table_panel(ctx);
        
        // Show GDPR modal if needed (for hybrid/remote mode and consent not set)
        self.render_gdpr_modal(ctx);
    }
}
