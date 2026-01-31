//! Settings window and tabs

use eframe::egui;
use egui_phosphor::regular;

use super::fonts::apply_font_settings;
use crate::app::SteamOverachieverApp;

impl SteamOverachieverApp {
    pub(in crate::app) fn render_settings_window(&mut self, ctx: &egui::Context) {
        use crate::app::SettingsTab;

        let mut show_settings = self.show_settings;

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
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::Debug, format!("{} Debug", regular::BUG));
                });

                ui.separator();
                ui.add_space(8.0);

                match self.settings_tab {
                    SettingsTab::General => self.render_settings_general_tab(ui, ctx),
                    SettingsTab::Steam => self.render_settings_steam_tab(ui),
                    SettingsTab::Debug => self.render_settings_debug_tab(ui),
                }
            });

        self.show_settings = show_settings;

        // Render cloud action confirmation dialog
        self.render_cloud_confirm_dialog(ctx);
    }

    fn render_settings_general_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use crate::config::FontSource;

        ui.heading("Appearance");
        ui.add_space(8.0);

        // Font source selection (radio buttons)
        ui.label("Font Source:");
        ui.add_space(4.0);

        let mut font_source_changed = false;

        ui.horizontal(|ui| {
            if ui.radio(self.config.font_source == FontSource::BuiltIn, "Built-in (egui default)").clicked() {
                self.config.font_source = FontSource::BuiltIn;
                font_source_changed = true;
            }
            if ui.radio(self.config.font_source == FontSource::Cjk, "CJK (Source Han Sans)").clicked() {
                self.config.font_source = FontSource::Cjk;
                font_source_changed = true;
            }
            if ui.radio(self.config.font_source == FontSource::System, "Windows Font").clicked() {
                self.config.font_source = FontSource::System;
                font_source_changed = true;
            }
        });

        ui.add_space(8.0);

        // Show CJK font download UI if CJK is selected
        if self.config.font_source == FontSource::Cjk {
            let is_downloaded = crate::cjk_font::is_font_downloaded();
            let is_downloading = self.cjk_font_download_receiver.is_some();

            if !is_downloaded && !is_downloading {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::YELLOW, format!("{} Font not downloaded", regular::WARNING));
                });
                ui.add_space(4.0);
                if ui.button(format!("{} Download Source Han Sans (~100 MB)", regular::DOWNLOAD_SIMPLE)).clicked() {
                    self.start_cjk_font_download();
                }
                ui.add_space(4.0);
                ui.hyperlink_to(
                    format!("{} View License", regular::LINK),
                    crate::cjk_font::get_license_url()
                );
            } else if is_downloading {
                if let Some(progress) = &self.cjk_font_download_progress {
                    match progress {
                        crate::cjk_font::DownloadProgress::Starting => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Starting download...");
                            });
                        }
                        crate::cjk_font::DownloadProgress::Downloading { bytes_downloaded, total_bytes } => {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    if let Some(total) = total_bytes {
                                        let mb_downloaded = *bytes_downloaded as f64 / (1024.0 * 1024.0);
                                        let mb_total = *total as f64 / (1024.0 * 1024.0);
                                        ui.label(format!("Downloading: {:.1} / {:.1} MB", mb_downloaded, mb_total));
                                    } else {
                                        let mb_downloaded = *bytes_downloaded as f64 / (1024.0 * 1024.0);
                                        ui.label(format!("Downloading: {:.1} MB", mb_downloaded));
                                    }
                                });
                                
                                // Show progress bar if we know the total size
                                if let Some(total) = total_bytes {
                                    if *total > 0 {
                                        let fraction = *bytes_downloaded as f32 / *total as f32;
                                        ui.add(
                                            egui::ProgressBar::new(fraction)
                                                .desired_width(300.0)
                                                .show_percentage()
                                                .animate(true)
                                        );
                                    }
                                }
                            });
                        }
                        crate::cjk_font::DownloadProgress::Extracting => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Extracting font...");
                            });
                        }
                        crate::cjk_font::DownloadProgress::Complete => {
                            ui.colored_label(egui::Color32::GREEN, format!("{} Download complete!", regular::CHECK));
                            // Clear the progress after showing for a bit
                            if self.cjk_font_download_receiver.is_none() {
                                self.cjk_font_download_progress = None;
                                font_source_changed = true; // Reload fonts
                            }
                        }
                        crate::cjk_font::DownloadProgress::Error(e) => {
                            ui.colored_label(egui::Color32::RED, format!("{} Download failed: {}", regular::WARNING, e));
                        }
                    }
                }
            } else {
                ui.colored_label(egui::Color32::GREEN, format!("{} Font downloaded", regular::CHECK));
            }

            ui.add_space(8.0);
            
            // Show weight selector if font is downloaded
            if is_downloaded {
                ui.horizontal(|ui| {
                    ui.label("Font Weight:");
                    ui.add_space(16.0);
                    
                    use crate::config::CjkFontWeight;
                    egui::ComboBox::from_id_salt("cjk_font_weight")
                        .selected_text(self.config.cjk_font_weight.display_name())
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            for weight in CjkFontWeight::all() {
                                if ui.selectable_label(self.config.cjk_font_weight == *weight, weight.display_name()).clicked() {
                                    self.config.cjk_font_weight = *weight;
                                    font_source_changed = true;
                                }
                            }
                        });
                });
                ui.add_space(8.0);
            }
        }

        // Show font picker only when System is selected
        if self.config.font_source == FontSource::System {
            // Lazily load available fonts when settings is first opened
            if self.available_fonts.is_none() {
                let fonts = crate::fonts::get_installed_fonts();
                self.available_fonts = Some(fonts.keys().cloned().collect());
            }

            ui.horizontal(|ui| {
                ui.label("Font:");
                ui.add_space(16.0);

                let current_font = self.config.system_font_name.clone().unwrap_or_else(|| "Select...".to_string());

                egui::ComboBox::from_id_salt("system_font_name")
                    .selected_text(&current_font)
                    .width(250.0)
                    .show_ui(ui, |ui| {
                        // System fonts
                        if let Some(fonts) = &self.available_fonts {
                            for font_name in fonts {
                                let is_selected = self.config.system_font_name.as_ref() == Some(font_name);
                                if ui.selectable_label(is_selected, font_name).clicked() {
                                    self.config.system_font_name = Some(font_name.clone());
                                    font_source_changed = true;
                                }
                            }
                        }
                    });
            });

            ui.add_space(8.0);
        }

        // Font size with pending value (only applied on Save)
        ui.horizontal(|ui| {
            ui.label("Font Size:");
            ui.add(egui::DragValue::new(&mut self.pending_font_size).range(8.0..=32.0).speed(0.5).suffix(" pt"));
        });

        ui.add_space(12.0);

        // Save button for font size
        let size_changed = (self.pending_font_size - self.config.font_size).abs() > 0.01;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(size_changed, egui::Button::new(format!("{} Save Font Size", regular::FLOPPY_DISK)))
                .clicked()
            {
                self.config.font_size = self.pending_font_size;
                let _ = self.config.save();
                self.fonts_need_update = true;
            }
            if size_changed {
                ui.label(egui::RichText::new("(unsaved)").color(egui::Color32::YELLOW).small());
            }
        });

        // Apply font changes live
        if font_source_changed || self.fonts_need_update {
            self.fonts_need_update = false;
            apply_font_settings(ctx, &self.config);
            let _ = self.config.save();
        }
    }

    fn render_settings_steam_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Steam Credentials");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Steam ID:");
            ui.add_space(20.0);
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.config.steam_id)
                        .desired_width(180.0)
                        .hint_text("12345678901234567"),
                )
                .changed()
            {
                let _ = self.config.save();
            }
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("API Key:");
            ui.add_space(28.0);
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.config.steam_web_api_key)
                        .desired_width(180.0)
                        .password(true)
                        .hint_text("Your Steam API key"),
                )
                .changed()
            {
                let _ = self.config.save();
            }
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.hyperlink_to(format!("{} Get API Key", regular::LINK), "https://steamcommunity.com/dev/apikey");
            ui.label(egui::RichText::new("(No affiliation)").color(egui::Color32::GRAY));
        });

        ui.horizontal(|ui| {
            ui.hyperlink_to(format!("{} Figure out Steam ID", regular::LINK), "https://steamid.io");
            ui.label(egui::RichText::new("(No affiliation)").color(egui::Color32::GRAY));
        });

        ui.add_space(12.0);

        // Validation status
        if !self.config.is_valid() {
            ui.colored_label(egui::Color32::YELLOW, format!("{} Steam ID and API Key are required", regular::WARNING));
        } else {
            ui.colored_label(egui::Color32::GREEN, format!("{} Configuration valid", regular::CHECK));
        }
    }

    fn render_settings_debug_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(format!("{} Debug", regular::BUG));
        ui.add_space(8.0);

        if ui
            .checkbox(&mut self.config.debug_recently_played, "Log recently played response")
            .on_hover_text("When running Update, write the recently played API response to recently_played_debug.txt")
            .changed()
        {
            let _ = self.config.save();
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);

        ui.label("Configuration Files:");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.button("Open Config File").clicked() {
                let config_path = crate::config::Config::get_config_file_path();
                if let Err(e) = open::that(&config_path) {
                    eprintln!("Failed to open config file: {}", e);
                }
            }

            if ui.button("Open Config Directory").clicked() {
                if let Some(config_dir) = crate::config::Config::get_config_dir() {
                    if let Err(e) = open::that(&config_dir) {
                        eprintln!("Failed to open config directory: {}", e);
                    }
                }
            }
        });
    }
}
