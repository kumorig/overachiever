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
        
        // Get server URL from localStorage or use default
        let server_url = get_server_url().unwrap_or_else(|| "wss://overachiever.space/ws".to_string());
        
        Self {
            server_url,
            ws_client: None,
            connection_state: ConnectionState::Disconnected,
            games: Vec::new(),
            run_history: Vec::new(),
            achievement_history: Vec::new(),
            log_entries: Vec::new(),
            status: "Not connected".to_string(),
            expanded_rows: HashSet::new(),
            achievements_cache: HashMap::new(),
            filter_name: String::new(),
            show_login: auth_token.is_none(),
            auth_token,
        }
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
                self.connection_state = ConnectionState::Connected;
                self.status = "Connected, authenticating...".to_string();
                
                // Authenticate if we have a token
                if let (Some(client), Some(token)) = (&self.ws_client, &self.auth_token) {
                    client.authenticate(token);
                }
            }
            Err(e) => {
                self.connection_state = ConnectionState::Error(e.clone());
                self.status = format!("Connection failed: {}", e);
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
                    self.connection_state = ConnectionState::Error(reason.clone());
                    self.status = format!("Auth failed: {}", reason);
                    self.show_login = true;
                }
                overachiever_core::ServerMessage::Games { games } => {
                    self.games = games;
                    self.status = format!("Loaded {} games", self.games.len());
                }
                overachiever_core::ServerMessage::Achievements { appid, achievements } => {
                    self.achievements_cache.insert(appid, achievements);
                }
                overachiever_core::ServerMessage::Error { message } => {
                    self.status = format!("Error: {}", message);
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for WasmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for incoming WebSocket messages
        self.check_messages();
        
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
                        if ui.button(format!("{} Connect", regular::PLUGS)).clicked() {
                            self.connect();
                        }
                    }
                    ConnectionState::Connecting => {
                        ui.spinner();
                        ui.label("Connecting...");
                    }
                    ConnectionState::Connected => {
                        ui.label("Connected, waiting for auth...");
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
        
        // Login/Settings window
        if self.show_login {
            let mut close_window = false;
            egui::Window::new("Settings")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Server URL:");
                        ui.text_edit_singleline(&mut self.server_url);
                    });
                    
                    ui.add_space(8.0);
                    
                    if matches!(self.connection_state, ConnectionState::Disconnected | ConnectionState::Error(_)) {
                        if ui.button("Connect").clicked() {
                            save_server_url(&self.server_url);
                            self.connect();
                            close_window = true;
                        }
                    }
                    
                    ui.add_space(8.0);
                    
                    if matches!(self.connection_state, ConnectionState::Connected) {
                        ui.label("Please log in with Steam:");
                        if ui.button(format!("{} Login with Steam", regular::STEAM_LOGO)).clicked() {
                            // Open Steam login in new window
                            let login_url = self.server_url.replace("/ws", "/auth/steam");
                            let _ = web_sys::window()
                                .and_then(|w| w.open_with_url(&login_url).ok());
                        }
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
                    if matches!(self.connection_state, ConnectionState::Authenticated(_)) {
                        ui.spinner();
                        ui.label("Loading games...");
                    } else {
                        ui.label("Connect and log in to view your games");
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

fn get_server_url() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|storage| storage.get_item("overachiever_server_url").ok())
        .flatten()
}

fn save_server_url(url: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("overachiever_server_url", url);
    }
}
