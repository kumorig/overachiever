//! History side panel - Games over time graph, achievement progress, run history

use eframe::egui;
use egui_phosphor::regular;
use egui_plot::{Line, Plot, PlotPoints};

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
                    self.render_run_history(ui);
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
        
        let avg_line = Line::new("Avg Game Completion %", avg_completion_points)
            .color(egui::Color32::from_rgb(100, 200, 100));
        let overall_line = Line::new("Overall Achievement %", overall_pct_points)
            .color(egui::Color32::from_rgb(100, 150, 255));
        
        Plot::new("achievements_history")
            .view_aspect(2.0)
            .legend(egui_plot::Legend::default())
            .include_y(0.0)
            .include_y(100.0)
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
                ui.label(egui::RichText::new(format!("{}", latest.unlocked_achievements)).color(yellow).strong());
                ui.label("/");
                ui.label(egui::RichText::new(format!("{}", latest.total_achievements)).color(yellow).strong());
                ui.label("achievements (");
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
                ui.label("Average completion:");
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
        }
    }
    
    fn render_run_history(&self, ui: &mut egui::Ui) {
        ui.collapsing(format!("{} Run History", regular::CLOCK_COUNTER_CLOCKWISE), |ui| {
            if self.run_history.is_empty() {
                ui.label("No runs recorded yet.");
            } else {
                for entry in self.run_history.iter().rev() {
                    ui.horizontal(|ui| {
                        ui.label(entry.run_at.format("%Y-%m-%d %H:%M").to_string());
                        ui.label(format!("{} games", entry.total_games));
                    });
                }
            }
        });
    }
}
