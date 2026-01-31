//! Top toolbar panel - Update/Full Scan buttons and status

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::ENABLE_ADMIN_MODE;

use crate::ui::AppState;
use crate::app::SteamOverachieverApp;

// Build info embedded at compile time
const BUILD_NUMBER: &str = env!("BUILD_NUMBER");
const BUILD_DATETIME: &str = env!("BUILD_DATETIME");

impl SteamOverachieverApp {
    pub(crate) fn render_top_panel(&mut self, ctx: &egui::Context) {
        let is_busy = self.state.is_busy();
        
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let heading = ui.heading("Overachiever");
                heading.on_hover_text(format!(
                    "Build #{}\n{}",
                    BUILD_NUMBER,
                    BUILD_DATETIME
                ));
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

                // TTB Scan button - only show if admin_mode is enabled
                if self.admin_mode {
                    let is_ttb_scanning = matches!(self.state, AppState::TtbScanning { .. });
                    let needs_ttb = self.games_needing_ttb_admin();

                    if is_ttb_scanning {
                        // Show stop button during scan
                        if ui.button(format!("{} Stop TTB", regular::X_CIRCLE)).clicked() {
                            self.stop_ttb_scan();
                        }
                    } else {
                        let ttb_label = if needs_ttb > 0 {
                            format!("{} TTB Scan ({})", regular::CLOCK, needs_ttb)
                        } else {
                            format!("{} TTB Scan", regular::CLOCK)
                        };
                        let can_ttb = needs_ttb > 0 && self.config.is_valid();
                        if ui.add_enabled(!is_busy && can_ttb, egui::Button::new(ttb_label))
                            .on_hover_text("Scan HowLongToBeat for game completion times (1 game/minute)")
                            .clicked()
                        {
                            self.start_ttb_scan();
                        }
                    }

                    // Tags Scan button - bulk fetch tags from SteamSpy
                    let is_tags_scanning = matches!(self.state, AppState::TagsScanning { .. });
                    let needs_tags = self.games_needing_tags();

                    if is_tags_scanning {
                        // Show stop button during scan
                        if ui.button(format!("{} Stop Tags", regular::X_CIRCLE)).clicked() {
                            self.stop_tags_scan();
                        }
                    } else {
                        let tags_label = if needs_tags > 0 {
                            format!("{} Tags Scan ({})", regular::TAG, needs_tags)
                        } else {
                            format!("{} Tags Scan", regular::TAG)
                        };
                        let can_tags = needs_tags > 0 && self.config.is_valid();
                        let tags_tooltip = format!("Fetch game tags from SteamSpy (1 game/{}s)", self.config.tags_scan_delay_secs);
                        if ui.add_enabled(!is_busy && can_tags, egui::Button::new(tags_label))
                            .on_hover_text(tags_tooltip)
                            .clicked()
                        {
                            self.start_tags_scan();
                        }
                    }
                }

                ui.separator();

                // Reserve space for right-side buttons (settings, privacy, profile, admin)
                let right_buttons_width = 180.0;
                let available_for_status = (ui.available_width() - right_buttons_width).max(100.0);

                if is_busy {
                    ui.spinner();
                    ui.add(egui::ProgressBar::new(self.state.progress())
                        .text(&self.status)
                        .desired_width(available_for_status - 20.0) // 20px for spinner
                        .animate(true));
                } else {
                    ui.add(egui::Label::new(&self.status).truncate());
                }
                
                // Settings cog button on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(regular::GEAR).on_hover_text("Settings").clicked() {
                        self.show_settings = true;
                    }
                    
                    // User profile button - opens profile menu if cloud linked
                    if let Some(_short_id) = self.config.get_short_id() {
                        if ui.button(regular::USER)
                            .on_hover_text("Profile Menu")
                            .clicked()
                        {
                            self.show_profile_menu = !self.show_profile_menu;
                        }
                    } else {
                        // Steam login button - show when not logged in
                        let green = egui::Color32::from_rgb(62, 130, 61);
                        let steam_button = egui::Button::new(regular::STEAM_LOGO)
                            .fill(green);
                        if ui.add(steam_button)
                            .on_hover_text("Login with Steam")
                            .clicked()
                        {
                            self.start_cloud_link();
                        }
                    }

                    // Admin mode toggle - only show if ENABLE_ADMIN_MODE is true
                    if ENABLE_ADMIN_MODE {
                        let admin_icon = if self.admin_mode { regular::SHIELD_STAR } else { regular::SHIELD };
                        let admin_tooltip = if self.admin_mode {
                            "Admin Mode: ON\nClick to disable TTB scanning"
                        } else {
                            "Admin Mode: OFF\nClick to enable TTB scanning"
                        };
                        if ui.button(admin_icon)
                            .on_hover_text(admin_tooltip)
                            .clicked()
                        {
                            self.admin_mode = !self.admin_mode;
                        }
                    }
                });
            });
        });
        
        // Settings window
        self.render_settings_window(ctx);
        
        // Profile menu window
        self.render_profile_menu(ctx);
    }
}
