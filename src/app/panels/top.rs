//! Top toolbar panel - Update/Full Scan buttons and status

use eframe::egui;
use egui_phosphor::regular;

use crate::app::SteamOverachieverApp;

impl SteamOverachieverApp {
    pub(crate) fn render_top_panel(&mut self, ctx: &egui::Context) {
        let is_busy = self.state.is_busy();
        
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Steam Overachiever v3");
                ui.separator();
                
                // Update button - for recently played games
                let update_button = egui::Button::new(format!("{} Update", regular::ARROWS_CLOCKWISE));
                let update_response = ui.add_enabled(!is_busy, update_button);
                
                // Show warning if update is stale
                if self.is_update_stale() && !is_busy {
                    update_response.clone().on_hover_text(
                        "⚠️ Last update was more than 2 weeks ago.\nThe recently played API only shows games from the last 2 weeks.\nConsider running a Full Scan instead."
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
                let can_scan = needs_scrape > 0 || self.force_full_scan;
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
            });
        });
    }
}
