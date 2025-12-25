//! Log panel - shared between desktop and WASM
//! 
//! Renders: Activity log (achievements and first plays)
//! Features: Star ratings, achievement selection, batch commenting

use egui::{self, Color32, RichText, Ui, Sense, Response};
use egui_phosphor::regular;

use crate::LogEntry;
use super::{StatsPanelPlatform, instant_tooltip};

// ============================================================================
// Constants
// ============================================================================

const STAR_SIZE: f32 = 14.0;
const STAR_SPACING: f32 = 2.0;
const FLAME_COLOR_EMPTY: Color32 = Color32::from_rgb(50, 50, 50); // More subtle empty circles

/// Get hover color for flames (with transparency)
fn flame_color_hover() -> Color32 {
    Color32::from_rgba_unmultiplied(255, 140, 0, 180) // Orange for fire
}

/// Get difficulty label for rating
fn difficulty_label(rating: u8) -> &'static str {
    match rating {
        1 => "Very easy",
        2 => "Easy",
        3 => "Moderate",
        4 => "Hard",
        5 => "Extreme",
        _ => "",
    }
}

/// Get color for difficulty label (green for easy, red for extreme)
fn difficulty_color(rating: u8) -> Color32 {
    match rating {
        1 => Color32::from_rgb(80, 200, 80),   // Green - Very easy
        2 => Color32::from_rgb(140, 200, 60),  // Yellow-green - Easy  
        3 => Color32::from_rgb(200, 200, 60),  // Yellow - Moderate
        4 => Color32::from_rgb(230, 140, 50),  // Orange - Hard
        5 => Color32::from_rgb(230, 60, 60),   // Red - Extreme
        _ => Color32::GRAY,
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Render a 5-flame difficulty rating widget with current rating displayed.
/// Returns Some(rating) if clicked.
fn star_rating_widget(ui: &mut Ui, current_rating: Option<u8>) -> Option<u8> {
    let flame_color = Color32::from_rgb(255, 100, 0); // Orange-red for flames
    let mut clicked_rating: Option<u8> = None;
    
    // Calculate total width needed
    let total_width = 5.0 * STAR_SIZE + 4.0 * STAR_SPACING;
    
    // Get the row height from surrounding text to center vertically
    let row_height = ui.text_style_height(&egui::TextStyle::Body);
    let widget_height = STAR_SIZE.max(row_height);
    
    // Allocate space and get the actual rect (this ensures proper vertical centering in horizontal layouts)
    let (rect, response) = ui.allocate_exact_size(egui::vec2(total_width, widget_height), Sense::click());
    
    // Calculate vertical center of the allocated rect
    let center_y = rect.center().y;
    
    // Calculate hover state for all flames
    let start_x = rect.min.x;
    let hover_flame = if response.hovered() {
        if let Some(pos) = response.hover_pos() {
            let rel_x = pos.x - start_x;
            Some(((rel_x / (STAR_SIZE + STAR_SPACING)).floor() as u8).min(4) + 1)
        } else {
            None
        }
    } else {
        None
    };
    
    // Draw flames
    let painter = ui.painter();
    for i in 0..5u8 {
        let flame_num = i + 1;
        let x = start_x + i as f32 * (STAR_SIZE + STAR_SPACING);
        let center = egui::pos2(x + STAR_SIZE / 2.0, center_y);
        
        // Determine flame color: hover > current rating > empty
        let is_filled = current_rating.map(|r| flame_num <= r).unwrap_or(false);
        let (icon, color) = if let Some(hover) = hover_flame {
            if flame_num <= hover {
                (regular::FIRE, flame_color_hover())
            } else if is_filled {
                (regular::FIRE, flame_color)
            } else {
                (regular::CIRCLE, FLAME_COLOR_EMPTY)
            }
        } else if is_filled {
            (regular::FIRE, flame_color)
        } else {
            (regular::CIRCLE, FLAME_COLOR_EMPTY)
        };
        
        // Draw flame or dot using phosphor icon
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(STAR_SIZE),
            color,
        );
    }
    
    // Show difficulty label after the flames (outside the allocated rect, so allocate more space)
    let display_rating = hover_flame.or(current_rating);
    if let Some(rating) = display_rating {
        let label = difficulty_label(rating);
        let label_color = difficulty_color(rating);
        
        // Add the label as a regular UI element so it flows properly
        ui.label(RichText::new(label).color(label_color).size(11.0));
    }
    
    // Handle click
    if response.clicked() {
        if let Some(rating) = hover_flame {
            clicked_rating = Some(rating);
        }
    }
    
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
    
    // Track positions for alternating backgrounds
    let mut row_tops: Vec<f32> = Vec::with_capacity(log_entries.len() + 1);
    
    for (_, entry) in log_entries.iter().enumerate() {
        // Record the top of this row
        row_tops.push(ui.cursor().min.y);
        
        match entry {
            LogEntry::Achievement { appid, apiname, game_name, achievement_name, timestamp, achievement_icon, game_icon_url } => {
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
                    
                    // Achievement name (clickable - navigates to game)
                    let is_selected = platform.get_log_selected_achievement()
                        .map(|(sel_appid, sel_apiname)| sel_appid == *appid && sel_apiname == *apiname)
                        .unwrap_or(false);
                    
                    let name_text = RichText::new(achievement_name).color(achievement_color).strong();
                    let name_response = ui.add(
                        egui::Label::new(name_text)
                            .selectable(false)
                            .sense(Sense::click())
                    );
                    
                    // Underline on hover or when selected
                    if name_response.hovered() || is_selected {
                        let rect = name_response.rect;
                        let underline_y = rect.bottom() - 1.0;
                        ui.painter().line_segment(
                            [egui::pos2(rect.left(), underline_y), egui::pos2(rect.right(), underline_y)],
                            egui::Stroke::new(1.0, achievement_color),
                        );
                    }
                    
                    // Pointer cursor on hover
                    if name_response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    
                    // Handle click to navigate to game
                    let clicked = icon_response.map(|r| r.clicked()).unwrap_or(false) || name_response.clicked();
                    if clicked {
                        // Set as selected in log and navigate to the game
                        platform.set_log_selected_achievement(*appid, apiname.clone());
                        platform.navigate_to_achievement(*appid, apiname.clone());
                    }
                    
                    // Star rating (inline after achievement name) - only show if authenticated
                    if platform.is_authenticated() {
                        ui.add_space(8.0);
                        let current_rating = platform.get_user_achievement_rating(*appid, apiname);
                        if let Some(rating) = star_rating_widget(ui, current_rating) {
                            platform.set_user_achievement_rating(*appid, apiname.clone(), rating);
                        }
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
    
    // Record final position and draw alternating backgrounds
    row_tops.push(ui.cursor().min.y);
    
    for i in (1..row_tops.len()).step_by(2) {
        // Draw stripe for odd rows (0-indexed, so rows 1, 3, 5...)
        let top = row_tops[i];
        let bottom = row_tops.get(i + 1).copied().unwrap_or(top + 24.0);
        let row_rect = egui::Rect::from_min_max(
            egui::pos2(ui.min_rect().left(), top),
            egui::pos2(ui.min_rect().right(), bottom)
        );
        ui.painter().rect_filled(row_rect, 2.0, alt_bg);
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
