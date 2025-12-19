//! WASM App state and UI

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::{Game, GameAchievement, RunHistory, AchievementHistory, LogEntry, UserProfile};
use std::collections::{HashMap, HashSet};

use crate::ws_client::WsClient;

#[derive(Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Authenticated(UserProfile),
    Error(String),
}

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
    expanded_rows: HashSet<u64>,
    achievements_cache: HashMap<u64, Vec<GameAchievement>>,
    filter_name: String,
    show_login: bool,
    
    // Token from URL or storage
    auth_token: Option<String>,
}

impl WasmApp {
    pub fn new() -> Self {
        // Try to get token from URL params or localStorage
        let auth_token = get_token_from_url().or_else(|| get_token_from_storage());
        
        // Auto-detect WebSocket URL from current page location
        let server_url = get_ws_url_from_location();
        
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
            expanded_rows: HashSet::new(),
            achievements_cache: HashMap::new(),
            filter_name: String::new(),
            show_login: false, // Don't show login window on startup - auto connect
            auth_token,
        };
        
        // Auto-connect on startup
        app.connect();
        app
    }
    
    fn connect(&mut self) {
        if self.connection_state != ConnectionState::Disconnected {
            return;
        }
        
        self.connection_state = ConnectionState::Connecting;
        self.status = "Connecting...".to_string();
        
        // Create WebSocket connection
        match WsClient::new(&self.server_url) {
            Ok(client) => {
                self.ws_client = Some(client);
                // Stay in Connecting state until WS is actually open
                // check_ws_state() will transition to Connected when ready
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
                    // WebSocket is now open, transition to Connected and authenticate
                    if self.connection_state == ConnectionState::Connecting {
                        self.connection_state = ConnectionState::Connected;
                        self.status = "Connected, authenticating...".to_string();
                        
                        // Authenticate if we have a token
                        if let Some(token) = &self.auth_token.clone() {
                            client.authenticate(token);
                        } else {
                            // No token - need to login
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
            log(&format!("Received message: {:?}", msg));
            match msg {
                overachiever_core::ServerMessage::Authenticated { user } => {
                    log(&format!("Authenticated as: {}", user.display_name));
                    self.connection_state = ConnectionState::Authenticated(user.clone());
                    self.status = format!("Logged in as {}", user.display_name);
                    
                    // Save token to localStorage
                    if let Some(token) = &self.auth_token {
                        save_token_to_storage(token);
                    }
                    
                    // Fetch games
                    if let Some(client) = &self.ws_client {
                        client.fetch_games();
                    }
                }
                overachiever_core::ServerMessage::AuthError { reason } => {
                    log(&format!("Auth error: {}", reason));
                    self.connection_state = ConnectionState::Error(reason.clone());
                    self.status = format!("Auth failed: {}", reason);
                    self.show_login = true;
                }
                overachiever_core::ServerMessage::Games { games } => {
                    log(&format!("Received {} games", games.len()));
                    self.games = games;
                    self.games_loaded = true;
                    self.status = format!("Loaded {} games", self.games.len());
                }
                overachiever_core::ServerMessage::Achievements { appid, achievements } => {
                    self.achievements_cache.insert(appid, achievements);
                }
                overachiever_core::ServerMessage::Error { message } => {
                    log(&format!("Server error: {}", message));
                    self.status = format!("Error: {}", message);
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for WasmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check WebSocket state transitions
        self.check_ws_state();
        
        // Check for incoming WebSocket messages
        self.check_messages();
        
        // Auto-reconnect if disconnected
        if matches!(self.connection_state, ConnectionState::Disconnected) {
            self.connect();
        }
        
        // Request repaint to keep checking for messages
        ctx.request_repaint();
        
        // Top panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Overachiever (Web)");
                ui.separator();
                
                // Connection status
                match &self.connection_state {
                    ConnectionState::Disconnected => {
                        ui.spinner();
                        ui.label("Reconnecting...");
                        // Auto-reconnect when disconnected
                        // (will be handled by next frame)
                    }
                    ConnectionState::Connecting => {
                        ui.spinner();
                        ui.label("Connecting...");
                    }
                    ConnectionState::Connected => {
                        ui.spinner();
                        ui.label("Authenticating...");
                    }
                    ConnectionState::Authenticated(user) => {
                        ui.label(format!("{} {}", regular::USER, user.display_name));
                        
                        // Sync button
                        if ui.button(format!("{} Sync", regular::ARROWS_CLOCKWISE)).clicked() {
                            if let Some(client) = &self.ws_client {
                                client.sync_from_steam();
                            }
                        }
                    }
                    ConnectionState::Error(e) => {
                        ui.colored_label(egui::Color32::RED, format!("{} {}", regular::WARNING, e));
                        if ui.button("Retry").clicked() {
                            self.connection_state = ConnectionState::Disconnected;
                            self.connect();
                        }
                    }
                }
                
                ui.separator();
                ui.label(&self.status);
                
                // Settings on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(regular::GEAR).clicked() {
                        self.show_login = true;
                    }
                });
            });
        });
        
        // Login window - shown when connected but no token
        if self.show_login && matches!(self.connection_state, ConnectionState::Connected) {
            let mut close_window = false;
            egui::Window::new("Login Required")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Please log in with your Steam account to continue:");
                    
                    ui.add_space(12.0);
                    
                    if ui.button(format!("{} Login with Steam", regular::STEAM_LOGO)).clicked() {
                        // Open Steam login - same origin
                        let login_url = get_auth_url();
                        let _ = web_sys::window()
                            .and_then(|w| w.location().set_href(&login_url).ok());
                    }
                    
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        close_window = true;
                    }
                });
            if close_window {
                self.show_login = false;
            }
        }
        
        // Main content
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.games.is_empty() {
                ui.centered_and_justified(|ui| {
                    match &self.connection_state {
                        ConnectionState::Authenticated(user) => {
                            if !self.games_loaded {
                                ui.spinner();
                                ui.label("Loading games...");
                            } else {
                                // Games loaded but empty - need to sync from Steam
                                ui.vertical_centered(|ui| {
                                    ui.label(format!("Welcome, {}!", user.display_name));
                                    ui.add_space(8.0);
                                    ui.label("No games found. Sync your Steam library to get started.");
                                    ui.add_space(12.0);
                                    if ui.button(format!("{} Sync from Steam", regular::ARROWS_CLOCKWISE)).clicked() {
                                        if let Some(client) = &self.ws_client {
                                            client.sync_from_steam();
                                            self.status = "Syncing from Steam...".to_string();
                                        }
                                    }
                                });
                            }
                        }
                        ConnectionState::Connecting => {
                            ui.spinner();
                            ui.label("Connecting to server...");
                        }
                        ConnectionState::Connected => {
                            if self.auth_token.is_some() {
                                ui.spinner();
                                ui.label("Authenticating...");
                            } else {
                                ui.vertical_centered(|ui| {
                                    ui.label("Please log in with Steam to view your games");
                                    ui.add_space(8.0);
                                    if ui.button(format!("{} Login with Steam", regular::STEAM_LOGO)).clicked() {
                                        let login_url = get_auth_url();
                                        let _ = web_sys::window()
                                            .and_then(|w| w.location().set_href(&login_url).ok());
                                    }
                                });
                            }
                        }
                        ConnectionState::Error(e) => {
                            ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                        }
                        ConnectionState::Disconnected => {
                            ui.spinner();
                            ui.label("Reconnecting...");
                        }
                    }
                });
            } else {
                ui.heading(format!("Games Library ({} games)", self.games.len()));
                ui.separator();
                
                // Filter
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    ui.text_edit_singleline(&mut self.filter_name);
                });
                
                ui.add_space(4.0);
                
                // Simple games list
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let filter_lower = self.filter_name.to_lowercase();
                    for game in &self.games {
                        if !filter_lower.is_empty() && !game.name.to_lowercase().contains(&filter_lower) {
                            continue;
                        }
                        
                        ui.horizontal(|ui| {
                            ui.label(&game.name);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(game.achievements_display());
                                ui.label(format!("{:.1}h", game.playtime_forever as f64 / 60.0));
                            });
                        });
                        ui.separator();
                    }
                });
            }
        });
    }
}

// Helper functions for browser storage

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

/// Auto-detect WebSocket URL from current page location
/// If page is served from https://example.com, connects to wss://example.com/ws
/// If page is served from http://localhost:8080, connects to ws://localhost:8080/ws
fn get_ws_url_from_location() -> String {
    web_sys::window()
        .and_then(|w| {
            let location = w.location();
            let protocol = location.protocol().ok()?;
            let host = location.host().ok()?;
            
            // Use wss for https, ws for http
            let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };
            Some(format!("{}//{}/ws", ws_protocol, host))
        })
        .unwrap_or_else(|| "wss://overachiever.space/ws".to_string())
}

/// Get auth URL from current page location
fn get_auth_url() -> String {
    web_sys::window()
        .and_then(|w| {
            let location = w.location();
            let origin = location.origin().ok()?;
            Some(format!("{}/auth/steam", origin))
        })
        .unwrap_or_else(|| "/auth/steam".to_string())
}

/// Log to browser console
fn log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}
