//! Achievement list rendering for expanded game rows

use egui::{self, Color32, RichText, Ui};
use super::platform::GamesTablePlatform;
use super::super::instant_tooltip;

/// Render the achievements list for an expanded game row
pub fn render_achievements_list<P: GamesTablePlatform>(ui: &mut Ui, platform: &mut P, appid: u64) {
    // Check if we have a navigation target for this game
    let nav_target = platform.get_navigation_target();
    let target_apiname = nav_target
        .as_ref()
        .filter(|(nav_appid, _)| *nav_appid == appid)
        .map(|(_, apiname)| apiname.clone());

    // Calculate font scale for achievement row heights
    let body_font_size = egui::TextStyle::Body.resolve(ui.style()).size;
    let font_scale = body_font_size / 14.0;
    let ach_row_height = 52.0 * font_scale;
    let ach_icon_size = 48.0 * font_scale;
    let ach_scroll_height = 300.0 * font_scale;

    if let Some(achievements) = platform.get_cached_achievements(appid) {
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

        // Collect data we need to avoid borrow issues
        let ach_data: Vec<_> = sorted_achs.iter().map(|ach| {
            (
                ach.apiname.clone(),
                ach.name.clone(),
                ach.achieved,
                if ach.achieved { ach.icon.clone() } else { ach.icon_gray.clone() },
                ach.description.clone(),
                ach.unlocktime,
            )
        }).collect();

        egui::ScrollArea::vertical().max_height(ach_scroll_height).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let is_authenticated = platform.is_authenticated();
            for (i, (apiname, name, achieved, icon_url, description, unlocktime)) in ach_data.iter().enumerate() {
                // Check if this is the navigation target
                let is_target = target_apiname.as_ref().map(|t| t == apiname).unwrap_or(false);

                let image_source = platform.achievement_icon_source(ui, icon_url);
                // Get user's own rating (for display purposes)
                let user_rating = if is_authenticated {
                    platform.get_user_achievement_rating(appid, apiname)
                } else {
                    None
                };
                // Get community average rating
                let avg_rating_data = platform.get_achievement_avg_rating(appid, apiname);

                // Alternate row background, or highlight if target
                let row_rect = ui.available_rect_before_wrap();
                let row_rect = egui::Rect::from_min_size(
                    row_rect.min,
                    egui::vec2(row_rect.width(), ach_row_height)
                );
                if is_target {
                    // Highlight the target achievement with a golden border
                    ui.painter().rect_filled(
                        row_rect,
                        4.0,
                        Color32::from_rgba_unmultiplied(255, 215, 0, 40) // Gold highlight
                    );
                    ui.painter().rect_stroke(
                        row_rect,
                        4.0,
                        egui::Stroke::new(2.0, Color32::from_rgb(255, 215, 0)),
                        egui::epaint::StrokeKind::Inside,
                    );
                    // Scroll to this row only if we haven't scrolled yet
                    if platform.needs_scroll_to_target() {
                        ui.scroll_to_rect(row_rect, Some(egui::Align::Center));
                        platform.mark_scrolled_to_target();
                    }
                } else if i % 2 == 1 {
                    ui.painter().rect_filled(
                        row_rect,
                        0.0,
                        ui.visuals().faint_bg_color
                    );
                }
                
                // Add top padding for the row content
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    // Add left padding so icon doesn't overlap the gold border
                    ui.add_space(4.0);
                    
                    let icon_response = ui.add(
                        egui::Image::new(image_source)
                            .fit_to_exact_size(egui::vec2(ach_icon_size, ach_icon_size))
                            .corner_radius(4.0)
                    );
                    
                    // Show unlock date on hover (instant, no delay)
                    if let Some(unlock_dt) = unlocktime {
                        instant_tooltip(&icon_response, unlock_dt.format("%Y-%m-%d").to_string());
                    }
                    
                    let name_text = if *achieved {
                        RichText::new(name).color(Color32::WHITE)
                    } else {
                        RichText::new(name).color(Color32::DARK_GRAY)
                    };
                    
                    let description_text = description.as_deref().unwrap_or("");
                    let desc_color = if *achieved {
                        Color32::GRAY
                    } else {
                        Color32::from_rgb(80, 80, 80)
                    };
                    
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        // Top row: name and date/stars
                        ui.horizontal(|ui| {
                            ui.label(name_text);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // Show compact average rating (read-only)
                                // Use average if available, otherwise show user's own rating
                                let (display_rating, count) = if let Some((avg, cnt)) = avg_rating_data {
                                    (Some(avg.round() as u8), Some(cnt))
                                } else {
                                    (user_rating, None)
                                };
                                super::ratings::render_compact_avg_rating(ui, display_rating, count);
                            });
                        });
                        // Description below, full width
                        if !description_text.is_empty() {
                            ui.label(RichText::new(description_text).color(desc_color));
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
