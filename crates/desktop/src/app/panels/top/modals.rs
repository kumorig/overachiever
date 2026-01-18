//! Modal dialogs for cloud actions and GDPR consent

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::GdprConsent;

use crate::app::{CloudAction, SteamOverachieverApp};

impl SteamOverachieverApp {
    /// Render confirmation dialog for cloud actions
    pub(crate) fn render_cloud_confirm_dialog(&mut self, ctx: &egui::Context) {
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
