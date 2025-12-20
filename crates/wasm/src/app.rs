//! WASM App state and UI - matches desktop version layout

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular;
use egui_plot::{Line, Plot, PlotPoints};
use overachiever_core::{Game, GameAchievement, UserProfile, RunHistory, AchievementHistory, SyncState, LogEntry};
use std::collections::{HashMap, HashSet};

use crate::ws_client::WsClient;

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

#[derive(Clone, Copy, PartialEq)]
pub enum SortColumn {
    Name,
    LastPlayed,
    Playtime,
    AchievementsTotal,
    AchievementsPercent,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn toggle(&self) -> Self {
        match self {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum TriFilter {
    #[default]
    All,
    With,
    Without,
}

impl TriFilter {
    pub fn cycle(&self) -> Self {
        match self {
            TriFilter::All => TriFilter::With,
            TriFilter::With => TriFilter::Without,
            TriFilter::Without => TriFilter::All,
        }
    }
    
    pub fn label(&self, with_text: &str, without_text: &str) -> String {
        match self {
            TriFilter::All => "All".to_string(),
            TriFilter::With => with_text.to_string(),
            TriFilter::Without => without_text.to_string(),
        }
    }
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
    server_url: String,
    ws_client: Option<WsClient>,
    connection_state: ConnectionState,
    
    // Data
    games: Vec<Game>,
    games_loaded: bool,
    run_history: Vec<RunHistory>,
    achievement_history: Vec<AchievementHistory>,
    log_entries: Vec<LogEntry>,
    
    // UI state
    status: String,
    app_state: AppState,
    scan_progress: Option<(i32, i32, String)>, // (current, total, game_name)
    force_full_scan: bool,
    sort_column: SortColumn,
    sort_order: SortOrder,
    expanded_rows: HashSet<u64>,
    achievements_cache: HashMap<u64, Vec<GameAchievement>>,
    filter_name: String,
    filter_achievements: TriFilter,
    filter_playtime: TriFilter,
    show_login: bool,
    include_unplayed_in_avg: bool,
    show_stats_panel: bool,
    
    // Token from URL or storage
    auth_token: Option<String>,
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
            filter_name: String::new(),
            filter_achievements: TriFilter::All,
            filter_playtime: TriFilter::All,
            show_login: false,
            include_unplayed_in_avg: false,
            show_stats_panel,
            auth_token,
        };
        
        // Auto-connect on startup
        app.connect();
        app
    }
    
    // ========================================================================
    // Connection Management
    // ========================================================================
    
    fn connect(&mut self) {
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
    
    fn check_ws_state(&mut self) {
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
    
    fn check_messages(&mut self) {
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
                    self.sort_games();
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
                    self.sort_games();
                    // Refresh history
                    if let Some(client) = &self.ws_client {
                        client.fetch_history();
                    }
                }
                overachiever_core::ServerMessage::History { run_history, achievement_history, log_entries } => {
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
    
    fn start_sync(&mut self) {
        if let Some(client) = &self.ws_client {
            self.app_state = AppState::Syncing;
            self.status = "Syncing from Steam...".to_string();
            client.sync_from_steam();
        }
    }
    
    fn start_full_scan(&mut self) {
        if let Some(client) = &self.ws_client {
            self.app_state = AppState::Scanning;
            self.status = "Starting full scan...".to_string();
            client.full_scan(self.force_full_scan);
        }
    }
    
    fn games_needing_scrape(&self) -> usize {
        self.games.iter().filter(|g| g.achievements_total.is_none()).count()
    }
    
    // ========================================================================
    // Sorting
    // ========================================================================
    
    fn set_sort(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.sort_order = self.sort_order.toggle();
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Ascending;
        }
        self.sort_games();
    }
    
    fn sort_games(&mut self) {
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
                    let cmp = b.rtime_last_played.cmp(&a.rtime_last_played);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::Playtime => {
                self.games.sort_by(|a, b| {
                    let cmp = b.playtime_forever.cmp(&a.playtime_forever);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::AchievementsTotal => {
                self.games.sort_by(|a, b| {
                    let cmp = b.achievements_total.cmp(&a.achievements_total);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::AchievementsPercent => {
                self.games.sort_by(|a, b| {
                    let a_pct = a.completion_percent().unwrap_or(-1.0);
                    let b_pct = b.completion_percent().unwrap_or(-1.0);
                    let cmp = b_pct.partial_cmp(&a_pct).unwrap_or(std::cmp::Ordering::Equal);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
        }
    }
    
    fn sort_indicator(&self, column: SortColumn) -> &'static str {
        if self.sort_column == column {
            match self.sort_order {
                SortOrder::Ascending => regular::CARET_UP,
                SortOrder::Descending => regular::CARET_DOWN,
            }
        } else {
            ""
        }
    }
    
    // ========================================================================
    // Filtering
    // ========================================================================
    
    fn get_filtered_indices(&self) -> Vec<usize> {
        let filter_name_lower = self.filter_name.to_lowercase();
        
        self.games.iter()
            .enumerate()
            .filter(|(_, g)| {
                if !filter_name_lower.is_empty() && !g.name.to_lowercase().contains(&filter_name_lower) {
                    return false;
                }
                let has_achievements = g.achievements_total.map(|t| t > 0).unwrap_or(false);
                match self.filter_achievements {
                    TriFilter::All => {}
                    TriFilter::With => if !has_achievements { return false; }
                    TriFilter::Without => if has_achievements { return false; }
                }
                let has_playtime = g.rtime_last_played.map(|ts| ts > 0).unwrap_or(false);
                match self.filter_playtime {
                    TriFilter::All => {}
                    TriFilter::With => if !has_playtime { return false; }
                    TriFilter::Without => if has_playtime { return false; }
                }
                true
            })
            .map(|(idx, _)| idx)
            .collect()
    }
    
    // ========================================================================
    // Stats Calculations
    // ========================================================================
    
    fn calculate_stats(&self) -> (i32, i32, f32, usize, usize) {
        let games_with_ach: Vec<_> = self.games.iter()
            .filter(|g| g.achievements_total.map(|t| t > 0).unwrap_or(false))
            .collect();
        
        let total_achievements: i32 = games_with_ach.iter()
            .filter_map(|g| g.achievements_total)
            .sum();
        
        let unlocked_achievements: i32 = games_with_ach.iter()
            .filter_map(|g| g.achievements_unlocked)
            .sum();
        
        let played_count = games_with_ach.iter()
            .filter(|g| g.playtime_forever > 0)
            .count();
        let unplayed_count = games_with_ach.len() - played_count;
        
        let completion_percents: Vec<f32> = if self.include_unplayed_in_avg {
            games_with_ach.iter()
                .filter_map(|g| g.completion_percent())
                .collect()
        } else {
            games_with_ach.iter()
                .filter(|g| g.playtime_forever > 0)
                .filter_map(|g| g.completion_percent())
                .collect()
        };
        
        let avg_completion = if completion_percents.is_empty() {
            0.0
        } else {
            completion_percents.iter().sum::<f32>() / completion_percents.len() as f32
        };
        
        (unlocked_achievements, total_achievements, avg_completion, played_count, unplayed_count)
    }
}

// ============================================================================
// eframe::App Implementation
// ============================================================================

impl eframe::App for WasmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_ws_state();
        self.check_messages();
        
        if matches!(self.connection_state, ConnectionState::Disconnected) {
            self.connect();
        }
        
        ctx.request_repaint();
        
        // Render panels
        self.render_top_panel(ctx);
        self.render_stats_panel(ctx);
        self.render_games_panel(ctx);
    }
}

impl WasmApp {
    // ========================================================================
    // Top Panel
    // ========================================================================
    
    fn render_top_panel(&mut self, ctx: &egui::Context) {
        let is_busy = self.app_state.is_busy();
        let is_authenticated = matches!(self.connection_state, ConnectionState::Authenticated(_));
        
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Overachiever");
                ui.separator();
                
                match &self.connection_state {
                    ConnectionState::Disconnected | ConnectionState::Connecting => {
                        ui.spinner();
                        ui.label("Connecting...");
                    }
                    ConnectionState::Connected => {
                        ui.spinner();
                        ui.label("Authenticating...");
                    }
                    ConnectionState::Authenticated(user) => {
                        ui.label(format!("{} {}", regular::USER, user.display_name));
                        ui.separator();
                        
                        // Sync button
                        if ui.add_enabled(!is_busy, egui::Button::new(format!("{} Sync", regular::ARROWS_CLOCKWISE))).clicked() {
                            self.start_sync();
                        }
                        
                        // Full Scan button
                        let needs_scan = self.games_needing_scrape();
                        let scan_label = if needs_scan > 0 {
                            format!("{} Full Scan ({})", regular::GAME_CONTROLLER, needs_scan)
                        } else {
                            format!("{} Full Scan", regular::GAME_CONTROLLER)
                        };
                        let can_scan = (needs_scan > 0 || self.force_full_scan) && self.games_loaded;
                        if ui.add_enabled(!is_busy && can_scan, egui::Button::new(scan_label)).clicked() {
                            self.start_full_scan();
                        }
                        
                        ui.checkbox(&mut self.force_full_scan, "Force");
                    }
                    ConnectionState::Error(e) => {
                        ui.colored_label(egui::Color32::RED, format!("{} {}", regular::WARNING, e));
                        if ui.button("Retry").clicked() {
                            self.connection_state = ConnectionState::Disconnected;
                        }
                    }
                }
                
                ui.separator();
                
                if is_busy {
                    ui.spinner();
                    if let Some((current, total, _)) = &self.scan_progress {
                        let progress = *current as f32 / *total as f32;
                        ui.add(egui::ProgressBar::new(progress)
                            .text(&self.status)
                            .animate(true));
                    } else {
                        ui.label(&self.status);
                    }
                } else {
                    ui.label(&self.status);
                }
                
                // Logout on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if is_authenticated {
                        if ui.button(format!("{} Logout", regular::SIGN_OUT)).clicked() {
                            self.auth_token = None;
                            clear_token_from_storage();
                            self.connection_state = ConnectionState::Disconnected;
                            self.games.clear();
                            self.games_loaded = false;
                        }
                    }
                });
            });
        });
    }
    
    // ========================================================================
    // Stats Panel (Right Sidebar)
    // ========================================================================
    
    fn render_stats_panel(&mut self, ctx: &egui::Context) {
        if !matches!(self.connection_state, ConnectionState::Authenticated(_)) {
            return;
        }
        
        // Slightly darker background for the sidebar in dark mode
        let panel_fill = ctx.style().visuals.window_fill();
        let darker_fill = egui::Color32::from_rgb(
            panel_fill.r().saturating_sub(8),
            panel_fill.g().saturating_sub(8),
            panel_fill.b().saturating_sub(8),
        );
        let panel_frame = egui::Frame::side_top_panel(&ctx.style())
            .fill(darker_fill);
        
        if !self.show_stats_panel {
            // Collapsed sidebar - only show open button
            egui::SidePanel::right("stats_panel_collapsed")
                .exact_width(36.0)
                .resizable(false)
                .frame(panel_frame)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    if ui.button(regular::CARET_LEFT.to_string()).on_hover_text("Open Stats Panel").clicked() {
                        self.show_stats_panel = true;
                    }
                });
            return;
        }
        
        // Get available width - for mobile (< 600px), use full width
        let available_width = ctx.input(|i| i.viewport().inner_rect.map(|r| r.width()).unwrap_or(800.0));
        let is_mobile = available_width < 600.0;
        
        let panel = egui::SidePanel::right("stats_panel")
            .resizable(!is_mobile)
            .frame(panel_frame);
        
        let panel = if is_mobile {
            panel.exact_width(available_width)
        } else {
            panel.default_width(320.0)
        };
        
        panel.show(ctx, |ui| {
                // Close button at top left (chevron right to close/collapse)
                ui.horizontal(|ui| {
                    if ui.small_button(regular::CARET_RIGHT.to_string()).on_hover_text("Close Stats Panel").clicked() {
                        self.show_stats_panel = false;
                    }
                });
                ui.separator();
                
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.render_games_over_time(ui);
                    ui.add_space(16.0);
                    self.render_achievement_progress(ui);
                    ui.add_space(16.0);
                    self.render_games_breakdown(ui);
                    ui.add_space(16.0);
                    self.render_log(ui);
                });
            });
    }
    
    fn render_games_over_time(&self, ui: &mut egui::Ui) {
        ui.heading("Games Over Time");
        ui.separator();
        
        let points: PlotPoints = if self.run_history.is_empty() {
            PlotPoints::default()
        } else {
            self.run_history
                .iter()
                .enumerate()
                .map(|(i, h)| [i as f64, h.total_games as f64])
                .collect()
        };
        
        let line = Line::new("Total Games", points)
            .color(egui::Color32::from_rgb(100, 180, 255));
        
        Plot::new("games_history")
            .height(120.0)
            .width(ui.available_width())
            .auto_bounds(egui::Vec2b::new(true, true))
            .show_axes([false, true])
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show(ui, |plot_ui| {
                plot_ui.line(line);
            });
    }
    
    fn render_achievement_progress(&mut self, ui: &mut egui::Ui) {
        ui.heading("Achievement Progress");
        ui.separator();
        
        let (avg_completion_points, overall_pct_points, y_min, y_max) = if self.achievement_history.is_empty() {
            (PlotPoints::default(), PlotPoints::default(), 0.0, 100.0)
        } else {
            // Line 1: Average game completion %
            let avg_points: PlotPoints = self.achievement_history
                .iter()
                .enumerate()
                .map(|(i, h)| [i as f64, h.avg_completion_percent as f64])
                .collect();
            
            // Line 2: Overall achievement % (unlocked / total)
            let overall_points: PlotPoints = self.achievement_history
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    let pct = if h.total_achievements > 0 {
                        h.unlocked_achievements as f64 / h.total_achievements as f64 * 100.0
                    } else {
                        0.0
                    };
                    [i as f64, pct]
                })
                .collect();
            
            // Calculate Y-axis bounds based on actual data
            let all_values: Vec<f64> = self.achievement_history
                .iter()
                .flat_map(|h| {
                    let overall_pct = if h.total_achievements > 0 {
                        h.unlocked_achievements as f64 / h.total_achievements as f64 * 100.0
                    } else {
                        0.0
                    };
                    vec![h.avg_completion_percent as f64, overall_pct]
                })
                .collect();
            
            let min_y = all_values.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
            let max_y = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max).min(100.0);
            
            // Add some padding
            let range = max_y - min_y;
            let padding = (range * 0.05).max(1.0);
            let y_min_val = (min_y - padding).max(0.0);
            let y_max_val = (max_y + padding).min(100.0);
            
            (avg_points, overall_points, y_min_val, y_max_val)
        };
        
        let avg_line = Line::new("Avg Game Completion %", avg_completion_points)
            .color(egui::Color32::from_rgb(100, 200, 100));
        let overall_line = Line::new("Overall Achievement %", overall_pct_points)
            .color(egui::Color32::from_rgb(100, 150, 255));
        
        Plot::new("achievements_history")
            .height(120.0)
            .width(ui.available_width())
            .legend(egui_plot::Legend::default())
            .auto_bounds(egui::Vec2b::new(true, true))
            .include_y(y_min)
            .include_y(y_max)
            .show_axes([false, true])
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show(ui, |plot_ui| {
                plot_ui.line(avg_line);
                plot_ui.line(overall_line);
            });
    }
    
    fn render_current_stats(&mut self, ui: &mut egui::Ui) {
        let (unlocked, total, avg_completion, played_count, unplayed_count) = self.calculate_stats();
        let yellow = egui::Color32::from_rgb(255, 215, 0);
        
        let overall_pct = if total > 0 {
            unlocked as f32 / total as f32 * 100.0
        } else {
            0.0
        };
        
        ui.horizontal(|ui| {
            ui.label("Total achievements:");
            ui.label(egui::RichText::new(format!("{}", unlocked)).color(yellow).strong());
            ui.label("/");
            ui.label(egui::RichText::new(format!("{}", total)).color(yellow).strong());
            ui.label("(");
            ui.label(egui::RichText::new(format!("{:.1}%", overall_pct)).color(yellow).strong());
            ui.label(")");
        });
        
        ui.horizontal(|ui| {
            ui.label("Avg. game completion:");
            ui.label(egui::RichText::new(format!("{:.1}%", avg_completion)).color(yellow).strong());
            ui.checkbox(&mut self.include_unplayed_in_avg, "Include unplayed");
        });
        
        let total_with_ach = played_count + unplayed_count;
        let unplayed_pct = if total_with_ach > 0 {
            unplayed_count as f32 / total_with_ach as f32 * 100.0
        } else {
            0.0
        };
        
        ui.horizontal(|ui| {
            ui.label("Unplayed games:");
            ui.label(egui::RichText::new(format!("{}", unplayed_count)).color(yellow).strong());
            ui.label("(");
            ui.label(egui::RichText::new(format!("{:.1}%", unplayed_pct)).color(yellow).strong());
            ui.label(")");
        });
    }
    
    fn render_games_breakdown(&mut self, ui: &mut egui::Ui) {
        ui.heading(format!("{} Breakdown", regular::GAME_CONTROLLER));
        ui.separator();
        
        if self.games.is_empty() {
            ui.label("Sync your games to see stats.");
            return;
        }
        
        // Show current stats (moved from below charts)
        self.render_current_stats(ui);
        ui.add_space(8.0);
        
        let yellow = egui::Color32::from_rgb(255, 215, 0);
        let (_, _, _, played_count, unplayed_count) = self.calculate_stats();
        let total_with_ach = played_count + unplayed_count;
        
        ui.add_space(8.0);
        
        ui.horizontal(|ui| {
            ui.label("Total games:");
            ui.label(egui::RichText::new(format!("{}", self.games.len())).color(yellow).strong());
        });
        
        ui.horizontal(|ui| {
            ui.label("Games with achievements:");
            ui.label(egui::RichText::new(format!("{}", total_with_ach)).color(yellow).strong());
        });
        
        let completed = self.games.iter()
            .filter(|g| g.completion_percent().map(|p| p >= 100.0).unwrap_or(false))
            .count();
        ui.horizontal(|ui| {
            ui.label(format!("{} 100% completed:", regular::MEDAL));
            ui.label(egui::RichText::new(format!("{}", completed)).color(yellow).strong());
        });
        
        let needs_scan = self.games_needing_scrape();
        if needs_scan > 0 {
            ui.horizontal(|ui| {
                ui.label("Needs scanning:");
                ui.label(egui::RichText::new(format!("{}", needs_scan)).color(egui::Color32::LIGHT_GRAY));
            });
        }
    }
    
    fn render_log(&self, ui: &mut egui::Ui) {
        // Colors for different elements
        let date_color = egui::Color32::from_rgb(130, 130, 130);  // Gray for dates
        let game_color = egui::Color32::from_rgb(100, 180, 255);  // Blue for game names
        let achievement_color = egui::Color32::from_rgb(255, 215, 0);  // Gold for achievement names
        let alt_bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8);  // Subtle alternating bg
        
        ui.collapsing(format!("{} Log", regular::SCROLL), |ui| {
            if self.log_entries.is_empty() {
                ui.label("No activity yet.");
            } else {
                for (i, entry) in self.log_entries.iter().enumerate() {
                    // Alternating background
                    let row_rect = ui.available_rect_before_wrap();
                    let row_rect = egui::Rect::from_min_size(
                        row_rect.min,
                        egui::vec2(row_rect.width(), 24.0)
                    );
                    if i % 2 == 1 {
                        ui.painter().rect_filled(row_rect, 2.0, alt_bg);
                    }
                    
                    match entry {
                        LogEntry::Achievement { appid, game_name, achievement_name, timestamp, achievement_icon, game_icon_url } => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                
                                // Game icon (left)
                                if let Some(icon_hash) = game_icon_url {
                                    if !icon_hash.is_empty() {
                                        let icon_url = game_icon_url_from_hash(*appid, icon_hash);
                                        ui.add(
                                            egui::Image::new(icon_url)
                                                .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                                .corner_radius(2.0)
                                        );
                                    }
                                }
                                
                                // Achievement icon (right of game icon)
                                if !achievement_icon.is_empty() {
                                    let proxied_icon = proxy_steam_image_url(achievement_icon);
                                    ui.add(
                                        egui::Image::new(proxied_icon)
                                            .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                            .corner_radius(2.0)
                                    );
                                }
                                
                                ui.label(egui::RichText::new(timestamp.format("%Y-%m-%d").to_string()).color(date_color).small());
                                ui.label(egui::RichText::new(achievement_name).color(achievement_color).strong());
                                ui.label(egui::RichText::new("in").small());
                                ui.label(egui::RichText::new(format!("{}!", game_name)).color(game_color));
                            });
                        }
                        LogEntry::FirstPlay { appid, game_name, timestamp, game_icon_url } => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                
                                // Game icon
                                if let Some(icon_hash) = game_icon_url {
                                    if !icon_hash.is_empty() {
                                        let icon_url = game_icon_url_from_hash(*appid, icon_hash);
                                        ui.add(
                                            egui::Image::new(icon_url)
                                                .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                                .corner_radius(2.0)
                                        );
                                    } else {
                                        ui.add_space(22.0);
                                    }
                                } else {
                                    ui.add_space(22.0);
                                }
                                
                                ui.label(egui::RichText::new(timestamp.format("%Y-%m-%d").to_string()).color(date_color).small());
                                ui.label(egui::RichText::new(game_name).color(game_color));
                                ui.label(egui::RichText::new("played for the first time!").small());
                            });
                        }
                    }
                }
            }
        });
    }
    
    // ========================================================================
    // Games Panel (Center)
    // ========================================================================
    
    fn render_games_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if !matches!(self.connection_state, ConnectionState::Authenticated(_)) {
                self.render_login_prompt(ui);
                return;
            }
            
            if self.games.is_empty() {
                if !self.games_loaded {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                        ui.label("Loading games...");
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.label("No games found. Click 'Sync' to load your Steam library.");
                            ui.add_space(12.0);
                            if ui.button(format!("{} Sync from Steam", regular::ARROWS_CLOCKWISE)).clicked() {
                                self.start_sync();
                            }
                        });
                    });
                }
                return;
            }
            
            ui.heading(format!("Games Library ({} games)", self.games.len()));
            ui.separator();
            
            self.render_filter_bar(ui);
            ui.add_space(4.0);
            
            let filtered_indices = self.get_filtered_indices();
            let filtered_count = filtered_indices.len();
            
            if filtered_count != self.games.len() {
                ui.label(format!("Showing {} of {} games", filtered_count, self.games.len()));
            }
            
            self.render_games_table(ui, filtered_indices);
        });
    }
    
    fn render_login_prompt(&self, ui: &mut egui::Ui) {
        match &self.connection_state {
            ConnectionState::Connecting | ConnectionState::Disconnected => {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                    ui.label("Connecting to server...");
                });
            }
            ConnectionState::Connected => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        
                        // Explanation text - use LayoutJob for inline formatting without spacing issues
                        let mut job = egui::text::LayoutJob::default();
                        job.append("A ", 0.0, egui::TextFormat::simple(egui::FontId::default(), ui.style().visuals.text_color()));
                        job.append("Steam ID", 0.0, egui::TextFormat::simple(egui::FontId::default(), egui::Color32::WHITE));
                        job.append(" is needed to fetch your game list and to see achievement completion status.", 0.0, egui::TextFormat::simple(egui::FontId::default(), ui.style().visuals.text_color()));
                        job.wrap = egui::text::TextWrapping {
                            max_width: ui.available_width().min(500.0),
                            ..Default::default()
                        };
                        ui.label(job);
                        
                        ui.add_space(12.0);
                        
                        let mut job2 = egui::text::LayoutJob::default();
                        job2.append("You also need to set your game list to ", 0.0, egui::TextFormat::simple(egui::FontId::default(), ui.style().visuals.text_color()));
                        job2.append("public", 0.0, egui::TextFormat::simple(egui::FontId::default(), egui::Color32::WHITE));
                        job2.append(" in Steam privacy settings for this to work.", 0.0, egui::TextFormat::simple(egui::FontId::default(), ui.style().visuals.text_color()));
                        job2.wrap = egui::text::TextWrapping {
                            max_width: ui.available_width().min(500.0),
                            ..Default::default()
                        };
                        ui.label(job2);
                        
                        ui.add_space(8.0);
                        ui.label("If you do not want to share this data, then this site will not accomplish much for you.");
                        
                        ui.add_space(24.0);
                        
                        // Steam Sign In button (clickable image with border)
                        let login_url = get_auth_url();
                        let image = egui::Image::new(egui::include_image!("../../../assets/sits_02.png"))
                            .fit_to_exact_size(egui::vec2(109.0, 66.0));
                        
                        let response = ui.add(
                            egui::Button::image(image)
                                .frame(true)
                        );
                        
                        if response.clicked() {
                            let _ = web_sys::window()
                                .and_then(|w| w.location().set_href(&login_url).ok());
                        }
                        
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        
                        ui.add_space(32.0);
                        ui.separator();
                        ui.add_space(16.0);
                        
                        // Instructions in collapsible section
                        egui::CollapsingHeader::new("How to set your Steam profile to public")
                            .default_open(false)
                            .show(ui, |ui| {
                                ui.add_space(16.0);
                                
                                ui.add(
                                    egui::Image::new(egui::include_image!("../../../assets/step1.png"))
                                        .fit_to_exact_size(egui::vec2(348.0,99.0))
                                        .corner_radius(4.0)
                                );
                                ui.add_space(16.0);
                                
                                ui.add(
                                    egui::Image::new(egui::include_image!("../../../assets/step2.png"))
                                        .fit_to_exact_size(egui::vec2(348.0, 200.0))
                                        .corner_radius(4.0)
                                );
                                ui.add_space(16.0);
                                
                                ui.add(
                                    egui::Image::new(egui::include_image!("../../../assets/step3.png"))
                                        .fit_to_exact_size(egui::vec2(300.0, 600.0))
                                        .corner_radius(4.0)
                                );
                                ui.add_space(16.0);
                                
                                ui.add(
                                    egui::Image::new(egui::include_image!("../../../assets/step4.png"))
                                        .fit_to_exact_size(egui::vec2(500.0, 300.0))
                                        .corner_radius(4.0)
                                );
                            });
                        
                        ui.add_space(20.0);
                    });
                });
            }
            ConnectionState::Error(e) => {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                });
            }
            ConnectionState::Authenticated(_) => {}
        }
    }
    
    fn render_filter_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.add(egui::TextEdit::singleline(&mut self.filter_name)
                .hint_text("Search by name...")
                .desired_width(150.0));
            
            ui.add_space(10.0);
            
            let ach_label = format!("Achievements: {}", self.filter_achievements.label("With", "Without"));
            if ui.button(&ach_label).clicked() {
                self.filter_achievements = self.filter_achievements.cycle();
            }
            
            let play_label = format!("Played: {}", self.filter_playtime.label("Yes", "No"));
            if ui.button(&play_label).clicked() {
                self.filter_playtime = self.filter_playtime.cycle();
            }
            
            let has_filters = !self.filter_name.is_empty() 
                || self.filter_achievements != TriFilter::All 
                || self.filter_playtime != TriFilter::All;
            
            if has_filters {
                if ui.button("Clear").clicked() {
                    self.filter_name.clear();
                    self.filter_achievements = TriFilter::All;
                    self.filter_playtime = TriFilter::All;
                }
            }
        });
    }
    
    fn render_games_table(&mut self, ui: &mut egui::Ui, filtered_indices: Vec<usize>) {
        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);
        
        let available_height = ui.available_height();
        
        // Calculate row heights - expanded rows are taller
        let row_heights: Vec<f32> = filtered_indices.iter().map(|&idx| {
            let appid = self.games[idx].appid;
            if self.expanded_rows.contains(&appid) {
                text_height + 330.0 // Extra height for achievement list
            } else {
                text_height
            }
        }).collect();
        
        // Track which rows need achievement fetch
        let mut needs_fetch: Vec<u64> = Vec::new();
        
        TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(200.0).clip(true).resizable(true)) // Name - resizable
            .column(Column::exact(90.0)) // Last Played - fixed
            .column(Column::exact(80.0)) // Playtime - fixed
            .column(Column::exact(100.0)) // Achievements - fixed
            .column(Column::exact(60.0)) // Percent - fixed
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height)
            .header(20.0, |mut header| {
                header.col(|ui| {
                    let indicator = self.sort_indicator(SortColumn::Name);
                    let label = if indicator.is_empty() { "Name".to_string() } else { format!("Name {}", indicator) };
                    if ui.selectable_label(self.sort_column == SortColumn::Name, label).clicked() {
                        self.set_sort(SortColumn::Name);
                    }
                });
                header.col(|ui| {
                    let indicator = self.sort_indicator(SortColumn::LastPlayed);
                    let label = if indicator.is_empty() { "Last Played".to_string() } else { format!("Last Played {}", indicator) };
                    if ui.selectable_label(self.sort_column == SortColumn::LastPlayed, label).clicked() {
                        self.set_sort(SortColumn::LastPlayed);
                    }
                });
                header.col(|ui| {
                    let indicator = self.sort_indicator(SortColumn::Playtime);
                    let label = if indicator.is_empty() { "Playtime".to_string() } else { format!("Playtime {}", indicator) };
                    if ui.selectable_label(self.sort_column == SortColumn::Playtime, label).clicked() {
                        self.set_sort(SortColumn::Playtime);
                    }
                });
                header.col(|ui| {
                    let indicator = self.sort_indicator(SortColumn::AchievementsTotal);
                    let label = if indicator.is_empty() { "Achievements".to_string() } else { format!("Achievements {}", indicator) };
                    if ui.selectable_label(self.sort_column == SortColumn::AchievementsTotal, label).clicked() {
                        self.set_sort(SortColumn::AchievementsTotal);
                    }
                });
                header.col(|ui| {
                    let indicator = self.sort_indicator(SortColumn::AchievementsPercent);
                    let label = if indicator.is_empty() { "%".to_string() } else { format!("% {}", indicator) };
                    if ui.selectable_label(self.sort_column == SortColumn::AchievementsPercent, label).clicked() {
                        self.set_sort(SortColumn::AchievementsPercent);
                    }
                });
            })
            .body(|body| {
                body.heterogeneous_rows(row_heights.into_iter(), |mut row| {
                    let row_idx = row.index();
                    let game_idx = filtered_indices[row_idx];
                    let game = &self.games[game_idx];
                    let appid = game.appid;
                    let is_expanded = self.expanded_rows.contains(&appid);
                    let has_achievements = game.achievements_total.map(|t| t > 0).unwrap_or(false);
                    
                    // Name with expand/collapse toggle
                    row.col(|ui| {
                        self.render_name_cell(ui, game_idx, is_expanded, has_achievements, &mut needs_fetch);
                    });
                    
                    // Only show other columns if not expanded
                    row.col(|ui| {
                        if !is_expanded {
                            if let Some(ts) = self.games[game_idx].rtime_last_played {
                                if ts > 0 {
                                    ui.label(format_timestamp(ts));
                                } else {
                                    ui.label("—");
                                }
                            } else {
                                ui.label("—");
                            }
                        }
                    });
                    
                    row.col(|ui| {
                        if !is_expanded {
                            let hours = self.games[game_idx].playtime_forever as f64 / 60.0;
                            ui.label(format!("{:.1}h", hours));
                        }
                    });
                    
                    row.col(|ui| {
                        if !is_expanded {
                            ui.label(self.games[game_idx].achievements_display());
                        }
                    });
                    
                    row.col(|ui| {
                        if !is_expanded {
                            if let Some(pct) = self.games[game_idx].completion_percent() {
                                let color = if pct >= 100.0 {
                                    egui::Color32::from_rgb(100, 255, 100)
                                } else if pct >= 50.0 {
                                    egui::Color32::from_rgb(255, 215, 0)
                                } else {
                                    egui::Color32::GRAY
                                };
                                ui.label(egui::RichText::new(format!("{:.0}%", pct)).color(color));
                            } else {
                                ui.label("—");
                            }
                        }
                    });
                });
            });
        
        // Fetch achievements for any rows that need them
        if let Some(client) = &self.ws_client {
            for appid in needs_fetch {
                client.fetch_achievements(appid);
            }
        }
    }
    
    fn render_name_cell(&mut self, ui: &mut egui::Ui, game_idx: usize, is_expanded: bool, has_achievements: bool, needs_fetch: &mut Vec<u64>) {
        let game = &self.games[game_idx];
        let appid = game.appid;
        let game_name = game.name.clone();
        let img_icon_url = game.img_icon_url.clone();
        
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                // Expand/collapse button for games with achievements
                if has_achievements {
                    let icon = if is_expanded { 
                        regular::CARET_DOWN 
                    } else { 
                        regular::CARET_RIGHT 
                    };
                    if ui.small_button(icon.to_string()).clicked() {
                        if is_expanded {
                            self.expanded_rows.remove(&appid);
                        } else {
                            self.expanded_rows.insert(appid);
                            // Load achievements if not cached
                            if !self.achievements_cache.contains_key(&appid) {
                                needs_fetch.push(appid);
                            }
                        }
                    }
                } else {
                    ui.add_space(20.0);
                }
                
                // Show game icon when expanded
                if is_expanded {
                    if let Some(icon_hash) = &img_icon_url {
                        if !icon_hash.is_empty() {
                            let icon_url = game_icon_url(appid, icon_hash);
                            ui.add(
                                egui::Image::new(icon_url)
                                    .fit_to_exact_size(egui::vec2(32.0, 32.0))
                                    .corner_radius(4.0)
                            );
                        }
                    }
                    ui.label(egui::RichText::new(&game_name).strong());
                } else {
                    ui.label(&game_name);
                }
            });
            
            // Show achievements list if expanded
            if is_expanded {
                self.render_achievements_list(ui, appid);
            }
        });
    }
    
    fn render_achievements_list(&self, ui: &mut egui::Ui, appid: u64) {
        if let Some(achievements) = self.achievements_cache.get(&appid) {
            ui.add_space(4.0);
            ui.separator();
            
            // Sort achievements: unlocked first (by unlock time desc), then locked
            let mut sorted_achs: Vec<_> = achievements.iter().collect();
            sorted_achs.sort_by(|a, b| {
                match (a.achieved, b.achieved) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    (true, true) => b.unlocktime.cmp(&a.unlocktime),
                    (false, false) => a.name.cmp(&b.name),
                }
            });
            
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (i, ach) in sorted_achs.iter().enumerate() {
                    let icon_url = if ach.achieved {
                        proxy_steam_image_url(&ach.icon)
                    } else {
                        proxy_steam_image_url(&ach.icon_gray)
                    };
                    
                    // Alternate row background
                    let row_rect = ui.available_rect_before_wrap();
                    let row_rect = egui::Rect::from_min_size(
                        row_rect.min,
                        egui::vec2(row_rect.width(), 52.0)
                    );
                    if i % 2 == 1 {
                        ui.painter().rect_filled(
                            row_rect,
                            0.0,
                            ui.visuals().faint_bg_color
                        );
                    }
                    
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Image::new(icon_url.as_str())
                                .fit_to_exact_size(egui::vec2(64.0, 64.0))
                                .corner_radius(4.0)
                        );
                        
                        let name_text = if ach.achieved {
                            egui::RichText::new(&ach.name).color(egui::Color32::WHITE)
                        } else {
                            egui::RichText::new(&ach.name).color(egui::Color32::DARK_GRAY)
                        };
                        
                        let description_text = ach.description.as_deref().unwrap_or("");
                        let desc_color = if ach.achieved {
                            egui::Color32::GRAY
                        } else {
                            egui::Color32::from_rgb(80, 80, 80)
                        };
                        
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            // Top row: name and date
                            ui.horizontal(|ui| {
                                ui.label(name_text);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if let Some(unlock_dt) = &ach.unlocktime {
                                        ui.label(
                                            egui::RichText::new(unlock_dt.format("%Y-%m-%d").to_string())
                                                .color(egui::Color32::from_rgb(100, 200, 100))
                                        );
                                    }
                                });
                            });
                            // Description below
                            if !description_text.is_empty() {
                                ui.label(egui::RichText::new(description_text).color(desc_color));
                            }
                        });
                    });
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Loading achievements...");
            });
        }
    }
    
}

// ============================================================================
// Helper Functions
// ============================================================================

fn get_token_from_url() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|search| {
            search.strip_prefix('?')
                .and_then(|s| {
                    s.split('&')
                        .find(|p| p.starts_with("token="))
                        .map(|p| p.strip_prefix("token=").unwrap_or("").to_string())
                })
        })
        .filter(|t| !t.is_empty())
}

fn get_token_from_storage() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|storage| storage.get_item("overachiever_token").ok())
        .flatten()
}

fn save_token_to_storage(token: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("overachiever_token", token);
    }
}

fn clear_token_from_storage() {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.remove_item("overachiever_token");
    }
}

fn get_ws_url_from_location() -> String {
    web_sys::window()
        .and_then(|w| {
            let location = w.location();
            let protocol = location.protocol().ok()?;
            let host = location.host().ok()?;
            let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };
            Some(format!("{}//{}/ws", ws_protocol, host))
        })
        .unwrap_or_else(|| "wss://overachiever.space/ws".to_string())
}

fn get_auth_url() -> String {
    web_sys::window()
        .and_then(|w| {
            let location = w.location();
            let origin = location.origin().ok()?;
            Some(format!("{}/auth/steam", origin))
        })
        .unwrap_or_else(|| "/auth/steam".to_string())
}

fn format_timestamp(ts: u32) -> String {
    use chrono::{TimeZone, Utc};
    if let Some(dt) = Utc.timestamp_opt(ts as i64, 0).single() {
        dt.format("%Y-%m-%d").to_string()
    } else {
        "—".to_string()
    }
}

/// Convert Steam CDN URLs to proxied URLs to avoid CORS issues
/// Handles both steamcdn-a.akamaihd.net and media.steampowered.com URLs
fn proxy_steam_image_url(url: &str) -> String {
    // Get the current origin for relative URLs
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    
    if url.contains("steamcdn-a.akamaihd.net") {
        // https://steamcdn-a.akamaihd.net/steamcommunity/public/images/apps/...
        // -> /steam-media/steamcommunity/public/images/apps/...
        if let Some(path) = url.strip_prefix("https://steamcdn-a.akamaihd.net/") {
            return format!("{}/steam-media/{}", origin, path);
        }
        if let Some(path) = url.strip_prefix("http://steamcdn-a.akamaihd.net/") {
            return format!("{}/steam-media/{}", origin, path);
        }
    }
    
    if url.contains("media.steampowered.com") {
        // https://media.steampowered.com/steamcommunity/public/images/apps/...
        // -> /steam-media/steamcommunity/public/images/apps/...
        if let Some(path) = url.strip_prefix("https://media.steampowered.com/") {
            return format!("{}/steam-media/{}", origin, path);
        }
        if let Some(path) = url.strip_prefix("http://media.steampowered.com/") {
            return format!("{}/steam-media/{}", origin, path);
        }
    }
    
    // Return original URL if not a Steam CDN URL
    url.to_string()
}

/// Build a game icon URL using the proxy
/// Game icons are at: media.steampowered.com/steamcommunity/public/images/apps/{appid}/{hash}.jpg
fn game_icon_url(appid: u64, icon_hash: &str) -> String {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    // Use steam-media proxy which routes to steamcdn-a.akamaihd.net
    format!("{}/steam-media/steamcommunity/public/images/apps/{}/{}.jpg", origin, appid, icon_hash)
}

/// Build a game icon URL from appid and hash (alias for use in render_log)
fn game_icon_url_from_hash(appid: u64, icon_hash: &str) -> String {
    game_icon_url(appid, icon_hash)
}
