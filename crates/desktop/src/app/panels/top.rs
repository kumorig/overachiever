//! Top toolbar panel - Update/Full Scan buttons and status

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::{DataMode, GdprConsent};

use crate::app::SteamOverachieverApp;
use crate::cloud_sync::CloudSyncState;

impl SteamOverachieverApp {
    pub(crate) fn render_top_panel(&mut self, ctx: &egui::Context) {
        let is_busy = self.state.is_busy();
        
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Overachiever v3");
                ui.separator();
                
                // Update button - for recently played games
                let update_button = egui::Button::new(format!("{} Update", regular::ARROWS_CLOCKWISE));
                let update_response = ui.add_enabled(!is_busy && self.config.is_valid(), update_button);
                
                // Show warning if update is stale
                if self.is_update_stale() && !is_busy {
                    update_response.clone().on_hover_text(
                        "⚠️ Last update was more than 2 weeks ago.\nThe recently played API only shows games from the last 2 weeks.\nConsider running a Full Scan instead."
                    );
                }
                
                if !self.config.is_valid() {
                    update_response.clone().on_hover_text(
                        "Please configure Steam API Key and Steam ID in Settings (⚙)"
                    );
                }
                
                if update_response.clicked() {
                    self.start_update();
                }
                
                // Full Scan button - scrapes achievements for all games not yet scraped
                let needs_scrape = self.games_needing_scrape();
                let full_scan_label = if needs_scrape > 0 {
                    format!("{} Full Scan ({})", regular::GAME_CONTROLLER, needs_scrape)
                } else {
                    format!("{} Full Scan", regular::GAME_CONTROLLER)
                };
                let can_scan = (needs_scrape > 0 || self.force_full_scan) && self.config.is_valid();
                if ui.add_enabled(!is_busy && can_scan, egui::Button::new(full_scan_label)).clicked() {
                    self.start_scrape();
                }
                
                ui.checkbox(&mut self.force_full_scan, "Force");
                
                ui.separator();
                
                if is_busy {
                    ui.spinner();
                    ui.add(egui::ProgressBar::new(self.state.progress())
                        .text(&self.status)
                        .animate(true));
                } else {
                    ui.label(&self.status);
                }
                
                // Settings cog button on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(regular::GEAR).on_hover_text("Settings").clicked() {
                        self.show_settings = true;
                    }
                    
                    // GDPR button - only show for hybrid/remote mode and if consent has been set
                    if self.config.data_mode.requires_server() && self.config.gdpr_consent.is_set() {
                        if ui.button(regular::SHIELD_CHECK).on_hover_text("Privacy Settings").clicked() {
                            self.show_gdpr_dialog = true;
                        }
                    }
                });
            });
        });
        
        // Settings window
        self.render_settings_window(ctx);
    }
    
    fn render_settings_window(&mut self, ctx: &egui::Context) {
        let mut show_settings = self.show_settings;
        
        egui::Window::new(format!("{} Settings", regular::GEAR))
            .open(&mut show_settings)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("'Overachiever' is in no way affiliated with or endorsed by Valve Corporation.")
                            .color(egui::Color32::GRAY)
                    );
                    
                    ui.add_space(12.0);
                    
                    // Data Mode selection
                    ui.horizontal(|ui| {
                        ui.label("Data Mode:");
                        egui::ComboBox::from_id_salt("data_mode")
                            .selected_text(self.config.data_mode.label())
                            .show_ui(ui, |ui| {
                                if ui.selectable_value(&mut self.config.data_mode, DataMode::Local, DataMode::Local.label()).changed() {
                                    let _ = self.config.save();
                                }
                                if ui.selectable_value(&mut self.config.data_mode, DataMode::Hybrid, DataMode::Hybrid.label()).changed() {
                                    let _ = self.config.save();
                                }
                                if ui.selectable_value(&mut self.config.data_mode, DataMode::Remote, DataMode::Remote.label()).changed() {
                                    let _ = self.config.save();
                                }
                            });
                    });
                    
                    ui.label(
                        egui::RichText::new(self.config.data_mode.description())
                            .color(egui::Color32::GRAY)
                            .small()
                    );
                    
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);
                    
                    // Steam credentials (for Local/Hybrid modes)
                    let needs_steam_creds = self.config.data_mode.requires_api_key();
                    
                    ui.add_enabled_ui(needs_steam_creds, |ui| {
                        ui.heading("Steam Credentials");
                        if !needs_steam_creds {
                            ui.label(egui::RichText::new("Not required for Cloud mode").color(egui::Color32::GRAY));
                        }
                        
                        ui.add_space(8.0);
                        
                        ui.horizontal(|ui| {
                            ui.label("Steam ID:");
                            ui.add_space(20.0);
                            if ui.add(
                                egui::TextEdit::singleline(&mut self.config.steam_id)
                                    .desired_width(180.0)
                                    .hint_text("12345678901234567")
                            ).changed() {
                                let _ = self.config.save();
                            }
                        });
                        
                        ui.add_space(8.0);
                        
                        ui.horizontal(|ui| {
                            ui.label("API Key:");
                            ui.add_space(28.0);
                            if ui.add(
                                egui::TextEdit::singleline(&mut self.config.steam_web_api_key)
                                    .desired_width(180.0)
                                    .password(true)
                                    .hint_text("Your Steam API key")
                            ).changed() {
                                let _ = self.config.save();
                            }
                        });
                        
                        ui.add_space(8.0);
                        
                        ui.horizontal(|ui| {
                            ui.hyperlink_to(
                                format!("{} Get API Key", regular::LINK),
                                "https://steamcommunity.com/dev/apikey"
                            );
                            ui.label(
                                egui::RichText::new("(No affiliation)")
                                    .color(egui::Color32::GRAY)
                            );
                        });
                        
                        ui.horizontal(|ui| {
                            ui.hyperlink_to(
                                format!("{} Figure out Steam ID", regular::LINK),
                                "https://steamid.io"
                            );
                            ui.label(
                                egui::RichText::new("(No affiliation)")
                                    .color(egui::Color32::GRAY)
                            );
                        });
                    });
                    
                    // Server settings (for Hybrid/Remote modes)
                    if self.config.data_mode.requires_server() {
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);
                        
                        ui.heading("Server Settings");
                        
                        ui.add_space(8.0);
                        
                        ui.horizontal(|ui| {
                            ui.label("Server URL:");
                            if ui.add(
                                egui::TextEdit::singleline(&mut self.config.server_url)
                                    .desired_width(250.0)
                                    .hint_text("wss://overachiever.example.com")
                            ).changed() {
                                let _ = self.config.save();
                            }
                        });
                        
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Server connection not yet implemented").color(egui::Color32::YELLOW));
                    }
                    
                    ui.add_space(12.0);
                    
                    // Validation status
                    if !self.config.is_valid() {
                        let msg = match self.config.data_mode {
                            DataMode::Local | DataMode::Hybrid => "Steam ID and API Key are required",
                            DataMode::Remote => "Server URL is required",
                        };
                        ui.colored_label(egui::Color32::YELLOW, format!("{} {}", regular::WARNING, msg));
                    } else {
                        ui.colored_label(egui::Color32::GREEN, format!("{} Configuration valid", regular::CHECK));
                    }
                    
                    // Cloud Sync section
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);
                    
                    ui.heading(format!("{} Cloud Sync", regular::CLOUD));
                    
                    ui.add_space(8.0);
                    
                    // Cloud status display
                    let cloud_state = self.cloud_sync_state.clone();
                    let is_linked = self.config.cloud_token.is_some();
                    
                    // Show status messages
                    match &cloud_state {
                        CloudSyncState::NotLinked => {
                            ui.label(egui::RichText::new(format!("{} Not linked", regular::CLOUD_SLASH)).color(egui::Color32::GRAY));
                        }
                        CloudSyncState::Linking => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Waiting for Steam login... (check your browser)");
                            });
                        }
                        CloudSyncState::Idle => {
                            ui.label(egui::RichText::new(format!("{} Linked", regular::CHECK)).color(egui::Color32::GREEN));
                        }
                        CloudSyncState::Checking => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Checking...");
                            });
                        }
                        CloudSyncState::Uploading => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Uploading...");
                            });
                        }
                        CloudSyncState::Downloading => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Downloading...");
                            });
                        }
                        CloudSyncState::Deleting => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Deleting...");
                            });
                        }
                        CloudSyncState::Success(msg) => {
                            ui.colored_label(egui::Color32::GREEN, format!("{} {}", regular::CHECK, msg));
                        }
                        CloudSyncState::Error(msg) => {
                            ui.colored_label(egui::Color32::RED, format!("{} {}", regular::WARNING, msg));
                        }
                    }
                    
                    ui.add_space(8.0);
                    
                    // Buttons
                    let is_busy = matches!(cloud_state, CloudSyncState::Checking | CloudSyncState::Uploading | CloudSyncState::Downloading | CloudSyncState::Deleting | CloudSyncState::Linking);
                    
                    let mut link_clicked = false;
                    let mut unlink_clicked = false;
                    let mut upload_clicked = false;
                    let mut download_clicked = false;
                    let mut delete_clicked = false;
                    
                    if !is_linked {
                        // Not linked - show link button
                        if ui.add_enabled(!is_busy, egui::Button::new(format!("{} Link with Steam", regular::STEAM_LOGO))).clicked() {
                            link_clicked = true;
                        }
                    } else {
                        // Linked - show action buttons
                        if ui.add_enabled(!is_busy, egui::Button::new(format!("{} Upload data to overachiever.space", regular::CLOUD_ARROW_UP))).clicked() {
                            upload_clicked = true;
                        }
                        if ui.add_enabled(!is_busy, egui::Button::new(format!("{} Download data from overachiever.space", regular::CLOUD_ARROW_DOWN))).clicked() {
                            download_clicked = true;
                        }
                        if ui.add_enabled(!is_busy, egui::Button::new(format!("{} Remove data from overachiever.space", regular::TRASH))).clicked() {
                            delete_clicked = true;
                        }
                        
                        ui.add_space(4.0);
                        if ui.add_enabled(!is_busy, egui::Button::new(format!("{} Unlink account", regular::LINK_BREAK))).clicked() {
                            unlink_clicked = true;
                        }
                    }
                    
                    // Handle clicks - set pending action for confirmation
                    if link_clicked {
                        self.start_cloud_link();
                    }
                    if unlink_clicked {
                        self.unlink_cloud();
                    }
                    if upload_clicked {
                        self.pending_cloud_action = Some(crate::app::CloudAction::Upload);
                    }
                    if download_clicked {
                        self.pending_cloud_action = Some(crate::app::CloudAction::Download);
                    }
                    if delete_clicked {
                        self.pending_cloud_action = Some(crate::app::CloudAction::Delete);
                    }
                });
            });
        
        self.show_settings = show_settings;
        
        // Render cloud action confirmation dialog
        self.render_cloud_confirm_dialog(ctx);
    }
    
    /// Render confirmation dialog for cloud actions
    fn render_cloud_confirm_dialog(&mut self, ctx: &egui::Context) {
        use crate::app::CloudAction;
        
        let pending = self.pending_cloud_action.clone();
        if pending.is_none() {
            return;
        }
        let action = pending.unwrap();
        
        let (title, message, confirm_text) = match &action {
            CloudAction::Upload => (
                "Upload to Cloud",
                "This will upload all your local data to overachiever.space.\nAny existing cloud data will be replaced.",
                "Upload"
            ),
            CloudAction::Download => (
                "Download from Cloud", 
                "This will download data from overachiever.space and replace your local data.",
                "Download"
            ),
            CloudAction::Delete => (
                "Remove from Cloud",
                "This will permanently delete all your data from overachiever.space.\nYour local data will not be affected.",
                "Delete"
            ),
        };
        
        let mut confirmed = false;
        let mut cancelled = false;
        
        egui::Window::new(format!("{} {}", regular::WARNING, title))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(message);
                ui.add_space(16.0);
                
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancelled = true;
                    }
                    if ui.button(confirm_text).clicked() {
                        confirmed = true;
                    }
                });
            });
        
        if cancelled {
            self.pending_cloud_action = None;
        }
        if confirmed {
            self.pending_cloud_action = None;
            match action {
                CloudAction::Upload => self.upload_to_cloud(),
                CloudAction::Download => self.download_from_cloud(),
                CloudAction::Delete => self.delete_from_cloud(),
            }
        }
    }
    
    /// Render GDPR modal for hybrid/remote modes
    pub(crate) fn render_gdpr_modal(&mut self, ctx: &egui::Context) {
        // Only show for hybrid/remote mode
        if !self.config.data_mode.requires_server() {
            return;
        }
        
        // If consent is already set and dialog not explicitly opened, don't show
        if self.config.gdpr_consent.is_set() && !self.show_gdpr_dialog {
            return;
        }
        
        // Semi-transparent backdrop
        let screen_rect = ctx.input(|i| i.viewport().inner_rect.unwrap_or(egui::Rect::NOTHING));
        egui::Area::new(egui::Id::new("gdpr_backdrop"))
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                let painter = ui.painter();
                painter.rect_filled(
                    screen_rect,
                    0.0,
                    egui::Color32::from_black_alpha(180),
                );
            });
        
        // Modal window
        egui::Window::new(format!("{} Privacy & Data Usage", regular::SHIELD_CHECK))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([450.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    
                    ui.label("This application processes personal data to provide its services:");
                    
                    ui.add_space(12.0);
                    
                    // Data we collect section
                    ui.heading("Data We Process");
                    ui.add_space(4.0);
                    
                    egui::Frame::new()
                        .fill(ui.style().visuals.extreme_bg_color)
                        .corner_radius(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.label("• Your Steam ID (public identifier)");
                            ui.label("• Your Steam display name");
                            ui.label("• Your game library (via Steam API)");
                            ui.label("• Achievement data for your games");
                            ui.label("• Community ratings/tips you submit (Hybrid mode)");
                        });
                    
                    ui.add_space(12.0);
                    
                    // Purpose section
                    ui.heading("Purpose");
                    ui.add_space(4.0);
                    let purpose_text = match self.config.data_mode {
                        DataMode::Hybrid => "In Hybrid mode, your personal game data stays local. Only community ratings and tips you submit are synced to the server.",
                        DataMode::Remote => "In Cloud mode, all data is stored on the server associated with your Steam ID.",
                        DataMode::Local => "In Local mode, no data is sent to any server.",
                    };
                    ui.label(purpose_text);
                    
                    ui.add_space(12.0);
                    
                    // Third party section
                    ui.heading("Third Parties");
                    ui.add_space(4.0);
                    ui.label("We use the Steam Web API to fetch your public game and achievement data. No data is shared with other third parties.");
                    
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);
                    
                    // Show current status if already set
                    if self.config.gdpr_consent.is_set() {
                        let status = if self.config.gdpr_consent.is_accepted() {
                            egui::RichText::new(format!("{} Currently: Accepted", regular::CHECK)).color(egui::Color32::GREEN)
                        } else {
                            egui::RichText::new(format!("{} Currently: Declined", regular::X)).color(egui::Color32::YELLOW)
                        };
                        ui.label(status);
                        ui.add_space(8.0);
                    }
                    
                    // Buttons
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(format!("{} Accept", regular::CHECK))
                                .on_hover_text("Accept data processing and continue")
                                .clicked() 
                            {
                                self.config.gdpr_consent = GdprConsent::Accepted;
                                let _ = self.config.save();
                                self.show_gdpr_dialog = false;
                            }
                            
                            if ui.button(format!("{} Decline", regular::X))
                                .on_hover_text("Decline - server features will be disabled")
                                .clicked() 
                            {
                                self.config.gdpr_consent = GdprConsent::Declined;
                                let _ = self.config.save();
                                self.show_gdpr_dialog = false;
                            }
                            
                            // Close button if already set (reviewing settings)
                            if self.config.gdpr_consent.is_set() {
                                if ui.button(format!("{} Close", regular::X_CIRCLE))
                                    .on_hover_text("Close without changes")
                                    .clicked() 
                                {
                                    self.show_gdpr_dialog = false;
                                }
                            }
                        });
                    });
                    
                    ui.add_space(4.0);
                });
            });
    }
}
