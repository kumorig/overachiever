//! Games table panel - Central panel with filterable, sortable games list

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular;

use crate::app::SteamOverachieverApp;
use crate::db::{open_connection, get_game_achievements};
use crate::ui::{SortColumn, TriFilter};

impl SteamOverachieverApp {
    pub(crate) fn render_games_table(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(format!("Games Library ({} games)", self.games.len()));
            ui.separator();
            
            if self.games.is_empty() {
                ui.label("No games loaded. Click 'Update' to load your Steam library.");
                return;
            }
            
            self.render_filter_bar(ui);
            ui.add_space(4.0);
            
            let filtered_indices = self.get_filtered_indices();
            let filtered_count = filtered_indices.len();
            
            if filtered_count != self.games.len() {
                ui.label(format!("Showing {} of {} games", filtered_count, self.games.len()));
            }
            
            self.render_table(ui, filtered_indices);
        });
    }
    
    fn render_filter_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.add(egui::TextEdit::singleline(&mut self.filter_name)
                .hint_text("Search by name...")
                .desired_width(150.0));
            
            ui.add_space(10.0);
            
            // Achievements filter - tri-state toggle button
            let ach_label = format!("Achievements: {}", self.filter_achievements.label("With", "Without"));
            if ui.button(&ach_label).clicked() {
                self.filter_achievements = self.filter_achievements.cycle();
            }
            
            // Playtime filter - tri-state toggle button
            let play_label = format!("Played: {}", self.filter_playtime.label("Yes", "No"));
            if ui.button(&play_label).clicked() {
                self.filter_playtime = self.filter_playtime.cycle();
            }
            
            // Clear filters button
            if self.filter_name.is_empty() 
                && self.filter_achievements == TriFilter::All 
                && self.filter_playtime == TriFilter::All {
                ui.add_enabled(false, egui::Button::new("Clear"));
            } else if ui.button("Clear").clicked() {
                self.filter_name.clear();
                self.filter_achievements = TriFilter::All;
                self.filter_playtime = TriFilter::All;
            }
        });
    }
    
    fn get_filtered_indices(&self) -> Vec<usize> {
        let filter_name_lower = self.filter_name.to_lowercase();
        
        self.games.iter()
            .enumerate()
            .filter(|(_, g)| {
                // Name filter
                if !filter_name_lower.is_empty() && !g.name.to_lowercase().contains(&filter_name_lower) {
                    return false;
                }
                // Achievements filter
                let has_achievements = g.achievements_total.map(|t| t > 0).unwrap_or(false);
                match self.filter_achievements {
                    TriFilter::All => {}
                    TriFilter::With => if !has_achievements { return false; }
                    TriFilter::Without => if has_achievements { return false; }
                }
                // Playtime filter
                let has_playtime = g.rtime_last_played.map(|ts| ts > 0).unwrap_or(false);
                match self.filter_playtime {
                    TriFilter::All => {}
                    TriFilter::With => if !has_playtime { return false; }
                    TriFilter::Without => if has_playtime { return false; }
                }
                true
            })
            .map(|(idx, _)| idx)
            .collect()
    }
    
    fn render_table(&mut self, ui: &mut egui::Ui, filtered_indices: Vec<usize>) {
        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);
        
        let available_height = ui.available_height();
        
        // Calculate row heights for each filtered game (including expanded achievements)
        let expanded_rows = self.expanded_rows.clone();
        let row_heights: Vec<f32> = filtered_indices.iter().map(|&idx| {
            let game = &self.games[idx];
            if expanded_rows.contains(&game.appid) {
                text_height + 330.0
            } else {
                text_height
            }
        }).collect();
        
        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(100.0).clip(true)) // Name
            .column(Column::initial(90.0).at_least(70.0)) // Last Played
            .column(Column::initial(80.0).at_least(60.0)) // Playtime
            .column(Column::initial(100.0).at_least(80.0)) // Achievements
            .column(Column::initial(60.0).at_least(40.0)) // Percent
            .min_scrolled_height(0.0)
            .max_scroll_height(available_height)
            .header(20.0, |mut header| {
                header.col(|ui| {
                    if ui.selectable_label(
                        self.sort_column == SortColumn::Name,
                        format!("Name{}", self.sort_indicator(SortColumn::Name))
                    ).clicked() {
                        self.set_sort(SortColumn::Name);
                    }
                });
                header.col(|ui| {
                    if ui.selectable_label(
                        self.sort_column == SortColumn::LastPlayed,
                        format!("Last Played{}", self.sort_indicator(SortColumn::LastPlayed))
                    ).clicked() {
                        self.set_sort(SortColumn::LastPlayed);
                    }
                });
                header.col(|ui| {
                    if ui.selectable_label(
                        self.sort_column == SortColumn::Playtime,
                        format!("Playtime{}", self.sort_indicator(SortColumn::Playtime))
                    ).clicked() {
                        self.set_sort(SortColumn::Playtime);
                    }
                });
                header.col(|ui| {
                    if ui.selectable_label(
                        self.sort_column == SortColumn::AchievementsTotal,
                        format!("Achievements{}", self.sort_indicator(SortColumn::AchievementsTotal))
                    ).clicked() {
                        self.set_sort(SortColumn::AchievementsTotal);
                    }
                });
                header.col(|ui| {
                    if ui.selectable_label(
                        self.sort_column == SortColumn::AchievementsPercent,
                        format!("%{}", self.sort_indicator(SortColumn::AchievementsPercent))
                    ).clicked() {
                        self.set_sort(SortColumn::AchievementsPercent);
                    }
                });
            })
            .body(|body| {
                body.heterogeneous_rows(row_heights.into_iter(), |mut row| {
                    let row_idx = row.index();
                    let game_idx = filtered_indices[row_idx];
                    let game = &self.games[game_idx];
                    let appid = game.appid;
                    let is_expanded = self.expanded_rows.contains(&appid);
                    
                    // Check if this game should be flashing
                    let flash_color = self.get_flash_intensity(appid).map(|intensity| {
                        egui::Color32::from_rgba_unmultiplied(
                            255,  // R
                            215,  // G (gold)
                            0,    // B
                            (intensity * 100.0) as u8
                        )
                    });
                    
                    // Name column with expand/collapse toggle
                    row.col(|ui| {
                        if let Some(color) = flash_color {
                            ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                        }
                        self.render_name_cell(ui, game_idx, is_expanded);
                    });
                    
                    row.col(|ui| {
                        if let Some(color) = flash_color {
                            ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                        }
                        if !is_expanded {
                            let game = &self.games[game_idx];
                            if let Some(ts) = game.rtime_last_played {
                                if ts > 0 {
                                    let dt = chrono::DateTime::from_timestamp(ts as i64, 0)
                                        .map(|d| d.format("%Y-%m-%d").to_string())
                                        .unwrap_or_else(|| "—".to_string());
                                    ui.label(dt);
                                } else {
                                    ui.label("Never");
                                }
                            } else {
                                ui.label("—");
                            }
                        }
                    });
                    
                    row.col(|ui| {
                        if let Some(color) = flash_color {
                            ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                        }
                        if !is_expanded {
                            let game = &self.games[game_idx];
                            let never_played = game.rtime_last_played.map(|ts| ts == 0).unwrap_or(true);
                            if never_played {
                                ui.label("--");
                            } else {
                                ui.label(format!("{:.1}h", game.playtime_forever as f64 / 60.0));
                            }
                        }
                    });
                    
                    row.col(|ui| {
                        if let Some(color) = flash_color {
                            ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                        }
                        if !is_expanded {
                            ui.label(self.games[game_idx].achievements_display());
                        }
                    });
                    
                    row.col(|ui| {
                        if let Some(color) = flash_color {
                            ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                        }
                        if !is_expanded {
                            if let Some(pct) = self.games[game_idx].completion_percent() {
                                ui.label(format!("{:.0}%", pct));
                            } else {
                                ui.label("—");
                            }
                        }
                    });
                });
            });
    }
    
    fn render_name_cell(&mut self, ui: &mut egui::Ui, game_idx: usize, is_expanded: bool) {
        // Extract all needed data before entering closures to avoid borrow conflicts
        let game = &self.games[game_idx];
        let appid = game.appid;
        let has_achievements = game.achievements_total.map(|t| t > 0).unwrap_or(false);
        let game_name = game.name.clone();
        let img_icon_url = game.img_icon_url.clone();
        
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                if has_achievements {
                    let icon = if is_expanded { regular::CARET_DOWN } else { regular::CARET_RIGHT };
                    if ui.small_button(icon.to_string()).clicked() {
                        if is_expanded {
                            self.expanded_rows.remove(&appid);
                        } else {
                            self.expanded_rows.insert(appid);
                            // Load achievements if not cached
                            if !self.achievements_cache.contains_key(&appid) {
                                if let Ok(conn) = open_connection() {
                                    if let Ok(achs) = get_game_achievements(&conn, appid) {
                                        self.achievements_cache.insert(appid, achs);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    ui.add_space(20.0);
                }
                
                // Show game icon when expanded
                if is_expanded {
                    if let Some(icon_hash) = &img_icon_url {
                        if !icon_hash.is_empty() {
                            let game_icon_url = format!(
                                "https://media.steampowered.com/steamcommunity/public/images/apps/{}/{}.jpg",
                                appid, icon_hash
                            );
                            let img_source: egui::ImageSource<'_> = if let Some(bytes) = self.icon_cache.get_icon_bytes(&game_icon_url) {
                                let cache_uri = format!("bytes://game/{}", appid);
                                ui.ctx().include_bytes(cache_uri.clone(), bytes);
                                egui::ImageSource::Uri(cache_uri.into())
                            } else {
                                egui::ImageSource::Uri(game_icon_url.into())
                            };
                            ui.add(
                                egui::Image::new(img_source)
                                    .fit_to_exact_size(egui::vec2(32.0, 32.0))
                                    .corner_radius(4.0)
                            );
                        }
                    }
                    ui.label(egui::RichText::new(&game_name).strong());
                } else {
                    ui.label(&game_name);
                }
            });
            
            // Show achievements table if expanded
            if is_expanded {
                self.render_achievements_list(ui, appid);
            }
        });
    }
    
    fn render_achievements_list(&self, ui: &mut egui::Ui, appid: u64) {
        if let Some(achievements) = self.achievements_cache.get(&appid) {
            ui.add_space(4.0);
            ui.separator();
            
            // Sort achievements: unlocked first (by unlock time desc), then locked
            let mut sorted_achs: Vec<_> = achievements.iter().collect();
            sorted_achs.sort_by(|a, b| {
                match (a.achieved, b.achieved) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    (true, true) => b.unlocktime.cmp(&a.unlocktime),
                    (false, false) => a.name.cmp(&b.name),
                }
            });
            
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (i, ach) in sorted_achs.iter().enumerate() {
                    let icon_url = if ach.achieved {
                        &ach.icon
                    } else {
                        &ach.icon_gray
                    };
                    
                    let image_source: egui::ImageSource<'_> = if let Some(bytes) = self.icon_cache.get_icon_bytes(icon_url) {
                        let cache_uri = format!("bytes://ach/{}", icon_url.replace(['/', ':', '.'], "_"));
                        ui.ctx().include_bytes(cache_uri.clone(), bytes);
                        egui::ImageSource::Uri(cache_uri.into())
                    } else {
                        egui::ImageSource::Uri(icon_url.to_string().into())
                    };
                    
                    // Alternate row background
                    let row_rect = ui.available_rect_before_wrap();
                    let row_rect = egui::Rect::from_min_size(
                        row_rect.min,
                        egui::vec2(row_rect.width(), 52.0)
                    );
                    if i % 2 == 1 {
                        ui.painter().rect_filled(
                            row_rect,
                            0.0,
                            ui.visuals().faint_bg_color
                        );
                    }
                    
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Image::new(image_source)
                                .fit_to_exact_size(egui::vec2(48.0, 48.0))
                                .corner_radius(4.0)
                        );
                        
                        let name_text = if ach.achieved {
                            egui::RichText::new(&ach.name).color(egui::Color32::WHITE)
                        } else {
                            egui::RichText::new(&ach.name).color(egui::Color32::DARK_GRAY)
                        };
                        
                        let description_text = ach.description.as_deref().unwrap_or("");
                        let desc_color = if ach.achieved {
                            egui::Color32::GRAY
                        } else {
                            egui::Color32::from_rgb(80, 80, 80)
                        };
                        
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            // Top row: name and date
                            ui.horizontal(|ui| {
                                ui.label(name_text);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if let Some(unlock_dt) = &ach.unlocktime {
                                        ui.label(
                                            egui::RichText::new(unlock_dt.format("%Y-%m-%d").to_string())
                                                .color(egui::Color32::from_rgb(100, 200, 100))
                                        );
                                    }
                                });
                            });
                            // Description below, full width
                            if !description_text.is_empty() {
                                ui.label(egui::RichText::new(description_text).color(desc_color));
                            }
                        });
                    });
                }
            });
        } else {
            ui.spinner();
            ui.label("Loading achievements...");
        }
    }
}
