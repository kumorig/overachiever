//! Top toolbar panel - Update/Full Scan buttons and status

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::{GdprConsent, DATA_HANDLING_DESCRIPTION, ENABLE_ADMIN_MODE};

use crate::app::SteamOverachieverApp;
use crate::cloud_sync::CloudSyncState;
use crate::ui::AppState;

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
                        if ui.button(format!("{} Stop TTB", regular::STOP)).clicked() {
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
                        if ui.button(format!("{} Stop Tags", regular::STOP)).clicked() {
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
                    
                    // GDPR button - show if consent has been set
                    if self.config.gdpr_consent.is_set() {
                        if ui.button(regular::SHIELD_CHECK).on_hover_text("Privacy Settings").clicked() {
                            self.show_gdpr_dialog = true;
                        }
                    }
                    
                    // User profile button - show shareable link if cloud linked
                    if let Some(short_id) = self.config.get_short_id() {
                        let profile_url = format!("https://overachiever.space/{}", short_id);
                        if ui.button(regular::USER)
                            .on_hover_text_at_pointer(format!("Open profile: {}", profile_url))
                            .clicked()
                        {
                            let _ = open::that(&profile_url);
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
    }
    
    fn render_settings_window(&mut self, ctx: &egui::Context) {
        use crate::app::SettingsTab;

        let mut show_settings = self.show_settings;

        // Lazily load available fonts when settings is first opened
        if show_settings && self.available_fonts.is_none() {
            let fonts = crate::fonts::get_installed_fonts();
            self.available_fonts = Some(fonts.keys().cloned().collect());
        }

        egui::Window::new(format!("{} Settings", regular::GEAR))
            .open(&mut show_settings)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(450.0)
            .show(ctx, |ui| {
                // Tab bar
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::General, format!("{} General", regular::SLIDERS));
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::Steam, format!("{} Steam", regular::STEAM_LOGO));
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::Cloud, format!("{} Cloud", regular::CLOUD));
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::Debug, format!("{} Debug", regular::BUG));
                });

                ui.separator();
                ui.add_space(8.0);

                match self.settings_tab {
                    SettingsTab::General => self.render_settings_general_tab(ui, ctx),
                    SettingsTab::Steam => self.render_settings_steam_tab(ui),
                    SettingsTab::Cloud => self.render_settings_cloud_tab(ui),
                    SettingsTab::Debug => self.render_settings_debug_tab(ui),
                }
            });

        self.show_settings = show_settings;

        // Render cloud action confirmation dialog
        self.render_cloud_confirm_dialog(ctx);
    }

    fn render_settings_general_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Appearance");
        ui.add_space(8.0);

        // Font family selection
        ui.horizontal(|ui| {
            ui.label("Font:");
            ui.add_space(16.0);

            let current_font = self.config.font_family.clone().unwrap_or_else(|| "Default".to_string());

            egui::ComboBox::from_id_salt("font_family")
                .selected_text(&current_font)
                .width(250.0)
                .show_ui(ui, |ui| {
                    // Default option
                    if ui.selectable_label(self.config.font_family.is_none(), "Default").clicked() {
                        self.config.font_family = None;
                        self.fonts_need_update = true;
                    }

                    ui.separator();

                    // System fonts
                    if let Some(fonts) = &self.available_fonts {
                        for font_name in fonts {
                            let is_selected = self.config.font_family.as_ref() == Some(font_name);
                            if ui.selectable_label(is_selected, font_name).clicked() {
                                self.config.font_family = Some(font_name.clone());
                                self.fonts_need_update = true;
                            }
                        }
                    }
                });
        });

        ui.add_space(8.0);

        // Font size with pending value (only applied on Save)
        ui.horizontal(|ui| {
            ui.label("Font Size:");
            ui.add(egui::DragValue::new(&mut self.pending_font_size)
                .range(8.0..=32.0)
                .speed(0.5)
                .suffix(" pt"));
        });

        ui.add_space(12.0);

        // Save button for font size
        let size_changed = (self.pending_font_size - self.config.font_size).abs() > 0.01;
        ui.horizontal(|ui| {
            if ui.add_enabled(size_changed, egui::Button::new(format!("{} Save Font Size", regular::FLOPPY_DISK))).clicked() {
                self.config.font_size = self.pending_font_size;
                let _ = self.config.save();
                self.fonts_need_update = true;
            }
            if size_changed {
                ui.label(egui::RichText::new("(unsaved)").color(egui::Color32::YELLOW).small());
            }
        });

        // Apply font changes live (for font family changes)
        if self.fonts_need_update {
            self.fonts_need_update = false;
            apply_font_settings(ctx, &self.config);
            let _ = self.config.save();
        }

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        // Data handling description
        ui.heading("How Data is Handled");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(DATA_HANDLING_DESCRIPTION)
                .color(egui::Color32::GRAY)
        );

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("'Overachiever' is in no way affiliated with or endorsed by Valve Corporation.")
                .color(egui::Color32::GRAY)
        );
    }

    fn render_settings_steam_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Steam Credentials");
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

        ui.add_space(12.0);

        // Validation status
        if !self.config.is_valid() {
            ui.colored_label(egui::Color32::YELLOW, format!("{} Steam ID and API Key are required", regular::WARNING));
        } else {
            ui.colored_label(egui::Color32::GREEN, format!("{} Configuration valid", regular::CHECK));
        }
    }

    fn render_settings_cloud_tab(&mut self, ui: &mut egui::Ui) {
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
            CloudSyncState::Uploading(progress) => {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        if progress.total_bytes > 0 {
                            let mb_total = progress.total_bytes as f64 / (1024.0 * 1024.0);
                            if progress.bytes_sent >= progress.total_bytes {
                                ui.label(format!("Uploaded {:.2} MB", mb_total));
                            } else {
                                ui.label(format!("Uploading {:.2} MB...", mb_total));
                            }
                        } else {
                            ui.label("Preparing upload...");
                        }
                    });
                    if progress.total_bytes > 0 {
                        let fraction = progress.bytes_sent as f32 / progress.total_bytes as f32;
                        ui.add(egui::ProgressBar::new(fraction).desired_width(200.0).animate(progress.bytes_sent < progress.total_bytes));
                    }
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
        let is_busy = matches!(cloud_state, CloudSyncState::Checking | CloudSyncState::Uploading(_) | CloudSyncState::Downloading | CloudSyncState::Deleting | CloudSyncState::Linking);

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
    }

    fn render_settings_debug_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(format!("{} Debug", regular::BUG));
        ui.add_space(8.0);

        if ui.checkbox(&mut self.config.debug_recently_played, "Log recently played response")
            .on_hover_text("When running Update, write the recently played API response to recently_played_debug.txt")
            .changed()
        {
            let _ = self.config.save();
        }
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
    
    /// Render GDPR modal
    pub(crate) fn render_gdpr_modal(&mut self, ctx: &egui::Context) {
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
                            ui.label("• Community ratings/tips you submit");
                        });
                    
                    ui.add_space(12.0);
                    
                    // Purpose section
                    ui.heading("Purpose");
                    ui.add_space(4.0);
                    ui.label("Your personal game data stays local on your computer. Only community ratings and tips you choose to submit are synced to overachiever.space.");
                    
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

/// Apply font settings to the egui context
fn apply_font_settings(ctx: &egui::Context, config: &crate::config::Config) {
    let mut fonts = egui::FontDefinitions::default();

    // Add phosphor icons
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    // If a custom font is selected, try to load it
    if let Some(font_name) = &config.font_family {
        let installed_fonts = crate::fonts::get_installed_fonts();
        if let Some(font_path) = installed_fonts.get(font_name) {
            if let Some(font_data) = crate::fonts::load_font_data(font_path) {
                // Add the custom font
                fonts.font_data.insert(
                    "custom_font".to_owned(),
                    egui::FontData::from_owned(font_data).into(),
                );

                // Make it the primary font for proportional text
                fonts.families.get_mut(&egui::FontFamily::Proportional)
                    .map(|family| family.insert(0, "custom_font".to_owned()));
            }
        }
    }

    ctx.set_fonts(fonts);

    // Apply font size via style
    let mut style = (*ctx.style()).clone();
    style.text_styles.iter_mut().for_each(|(text_style, font_id)| {
        // Scale font sizes based on the configured size (14.0 is default)
        let scale = config.font_size / 14.0;
        match text_style {
            egui::TextStyle::Small => font_id.size = 10.0 * scale,
            egui::TextStyle::Body => font_id.size = 14.0 * scale,
            egui::TextStyle::Monospace => font_id.size = 14.0 * scale,
            egui::TextStyle::Button => font_id.size = 14.0 * scale,
            egui::TextStyle::Heading => font_id.size = 20.0 * scale,
            egui::TextStyle::Name(_) => {}
        }
    });
    style.interaction.tooltip_delay = 0.0;
    ctx.set_style(style);
}
