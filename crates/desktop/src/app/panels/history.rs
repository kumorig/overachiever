//! History side panel - Games over time graph, achievement progress, run history

use eframe::egui;
use egui_phosphor::regular;
use egui_plot::{Line, Plot, PlotPoints};
use overachiever_core::LogEntry;

use crate::app::SteamOverachieverApp;

impl SteamOverachieverApp {
    pub(crate) fn render_history_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("history_panel")
            .min_width(350.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.render_games_over_time(ui);
                    ui.add_space(16.0);
                    self.render_achievement_progress(ui);
                    ui.add_space(16.0);
                    self.render_log(ui);
                });
            });
    }
    
    fn render_games_over_time(&self, ui: &mut egui::Ui) {
        ui.heading("Games Over Time");
        ui.separator();
        
        if self.run_history.is_empty() {
            ui.label("No history yet. Click 'Update' to start tracking!");
        } else {
            let points: PlotPoints = self.run_history
                .iter()
                .enumerate()
                .map(|(i, h)| [i as f64, h.total_games as f64])
                .collect();
            
            let line = Line::new("Total Games", points);
            
            Plot::new("games_history")
                .view_aspect(2.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(line);
                });
        }
    }
    
    fn render_achievement_progress(&mut self, ui: &mut egui::Ui) {
        ui.heading("Achievement Progress");
        ui.separator();
        
        if self.achievement_history.is_empty() {
            ui.label("No achievement data yet. Click 'Full Scan' to start tracking!");
            return;
        }
        
        // Line 1: Average game completion %
        let avg_completion_points: PlotPoints = self.achievement_history
            .iter()
            .enumerate()
            .map(|(i, h)| [i as f64, h.avg_completion_percent as f64])
            .collect();
        
        // Line 2: Overall achievement % (unlocked / total)
        let overall_pct_points: PlotPoints = self.achievement_history
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let pct = if h.total_achievements > 0 {
                    h.unlocked_achievements as f64 / h.total_achievements as f64 * 100.0
                } else {
                    0.0
                };
                [i as f64, pct]
            })
            .collect();
        
        // Calculate Y-axis bounds based on actual data
        let all_values: Vec<f64> = self.achievement_history
            .iter()
            .flat_map(|h| {
                let overall_pct = if h.total_achievements > 0 {
                    h.unlocked_achievements as f64 / h.total_achievements as f64 * 100.0
                } else {
                    0.0
                };
                vec![h.avg_completion_percent as f64, overall_pct]
            })
            .collect();
        
        let min_y = all_values.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
        let max_y = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max).min(100.0);
        
        // Add some padding (5% of range, minimum 1.0)
        let range = max_y - min_y;
        let padding = (range * 0.05).max(1.0);
        let y_min = (min_y - padding).max(0.0);
        let y_max = (max_y + padding).min(100.0);
        
        let avg_line = Line::new("Avg Game Completion %", avg_completion_points)
            .color(egui::Color32::from_rgb(100, 200, 100));
        let overall_line = Line::new("Overall Achievement %", overall_pct_points)
            .color(egui::Color32::from_rgb(100, 150, 255));
        
        Plot::new("achievements_history")
            .view_aspect(2.0)
            .legend(egui_plot::Legend::default())
            .include_y(y_min)
            .include_y(y_max)
            .show(ui, |plot_ui| {
                plot_ui.line(avg_line);
                plot_ui.line(overall_line);
            });
        
        // Show current stats
        if let Some(latest) = self.achievement_history.last() {
            ui.add_space(8.0);
            
            // Yellow color for prominent numbers
            let yellow = egui::Color32::from_rgb(255, 215, 0);
            
            let overall_pct = if latest.total_achievements > 0 {
                latest.unlocked_achievements as f32 / latest.total_achievements as f32 * 100.0
            } else {
                0.0
            };
            
            ui.horizontal(|ui| {
                ui.label("Total achievements:");
                ui.label(egui::RichText::new(format!("{}", latest.unlocked_achievements)).color(yellow).strong());
                ui.label("/");
                ui.label(egui::RichText::new(format!("{}", latest.total_achievements)).color(yellow).strong());
                ui.label("(");
                ui.label(egui::RichText::new(format!("{:.1}%", overall_pct)).color(yellow).strong());
                ui.label(")");
            });
            
            // Calculate current avg completion based on toggle
            let games_with_ach: Vec<_> = self.games.iter()
                .filter(|g| g.achievements_total.map(|t| t > 0).unwrap_or(false))
                .collect();
            
            // Count played vs unplayed games (with achievements)
            let played_count = games_with_ach.iter()
                .filter(|g| g.playtime_forever > 0)
                .count();
            let unplayed_count = games_with_ach.len() - played_count;
            
            let completion_percents: Vec<f32> = if self.include_unplayed_in_avg {
                games_with_ach.iter()
                    .filter_map(|g| g.completion_percent())
                    .collect()
            } else {
                games_with_ach.iter()
                    .filter(|g| g.playtime_forever > 0)
                    .filter_map(|g| g.completion_percent())
                    .collect()
            };
            
            let current_avg = if completion_percents.is_empty() {
                0.0
            } else {
                completion_percents.iter().sum::<f32>() / completion_percents.len() as f32
            };
            
            ui.horizontal(|ui| {
                ui.label("Avg. game completion:");
                ui.label(egui::RichText::new(format!("{:.1}%", current_avg)).color(yellow).strong());
                ui.checkbox(&mut self.include_unplayed_in_avg, "Include unplayed");
            });
            
            // Show unplayed games count and percentage
            let total_games_with_ach = games_with_ach.len();
            let unplayed_pct = if total_games_with_ach > 0 {
                unplayed_count as f32 / total_games_with_ach as f32 * 100.0
            } else {
                0.0
            };
            ui.horizontal(|ui| {
                ui.label("Unplayed games:");
                ui.label(egui::RichText::new(format!("{}", unplayed_count)).color(yellow).strong());
                ui.label("(");
                ui.label(egui::RichText::new(format!("{:.1}%", unplayed_pct)).color(yellow).strong());
                ui.label(")");
            });
            
            // Additional stats (matching WASM breakdown)
            ui.add_space(8.0);
            
            ui.horizontal(|ui| {
                ui.label("Total games:");
                ui.label(egui::RichText::new(format!("{}", self.games.len())).color(yellow).strong());
            });
            
            ui.horizontal(|ui| {
                ui.label("Games with achievements:");
                ui.label(egui::RichText::new(format!("{}", total_games_with_ach)).color(yellow).strong());
            });
            
            let completed = self.games.iter()
                .filter(|g| g.completion_percent().map(|p| p >= 100.0).unwrap_or(false))
                .count();
            ui.horizontal(|ui| {
                ui.label(format!("{} 100% completed:", regular::MEDAL));
                ui.label(egui::RichText::new(format!("{}", completed)).color(yellow).strong());
            });
            
            let needs_scan = self.games.iter().filter(|g| g.achievements_total.is_none()).count();
            if needs_scan > 0 {
                ui.horizontal(|ui| {
                    ui.label("Needs scanning:");
                    ui.label(egui::RichText::new(format!("{}", needs_scan)).color(egui::Color32::LIGHT_GRAY));
                });
            }
        }
    }
    
    fn render_log(&mut self, ui: &mut egui::Ui) {
        // Colors for different elements
        let date_color = egui::Color32::from_rgb(130, 130, 130);  // Gray for dates
        let game_color = egui::Color32::from_rgb(100, 180, 255);  // Blue for game names
        let achievement_color = egui::Color32::from_rgb(255, 215, 0);  // Gold for achievement names
        let alt_bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8);  // Subtle alternating bg
        
        ui.collapsing(format!("{} Log", regular::SCROLL), |ui| {
            if self.log_entries.is_empty() {
                ui.label("No activity yet.");
            } else {
                for (i, entry) in self.log_entries.iter().enumerate() {
                    // Alternating background
                    let row_rect = ui.available_rect_before_wrap();
                    let row_rect = egui::Rect::from_min_size(
                        row_rect.min,
                        egui::vec2(row_rect.width(), 24.0)
                    );
                    if i % 2 == 1 {
                        ui.painter().rect_filled(row_rect, 2.0, alt_bg);
                    }
                    
                    match entry {
                        LogEntry::Achievement { appid, game_name, achievement_name, timestamp, achievement_icon, game_icon_url } => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                
                                // Game icon (left)
                                if let Some(icon_hash) = game_icon_url {
                                    if !icon_hash.is_empty() {
                                        let game_icon_url = format!(
                                            "https://media.steampowered.com/steamcommunity/public/images/apps/{}/{}.jpg",
                                            appid, icon_hash
                                        );
                                        let img_source: egui::ImageSource<'_> = if let Some(bytes) = self.icon_cache.get_icon_bytes(&game_icon_url) {
                                            let cache_uri = format!("bytes://log_game/{}", appid);
                                            ui.ctx().include_bytes(cache_uri.clone(), bytes);
                                            egui::ImageSource::Uri(cache_uri.into())
                                        } else {
                                            egui::ImageSource::Uri(game_icon_url.into())
                                        };
                                        ui.add(
                                            egui::Image::new(img_source)
                                                .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                                .corner_radius(2.0)
                                        );
                                    }
                                }
                                
                                // Achievement icon (right of game icon)
                                if !achievement_icon.is_empty() {
                                    let img_source: egui::ImageSource<'_> = if let Some(bytes) = self.icon_cache.get_icon_bytes(achievement_icon) {
                                        let cache_uri = format!("bytes://log_ach/{}", achievement_icon.replace(['/', ':', '.'], "_"));
                                        ui.ctx().include_bytes(cache_uri.clone(), bytes);
                                        egui::ImageSource::Uri(cache_uri.into())
                                    } else {
                                        egui::ImageSource::Uri(achievement_icon.clone().into())
                                    };
                                    ui.add(
                                        egui::Image::new(img_source)
                                            .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                            .corner_radius(2.0)
                                    );
                                }
                                
                                ui.label(egui::RichText::new(timestamp.format("%Y-%m-%d").to_string()).color(date_color).small());
                                ui.label(egui::RichText::new(achievement_name).color(achievement_color).strong());
                                ui.label(egui::RichText::new("in").small());
                                ui.label(egui::RichText::new(format!("{}!", game_name)).color(game_color));
                            });
                        }
                        LogEntry::FirstPlay { appid, game_name, timestamp, game_icon_url } => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                
                                // Game icon
                                if let Some(icon_hash) = game_icon_url {
                                    if !icon_hash.is_empty() {
                                        let game_icon_url = format!(
                                            "https://media.steampowered.com/steamcommunity/public/images/apps/{}/{}.jpg",
                                            appid, icon_hash
                                        );
                                        let img_source: egui::ImageSource<'_> = if let Some(bytes) = self.icon_cache.get_icon_bytes(&game_icon_url) {
                                            let cache_uri = format!("bytes://log_game/{}", appid);
                                            ui.ctx().include_bytes(cache_uri.clone(), bytes);
                                            egui::ImageSource::Uri(cache_uri.into())
                                        } else {
                                            egui::ImageSource::Uri(game_icon_url.into())
                                        };
                                        ui.add(
                                            egui::Image::new(img_source)
                                                .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                                .corner_radius(2.0)
                                        );
                                    } else {
                                        ui.add_space(22.0);
                                    }
                                } else {
                                    ui.add_space(22.0);
                                }
                                
                                ui.label(egui::RichText::new(timestamp.format("%Y-%m-%d").to_string()).color(date_color).small());
                                ui.label(egui::RichText::new(game_name).color(game_color));
                                ui.label(egui::RichText::new("played for the first time!").small());
                            });
                        }
                    }
                }
            }
        });
    }
}
