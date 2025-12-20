//! WASM App state and UI - matches desktop version layout

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::{Game, GameAchievement, UserProfile, RunHistory, AchievementHistory, SyncState, LogEntry};
use overachiever_core::{StatsPanelPlatform, StatsPanelConfig, render_stats_content, render_log_content, SidebarPanel};
use overachiever_core::{GamesTablePlatform, SortColumn, SortOrder, TriFilter, sort_games, get_filtered_indices, render_filter_bar, render_games_table};
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
    sidebar_panel: SidebarPanel,
    
    // Token from URL or storage
    auth_token: Option<String>,
}

// ============================================================================
// StatsPanelPlatform Implementation
// ============================================================================

impl StatsPanelPlatform for WasmApp {
    fn games(&self) -> &[Game] {
        &self.games
    }
    
    fn run_history(&self) -> &[RunHistory] {
        &self.run_history
    }
    
    fn achievement_history(&self) -> &[AchievementHistory] {
        &self.achievement_history
    }
    
    fn log_entries(&self) -> &[LogEntry] {
        &self.log_entries
    }
    
    fn include_unplayed_in_avg(&self) -> bool {
        self.include_unplayed_in_avg
    }
    
    fn set_include_unplayed_in_avg(&mut self, value: bool) {
        self.include_unplayed_in_avg = value;
    }
    
    fn game_icon_source(&self, _ui: &egui::Ui, appid: u64, icon_hash: &str) -> egui::ImageSource<'static> {
        let url = game_icon_url(appid, icon_hash);
        egui::ImageSource::Uri(url.into())
    }
    
    fn achievement_icon_source(&self, _ui: &egui::Ui, icon_url: &str) -> egui::ImageSource<'static> {
        let proxied = proxy_steam_image_url(icon_url);
        egui::ImageSource::Uri(proxied.into())
    }
}

impl GamesTablePlatform for WasmApp {
    fn sort_column(&self) -> SortColumn {
        self.sort_column
    }
    
    fn sort_order(&self) -> SortOrder {
        self.sort_order
    }
    
    fn set_sort(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.sort_order = self.sort_order.toggle();
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Ascending;
        }
        sort_games(&mut self.games, self.sort_column, self.sort_order);
    }
    
    fn filter_name(&self) -> &str {
        &self.filter_name
    }
    
    fn set_filter_name(&mut self, name: String) {
        self.filter_name = name;
    }
    
    fn filter_achievements(&self) -> TriFilter {
        self.filter_achievements
    }
    
    fn set_filter_achievements(&mut self, filter: TriFilter) {
        self.filter_achievements = filter;
    }
    
    fn filter_playtime(&self) -> TriFilter {
        self.filter_playtime
    }
    
    fn set_filter_playtime(&mut self, filter: TriFilter) {
        self.filter_playtime = filter;
    }
    
    fn is_expanded(&self, appid: u64) -> bool {
        self.expanded_rows.contains(&appid)
    }
    
    fn toggle_expanded(&mut self, appid: u64) {
        if self.expanded_rows.contains(&appid) {
            self.expanded_rows.remove(&appid);
        } else {
            self.expanded_rows.insert(appid);
        }
    }
    
    fn get_cached_achievements(&self, appid: u64) -> Option<&Vec<GameAchievement>> {
        self.achievements_cache.get(&appid)
    }
    
    fn request_achievements(&mut self, appid: u64) {
        if let Some(client) = &self.ws_client {
            client.fetch_achievements(appid);
        }
    }
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
            sidebar_panel: SidebarPanel::Stats,
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
                    sort_games(&mut self.games, self.sort_column, self.sort_order);
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
    
    fn do_sort_games(&mut self) {
        sort_games(&mut self.games, self.sort_column, self.sort_order);
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
            // Collapsed sidebar - show two buttons (Stats and Log)
            egui::SidePanel::right("stats_panel_collapsed")
                .exact_width(36.0)
                .resizable(false)
                .frame(panel_frame)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    // Stats button
                    if ui.button(regular::CHART_LINE.to_string())
                        .on_hover_text("Open Stats Panel")
                        .clicked() 
                    {
                        self.sidebar_panel = SidebarPanel::Stats;
                        self.show_stats_panel = true;
                    }
                    // Log button
                    if ui.button(regular::SCROLL.to_string())
                        .on_hover_text("Open Log Panel")
                        .clicked()
                    {
                        self.sidebar_panel = SidebarPanel::Log;
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
            // Use min_width like desktop - panel content will fill this width
            // but won't force the panel to grow larger
            panel.min_width(320.0)
        };
        
        panel.show(ctx, |ui| {
                // Top navigation bar: close button + panel tabs
                ui.horizontal(|ui| {
                    // Close button (chevron right to collapse)
                    if ui.small_button(regular::CARET_RIGHT.to_string())
                        .on_hover_text("Close Panel")
                        .clicked() 
                    {
                        self.show_stats_panel = false;
                    }
                    
                    ui.separator();
                    
                    // Panel navigation tabs
                    let stats_selected = self.sidebar_panel == SidebarPanel::Stats;
                    let log_selected = self.sidebar_panel == SidebarPanel::Log;
                    
                    if ui.selectable_label(stats_selected, format!("{} Stats", regular::CHART_LINE)).clicked() {
                        self.sidebar_panel = SidebarPanel::Stats;
                    }
                    if ui.selectable_label(log_selected, format!("{} Log", regular::SCROLL)).clicked() {
                        self.sidebar_panel = SidebarPanel::Log;
                    }
                });
                ui.separator();
                
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.sidebar_panel {
                        SidebarPanel::Stats => {
                            let config = StatsPanelConfig::wasm();
                            render_stats_content(ui, self, &config);
                        }
                        SidebarPanel::Log => {
                            render_log_content(ui, self);
                        }
                    }
                });
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
            
            render_filter_bar(ui, self);
            ui.add_space(4.0);
            
            let filtered_indices = get_filtered_indices(self);
            let filtered_count = filtered_indices.len();
            
            if filtered_count != self.games.len() {
                ui.label(format!("Showing {} of {} games", filtered_count, self.games.len()));
            }
            
            let needs_fetch = render_games_table(ui, self, filtered_indices);
            
            // Fetch achievements for any rows that need them
            if let Some(client) = &self.ws_client {
                for appid in needs_fetch {
                    client.fetch_achievements(appid);
                }
            }
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
