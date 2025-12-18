//! Main application module

mod state;
mod panels;

use crate::db::{get_all_games, get_run_history, get_achievement_history, open_connection, get_last_update};
use crate::icon_cache::IconCache;
use crate::models::{Game, RunHistory, AchievementHistory, GameAchievement};
use crate::ui::{AppState, SortColumn, SortOrder, TriFilter, ProgressReceiver};

use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

pub struct SteamOverachieverApp {
    pub(crate) games: Vec<Game>,
    pub(crate) run_history: Vec<RunHistory>,
    pub(crate) achievement_history: Vec<AchievementHistory>,
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
    // Filters
    pub(crate) filter_name: String,
    pub(crate) filter_achievements: TriFilter,
    pub(crate) filter_playtime: TriFilter,
}

impl SteamOverachieverApp {
    pub fn new() -> Self {
        let conn = open_connection().expect("Failed to open database");
        let games = get_all_games(&conn).unwrap_or_default();
        let run_history = get_run_history(&conn).unwrap_or_default();
        let achievement_history = get_achievement_history(&conn).unwrap_or_default();
        let last_update_time = get_last_update(&conn).unwrap_or(None);
        
        let mut app = Self {
            games,
            run_history,
            achievement_history,
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
            filter_name: String::new(),
            filter_achievements: TriFilter::All,
            filter_playtime: TriFilter::All,
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
        
        let is_busy = self.state.is_busy();
        let has_flashing = !self.updated_games.is_empty();
        
        // Request repaint while busy or while animations are active
        if is_busy || has_flashing {
            ctx.request_repaint();
        }
        
        // Render panels
        self.render_top_panel(ctx);
        self.render_history_panel(ctx);
        self.render_games_table(ctx);
    }
}
