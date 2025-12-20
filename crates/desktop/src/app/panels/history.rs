//! History side panel - Uses shared stats panel from core

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::{render_stats_content, render_log_content, StatsPanelConfig, SidebarPanel};

use crate::app::SteamOverachieverApp;

impl SteamOverachieverApp {
    pub(crate) fn render_history_panel(&mut self, ctx: &egui::Context) {
        // Slightly darker background for the sidebar
        let panel_fill = ctx.style().visuals.window_fill();
        let darker_fill = egui::Color32::from_rgb(
            panel_fill.r().saturating_sub(8),
            panel_fill.g().saturating_sub(8),
            panel_fill.b().saturating_sub(8),
        );
        let panel_frame = egui::Frame::side_top_panel(&ctx.style())
            .fill(darker_fill);

        if !self.show_stats_panel {
            // Collapsed sidebar - show two buttons (Stats and Log)
            egui::SidePanel::right("history_panel_collapsed")
                .exact_width(36.0)
                .resizable(false)
                .frame(panel_frame)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    // Stats button
                    if ui.button(regular::CHART_LINE.to_string())
                        .on_hover_text("Open Stats Panel")
                        .clicked() 
                    {
                        self.sidebar_panel = SidebarPanel::Stats;
                        self.show_stats_panel = true;
                    }
                    // Log button
                    if ui.button(regular::SCROLL.to_string())
                        .on_hover_text("Open Log Panel")
                        .clicked()
                    {
                        self.sidebar_panel = SidebarPanel::Log;
                        self.show_stats_panel = true;
                    }
                });
            return;
        }

        egui::SidePanel::right("history_panel")
            .min_width(350.0)
            .frame(panel_frame)
            .show(ctx, |ui| {
                // Top navigation bar: close button + panel tabs
                ui.horizontal(|ui| {
                    // Close button (chevron right to collapse)
                    if ui.small_button(regular::CARET_RIGHT.to_string())
                        .on_hover_text("Close Panel")
                        .clicked() 
                    {
                        self.show_stats_panel = false;
                    }
                    
                    ui.separator();
                    
                    // Panel navigation tabs
                    let stats_selected = self.sidebar_panel == SidebarPanel::Stats;
                    let log_selected = self.sidebar_panel == SidebarPanel::Log;
                    
                    if ui.selectable_label(stats_selected, format!("{} Stats", regular::CHART_LINE)).clicked() {
                        self.sidebar_panel = SidebarPanel::Stats;
                    }
                    if ui.selectable_label(log_selected, format!("{} Log", regular::SCROLL)).clicked() {
                        self.sidebar_panel = SidebarPanel::Log;
                    }
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.sidebar_panel {
                        SidebarPanel::Stats => {
                            let config = StatsPanelConfig::desktop();
                            render_stats_content(ui, self, &config);
                        }
                        SidebarPanel::Log => {
                            render_log_content(ui, self);
                        }
                    }
                });
            });
    }
}
