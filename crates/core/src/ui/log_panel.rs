//! Log panel - shared between desktop and WASM
//! 
//! Renders: Activity log (achievements and first plays)
//! Features: Star ratings, achievement selection, batch commenting

use egui::{self, Color32, RichText, Ui, Response, RectAlign, Sense};
use egui::containers::Popup;
use egui_phosphor::regular;

use crate::LogEntry;
use super::StatsPanelPlatform;

// ============================================================================
// Constants
// ============================================================================

const STAR_SIZE: f32 = 14.0;
const STAR_SPACING: f32 = 2.0;
const STAR_COLOR_EMPTY: Color32 = Color32::from_rgb(80, 80, 80);
// Note: STAR_COLOR_HOVER is computed at runtime due to alpha
const SELECTION_COLOR: Color32 = Color32::from_rgb(100, 150, 255);

/// Get hover color for stars (with transparency)
fn star_color_hover() -> Color32 {
    Color32::from_rgba_unmultiplied(255, 215, 0, 180)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Show a tooltip immediately (no delay) positioned to the left
fn instant_tooltip(response: &Response, text: impl Into<String>) {
    if response.hovered() {
        let text = text.into();
        Popup::from_response(response)
            .align(RectAlign::LEFT_START)
            .gap(4.0)
            .show(|ui| { ui.label(&text); });
    }
}

/// Render a 5-star rating widget. Returns Some(rating) if clicked.
fn star_rating_widget(ui: &mut Ui, id: egui::Id) -> Option<u8> {
    let mut clicked_rating: Option<u8> = None;
    
    // Calculate hover state for all stars
    let start_pos = ui.cursor().min;
    let total_width = 5.0 * STAR_SIZE + 4.0 * STAR_SPACING;
    let rating_rect = egui::Rect::from_min_size(start_pos, egui::vec2(total_width, STAR_SIZE));
    
    // Sense for the whole rating area
    let response = ui.allocate_rect(rating_rect, Sense::click());
    let hover_star = if response.hovered() {
        if let Some(pos) = response.hover_pos() {
            let rel_x = pos.x - start_pos.x;
            Some(((rel_x / (STAR_SIZE + STAR_SPACING)).floor() as u8).min(4) + 1)
        } else {
            None
        }
    } else {
        None
    };
    
    // Draw stars
    let painter = ui.painter();
    for i in 0..5u8 {
        let star_num = i + 1;
        let x = start_pos.x + i as f32 * (STAR_SIZE + STAR_SPACING);
        let center = egui::pos2(x + STAR_SIZE / 2.0, start_pos.y + STAR_SIZE / 2.0);
        
        let color = if let Some(hover) = hover_star {
            if star_num <= hover {
                star_color_hover()
            } else {
                STAR_COLOR_EMPTY
            }
        } else {
            STAR_COLOR_EMPTY
        };
        
        // Draw star using phosphor icon
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            regular::STAR,
            egui::FontId::proportional(STAR_SIZE),
            color,
        );
    }
    
    // Handle click
    if response.clicked() {
        if let Some(rating) = hover_star {
            clicked_rating = Some(rating);
        }
    }
    
    // Suppress unused variable warning
    let _ = id;
    
    clicked_rating
}

// ============================================================================
// Main Render Functions  
// ============================================================================

/// Render the complete log panel content (inside a scroll area)
pub fn render_log_content<P: StatsPanelPlatform>(ui: &mut Ui, platform: &mut P) {
    ui.heading(format!("{} Activity Log", regular::SCROLL));
    ui.separator();
    
    render_log(ui, platform);
    
    // Show comment panel if achievements are selected
    let selected = platform.selected_achievements();
    if !selected.is_empty() {
        ui.add_space(8.0);
        render_comment_panel(ui, platform, &selected);
    }
}

/// Render the activity log (achievements and first plays)
pub fn render_log<P: StatsPanelPlatform>(ui: &mut Ui, platform: &mut P) {
    let achievement_color = Color32::from_rgb(255, 215, 0);
    let game_color = Color32::from_rgb(100, 180, 255);
    let alt_bg = Color32::from_rgba_unmultiplied(255, 255, 255, 8);
    
    let log_entries = platform.log_entries().to_vec(); // Clone to avoid borrow issues
    
    if log_entries.is_empty() {
        ui.label("No activity yet. Sync and scan to start tracking!");
        return;
    }
    
    for (i, entry) in log_entries.iter().enumerate() {
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
            LogEntry::Achievement { appid, apiname, game_name, achievement_name, timestamp, achievement_icon, game_icon_url } => {
                let is_selected = platform.is_achievement_selected(*appid, apiname);
                
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    
                    // Game icon - tooltip shows game name
                    if let Some(icon_hash) = game_icon_url {
                        if !icon_hash.is_empty() {
                            let img_source = platform.game_icon_source(ui, *appid, icon_hash);
                            let response = ui.add(
                                egui::Image::new(img_source)
                                    .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                    .corner_radius(2.0)
                            );
                            instant_tooltip(&response, game_name.clone());
                        }
                    }
                    
                    // Selectable achievement area (icon + name)
                    let selectable_start = ui.cursor().min;
                    
                    // Achievement icon - tooltip shows date
                    let mut icon_response: Option<Response> = None;
                    if !achievement_icon.is_empty() {
                        let img_source = platform.achievement_icon_source(ui, achievement_icon);
                        let response = ui.add(
                            egui::Image::new(img_source)
                                .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                .corner_radius(2.0)
                                .sense(Sense::click())
                        );
                        instant_tooltip(&response, timestamp.format("%Y-%m-%d").to_string());
                        icon_response = Some(response);
                    }
                    
                    // Achievement name (clickable)
                    let name_response = ui.add(
                        egui::Label::new(RichText::new(achievement_name).color(achievement_color).strong())
                            .sense(Sense::click())
                    );
                    
                    let selectable_end = ui.cursor().min;
                    
                    // Draw selection rectangle if selected
                    if is_selected {
                        let select_rect = egui::Rect::from_min_max(
                            egui::pos2(selectable_start.x - 2.0, selectable_start.y - 2.0),
                            egui::pos2(selectable_end.x + 2.0, selectable_start.y + 20.0)
                        );
                        ui.painter().rect_stroke(
                            select_rect,
                            3.0,
                            egui::Stroke::new(2.0, SELECTION_COLOR),
                            egui::epaint::StrokeKind::Outside,
                        );
                    }
                    
                    // Handle selection clicks
                    let clicked = icon_response.map(|r| r.clicked()).unwrap_or(false) || name_response.clicked();
                    if clicked {
                        platform.toggle_achievement_selection(*appid, apiname.clone(), achievement_name.clone());
                    }
                    
                    // Star rating (inline after achievement name)
                    ui.add_space(8.0);
                    let star_id = egui::Id::new(("star_rating", appid, apiname));
                    if let Some(rating) = star_rating_widget(ui, star_id) {
                        platform.submit_achievement_rating(*appid, apiname.clone(), rating);
                    }
                });
            }
            LogEntry::FirstPlay { appid, game_name, timestamp, game_icon_url } => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    
                    // Game icon - tooltip shows date
                    if let Some(icon_hash) = game_icon_url {
                        if !icon_hash.is_empty() {
                            let img_source = platform.game_icon_source(ui, *appid, icon_hash);
                            let response = ui.add(
                                egui::Image::new(img_source)
                                    .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                    .corner_radius(2.0)
                            );
                            instant_tooltip(&response, timestamp.format("%Y-%m-%d").to_string());
                        } else {
                            ui.add_space(22.0);
                        }
                    } else {
                        ui.add_space(22.0);
                    }
                    
                    ui.label(RichText::new(game_name).color(game_color));
                    ui.label(RichText::new("played for the first time!").small());
                    
                    // No star rating for first plays - just fill the space
                });
            }
        }
    }
}

/// Render the comment panel for selected achievements
fn render_comment_panel<P: StatsPanelPlatform>(
    ui: &mut Ui,
    platform: &mut P,
    selected: &[(u64, String, String)],
) {
    ui.separator();
    
    // Panel header
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} Comment on {} achievement(s)", regular::CHAT_CIRCLE, selected.len())).strong());
        if ui.button(format!("{} Clear selection", regular::X)).clicked() {
            platform.clear_achievement_selections();
        }
    });
    
    // Show selected achievements
    ui.horizontal_wrapped(|ui| {
        ui.label("Selected:");
        for (_, _, name) in selected.iter().take(5) {
            ui.label(RichText::new(name).color(Color32::from_rgb(255, 215, 0)).small());
            ui.label("•");
        }
        if selected.len() > 5 {
            ui.label(RichText::new(format!("and {} more...", selected.len() - 5)).small().italics());
        }
    });
    
    // Comment input
    ui.add_space(4.0);
    let mut comment = platform.pending_comment().to_string();
    
    let text_edit = egui::TextEdit::multiline(&mut comment)
        .hint_text("Add a comment about these achievements...")
        .desired_rows(2);
    
    if ui.add(text_edit).changed() {
        // Will update below
    }
    
    ui.horizontal(|ui| {
        let can_submit = !comment.trim().is_empty();
        if ui.add_enabled(can_submit, egui::Button::new(format!("{} Submit", regular::PAPER_PLANE_TILT))).clicked() {
            platform.submit_achievement_comment(comment.clone());
            platform.set_pending_comment(String::new());
            platform.clear_achievement_selections();
        }
    });
    
    // Update pending comment
    platform.set_pending_comment(comment);
}
