//! Profile menu window

use crate::app::SteamOverachieverApp;
use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::{render_tag_search, TagSearchState};

impl SteamOverachieverApp {
    pub(in crate::app) fn render_profile_menu(&mut self, ctx: &egui::Context) {
        if !self.show_profile_menu {
            return;
        }

        let mut keep_open = true;

        egui::Window::new(format!("{} Profile", regular::USER))
            .collapsible(false)
            .resizable(true)
            .default_width(500.0)
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(4.0);

                    // 1. Steam ID (read-only)
                    ui.horizontal(|ui| {
                        ui.label("Steam ID:");
                        ui.add_space(8.0);
                        let steam_id_display = if self.config.steam_id.is_empty() {
                            "Not set".to_string()
                        } else {
                            self.config.steam_id.clone()
                        };
                        ui.add(
                            egui::TextEdit::singleline(&mut steam_id_display.clone())
                                .desired_width(180.0)
                                .interactive(false),
                        );
                    });

                    ui.add_space(8.0);

                    // 2. Cloud sync status and buttons
                    let is_linked = self.config.cloud_token.is_some();
                    let cloud_state = self.cloud_sync_state.clone();
                    let is_busy = matches!(
                        cloud_state,
                        crate::cloud_sync::CloudSyncState::Checking
                            | crate::cloud_sync::CloudSyncState::Uploading(_)
                            | crate::cloud_sync::CloudSyncState::Downloading
                            | crate::cloud_sync::CloudSyncState::Deleting
                            | crate::cloud_sync::CloudSyncState::Linking
                    );

                    if !is_linked {
                        // Not logged in - show login button with Steam image
                        let sits_image_bytes = include_bytes!("../../../../../../assets/sits_02.png");
                        ui.ctx().include_bytes("bytes://sits_02", sits_image_bytes);
                        let sits_source = egui::ImageSource::Bytes {
                            uri: "bytes://sits_02".into(),
                            bytes: egui::load::Bytes::Static(sits_image_bytes),
                        };
                        
                        let image_button = egui::Button::image(egui::Image::new(sits_source))
                            .frame(false);
                        
                        if ui.add(image_button)
                            .on_hover_text("Link your account to enable cloud sync")
                            .clicked()
                        {
                            self.start_cloud_link();
                        }
                    } else {
                        // Logged in - show profile link and cloud sync buttons
                        if let Some(short_id) = self.config.get_short_id() {
                            let profile_url = format!("https://overachiever.space/{}", short_id);

                            ui.horizontal(|ui| {
                                ui.label("Your profile:");
                                if ui.button(format!("{} Copy Link", regular::COPY)).on_hover_text(&profile_url).clicked() {
                                    ui.ctx().copy_text(profile_url.clone());
                                }
                                if ui.button(format!("{} Open", regular::ARROW_SQUARE_OUT)).on_hover_text(&profile_url).clicked() {
                                    let _ = open::that(&profile_url);
                                }
                            });

                            ui.add_space(8.0);
                        }

                        // Cloud sync buttons
                        if ui
                            .add_enabled(!is_busy, egui::Button::new(format!("{} Publish online", regular::CLOUD_ARROW_UP)))
                            .on_hover_text("Upload your local data to overachiever.space")
                            .clicked()
                        {
                            self.pending_cloud_action = Some(crate::app::CloudAction::Upload);
                        }

                        if ui
                            .add_enabled(!is_busy, egui::Button::new(format!("{} Import from cloud", regular::CLOUD_ARROW_DOWN)))
                            .on_hover_text("Download data from overachiever.space (backup if removed locally)")
                            .clicked()
                        {
                            self.pending_cloud_action = Some(crate::app::CloudAction::Download);
                        }

                        if ui
                            .add_enabled(!is_busy, egui::Button::new(format!("{} Erase online data", regular::TRASH)))
                            .on_hover_text("Remove your data from overachiever.space")
                            .clicked()
                        {
                            self.pending_cloud_action = Some(crate::app::CloudAction::Delete);
                        }

                        ui.add_space(4.0);

                        if ui
                            .add_enabled(!is_busy, egui::Button::new(format!("{} Logout", regular::SIGN_OUT)))
                            .on_hover_text("Unlink your account from cloud sync")
                            .clicked()
                        {
                            self.unlink_cloud();
                        }
                    }

                    // Show cloud sync status
                    ui.add_space(8.0);
                    match &cloud_state {
                        crate::cloud_sync::CloudSyncState::Linking => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Waiting for Steam login... (check your browser)");
                            });
                        }
                        crate::cloud_sync::CloudSyncState::Uploading(progress) => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                if progress.total_bytes > 0 {
                                    let mb_total = progress.total_bytes as f64 / (1024.0 * 1024.0);
                                    ui.label(format!("Uploading {:.2} MB...", mb_total));
                                } else {
                                    ui.label("Preparing upload...");
                                }
                            });
                            if progress.total_bytes > 0 {
                                let fraction = progress.bytes_sent as f32 / progress.total_bytes as f32;
                                ui.add(egui::ProgressBar::new(fraction).animate(progress.bytes_sent < progress.total_bytes));
                            }
                        }
                        crate::cloud_sync::CloudSyncState::Downloading => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Downloading...");
                            });
                        }
                        crate::cloud_sync::CloudSyncState::Deleting => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Deleting...");
                            });
                        }
                        crate::cloud_sync::CloudSyncState::Success(msg) => {
                            ui.colored_label(egui::Color32::GREEN, format!("{} {}", regular::CHECK, msg));
                        }
                        crate::cloud_sync::CloudSyncState::Error(msg) => {
                            ui.colored_label(egui::Color32::RED, format!("{} {}", regular::WARNING, msg));
                        }
                        _ => {}
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // 3. Private Games section
                    ui.heading("Private Games");
                    
                    // Show counts
                    let hidden_count = self.games.iter().filter(|g| g.hidden).count();
                    let private_count = self.games.iter().filter(|g| g.steam_private).count();
                    ui.label(format!("Hidden from library: {} games", hidden_count));
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        // Left column: Import button and info
                        ui.vertical(|ui| {
                            ui.label("Private games in Steam:");
                            if ui.button(format!("{} Import Private Games from Steam", regular::DOWNLOAD_SIMPLE)).clicked() {
                                if let Ok(conn) = crate::db::open_connection() {
                                    match crate::steam_config::sync_steam_hidden_games(&conn, &self.config.steam_id) {
                                        Ok(count) => {
                                            self.status = format!("Imported {} private games from Steam", count);
                                            // Reload games from database
                                            if let Ok(games) = crate::db::get_all_games(&conn, &self.config.steam_id) {
                                                self.games = games;
                                                self.sort_games();
                                            }
                                        }
                                        Err(e) => {
                                            self.status = format!("Failed to import Steam private games: {}", e);
                                            eprintln!("Failed to import Steam private games: {}", e);
                                        }
                                    }
                                }
                            }
                        });

                        ui.add_space(8.0);

                        // Right column: List of private games
                        ui.vertical(|ui| {
                            ui.label(format!("{} private games", private_count));
                            let private_games: Vec<_> = self.games.iter()
                                .filter(|g| g.steam_private)
                                .map(|g| g.name.clone())
                                .collect();

                            if !private_games.is_empty() {
                                egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                                    for name in &private_games {
                                        ui.label(format!("• {}", name));
                                    }
                                });
                            }
                        });
                    });

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // 5. Hide games with tag section
                    ui.heading("Hide games with tag:");
                    ui.add_space(4.0);

                    // Initialize tag search state if needed
                    if self.hidden_tags_search.is_none() {
                        self.hidden_tags_search = Some(TagSearchState {
                            search_text: String::new(),
                            popup_open: false,
                            selected_tags: self.hidden_tags.clone(),
                        });
                    }

                    if let Some(ref mut tag_state) = self.hidden_tags_search {
                        ui.vertical(|ui| {
                            // Sync selected tags from persistent state
                            tag_state.selected_tags = self.hidden_tags.clone();

                            let changed = render_tag_search(ui, "hidden_tags_search", tag_state, &self.available_tags, "Search tags to hide...", false);

                            if changed {
                                // Update persistent hidden tags
                                self.hidden_tags = tag_state.selected_tags.clone();
                                // TODO: Save to config and sync to backend
                            }
                        });
                    }

                    ui.add_space(4.0);
                });
            });

        if !keep_open {
            self.show_profile_menu = false;
        }
    }
}
