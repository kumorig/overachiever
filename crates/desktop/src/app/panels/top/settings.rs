//! Settings window and tabs

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::DATA_HANDLING_DESCRIPTION;

use crate::cloud_sync::CloudSyncState;
use crate::app::SteamOverachieverApp;
use super::fonts::apply_font_settings;

impl SteamOverachieverApp {
    pub(in crate::app) fn render_settings_window(&mut self, ctx: &egui::Context) {
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
}
