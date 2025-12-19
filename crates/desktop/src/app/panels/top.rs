//! Top toolbar panel - Update/Full Scan buttons and status

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::DataMode;

use crate::app::SteamOverachieverApp;

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
                });
            });
        });
        
        // Settings window
        self.render_settings_window(ctx);
    }
    
    fn render_settings_window(&mut self, ctx: &egui::Context) {
        egui::Window::new(format!("{} Settings", regular::GEAR))
            .open(&mut self.show_settings)
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
                                    .hint_text("76561197960287930")
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
                });
            });
    }
}
