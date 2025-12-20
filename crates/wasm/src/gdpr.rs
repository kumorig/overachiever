//! GDPR consent modal UI

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::GdprConsent;

use crate::app::WasmApp;
use crate::storage::{save_gdpr_consent_to_storage, clear_token_from_storage};

impl WasmApp {
    /// Render the GDPR consent modal if consent hasn't been given
    pub fn render_gdpr_modal(&mut self, ctx: &egui::Context) {
        // Only show if consent not set
        if self.gdpr_consent.is_set() {
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
        egui::Window::new(format!("{} Privacy & Data Usage (GDPR)", regular::SHIELD_CHECK))
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
                            ui.label("• Session authentication tokens");
                        });
                    
                    ui.add_space(12.0);
                    
                    // Purpose section
                    ui.heading("Purpose");
                    ui.add_space(4.0);
                    ui.label("This data is used to display your game library and track achievement progress. Your data is stored on our server and associated with your Steam ID.");
                    
                    ui.add_space(12.0);
                    
                    // Third party section
                    ui.heading("Third Parties");
                    ui.add_space(4.0);
                    ui.label("We use the Steam Web API to fetch your public game and achievement data. No data is shared with other third parties.");
                    
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);
                    
                    // Buttons
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(format!("{} Accept", regular::CHECK))
                                .on_hover_text("Accept data processing and continue")
                                .clicked() 
                            {
                                self.gdpr_consent = GdprConsent::Accepted;
                                save_gdpr_consent_to_storage(GdprConsent::Accepted);
                            }
                            
                            if ui.button(format!("{} Decline", regular::X))
                                .on_hover_text("Decline - you won't be able to use the application")
                                .clicked() 
                            {
                                self.gdpr_consent = GdprConsent::Declined;
                                save_gdpr_consent_to_storage(GdprConsent::Declined);
                                // Clear any existing auth data
                                self.auth_token = None;
                                clear_token_from_storage();
                            }
                        });
                    });
                    
                    ui.add_space(4.0);
                });
            });
    }
}
