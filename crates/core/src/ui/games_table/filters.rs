//! Filter bar rendering for games table

use egui::{self, Color32, RichText, Ui};
use egui_phosphor::regular;
use super::platform::GamesTablePlatform;
use super::types::TriFilter;
use super::super::instant_tooltip;

/// Render the filter bar above the games table
pub fn render_filter_bar<P: GamesTablePlatform>(ui: &mut Ui, platform: &mut P) {
    // First row: name search and other filters
    ui.horizontal(|ui| {
        let mut filter_name = platform.filter_name().to_string();
        let response = ui.add(egui::TextEdit::singleline(&mut filter_name)
            .hint_text("Search by name...")
            .desired_width(150.0));
        if response.changed() {
            platform.set_filter_name(filter_name);
        }

        ui.add_space(10.0);

        // Achievements filter - tri-state toggle button (short label)
        let ach_label = format!("A: {}", platform.filter_achievements().label("Yes", "No"));
        let ach_btn = ui.button(&ach_label);
        if ach_btn.clicked() {
            let next = platform.filter_achievements().cycle();
            platform.set_filter_achievements(next);
        }
        instant_tooltip(&ach_btn, "Achievements");

        // Playtime filter - tri-state toggle button (short label)
        let play_label = format!("P: {}", platform.filter_playtime().label("Yes", "No"));
        let play_btn = ui.button(&play_label);
        if play_btn.clicked() {
            let next = platform.filter_playtime().cycle();
            platform.set_filter_playtime(next);
        }
        instant_tooltip(&play_btn, "Played");

        // Installed filter - only show on desktop (platform that can detect installed games)
        if platform.can_detect_installed() {
            let inst_label = format!("I: {}", platform.filter_installed().label("Yes", "No"));
            let inst_btn = ui.button(&inst_label);
            if inst_btn.clicked() {
                let next = platform.filter_installed().cycle();
                platform.set_filter_installed(next);
            }
            instant_tooltip(&inst_btn, "Installed");
        }

        // TTB filter - only show if platform shows TTB column
        if platform.show_ttb_column() {
            let ttb_label = format!("T: {}", platform.filter_ttb().label("Yes", "No"));
            let ttb_btn = ui.button(&ttb_label);
            if ttb_btn.clicked() {
                let next = platform.filter_ttb().cycle();
                platform.set_filter_ttb(next);
            }
            instant_tooltip(&ttb_btn, "Time to Beat");
        }

        // Hidden filter - tri-state toggle (All, Show Hidden, Hide Hidden)
        let hidden_label = format!("H: {}", platform.filter_hidden().label("Hidden", "Visible"));
        let hidden_btn = ui.button(&hidden_label);
        if hidden_btn.clicked() {
            let next = platform.filter_hidden().cycle();
            platform.set_filter_hidden(next);
        }
        instant_tooltip(&hidden_btn, "Private Games");

        // Clear filters button
        let has_filters = !platform.filter_name().is_empty()
            || platform.filter_achievements() != TriFilter::All
            || platform.filter_playtime() != TriFilter::All
            || (platform.can_detect_installed() && platform.filter_installed() != TriFilter::All)
            || (platform.show_ttb_column() && platform.filter_ttb() != TriFilter::All)
            || platform.filter_hidden() != TriFilter::Without  // Default is "Without" (hide hidden)
            || !platform.filter_tags().is_empty();

        if !has_filters {
            ui.add_enabled(false, egui::Button::new("Clear"));
        } else if ui.button("Clear").clicked() {
            platform.set_filter_name(String::new());
            platform.set_filter_achievements(TriFilter::All);
            platform.set_filter_playtime(TriFilter::All);
            if platform.can_detect_installed() {
                platform.set_filter_installed(TriFilter::All);
            }
            if platform.show_ttb_column() {
                platform.set_filter_ttb(TriFilter::All);
            }
            platform.set_filter_hidden(TriFilter::Without);  // Reset to default: hide hidden
            platform.set_filter_tags(Vec::new());
            platform.set_tag_search_input(String::new());
        }
    });

    // Second row: Tags filter with searchable dropdown and selected tag chips
    let available_tags: Vec<String> = platform.available_tags().to_vec();
    if !available_tags.is_empty() {
        // Get text input height for pills area (calculate once)
        let text_input_height = ui.text_style_height(&egui::TextStyle::Body) + 6.0;

        ui.horizontal(|ui| {
            // Searchable tag dropdown
            let mut search_input = platform.tag_search_input().to_string();
            let mut current_tags: Vec<String> = platform.filter_tags().to_vec();

            // Filter available tags based on search input and exclude already selected
            let search_lower = search_input.to_lowercase();
            let filtered_tags: Vec<String> = available_tags.iter()
                .filter(|tag| !current_tags.contains(tag))
                .filter(|tag| search_lower.is_empty() || tag.to_lowercase().contains(&search_lower))
                .take(20)
                .cloned()
                .collect();

            // Popup state management
            let popup_open_id = egui::Id::new("tag_popup_open_state");
            let mut popup_open = ui.ctx().memory(|mem| mem.data.get_temp::<bool>(popup_open_id).unwrap_or(false));

            // Text input with dropdown button
            let text_response = ui.add(
                egui::TextEdit::singleline(&mut search_input)
                    .hint_text("Search tags...")
                    .desired_width(120.0)
            );

            // Toggle button to open/close dropdown
            let toggle_btn = ui.button(if popup_open { regular::CARET_UP } else { regular::CARET_DOWN });
            if toggle_btn.clicked() {
                popup_open = !popup_open;
            }

            // Open popup when text field gains focus
            if text_response.gained_focus() {
                popup_open = true;
            }

            // Update search input in platform
            if text_response.changed() {
                platform.set_tag_search_input(search_input.clone());
                popup_open = true; // Open when typing
            }

            // Track if we selected a tag this frame
            let mut selected_tag: Option<String> = None;

            // Show dropdown as Area when open
            if popup_open && !filtered_tags.is_empty() {
                let area_response = egui::Area::new(egui::Id::new("tag_dropdown_area"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(text_response.rect.left_bottom())
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_min_width(340.0);
                            ui.style_mut().interaction.selectable_labels = true;
                            egui::ScrollArea::vertical()
                                .max_height(450.0)
                                .show(ui, |ui| {
                                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                                        for tag in &filtered_tags {
                                            let response = ui.selectable_label(false, tag);
                                            if response.clicked() {
                                                selected_tag = Some(tag.clone());
                                            }
                                        }
                                    });
                                });
                        });
                    });

                // Close popup if clicked outside both text field and dropdown area
                if ui.input(|i| i.pointer.any_click()) {
                    let click_pos = ui.input(|i| i.pointer.interact_pos());
                    if let Some(pos) = click_pos {
                        let in_text = text_response.rect.contains(pos);
                        let in_toggle = toggle_btn.rect.contains(pos);
                        let in_area = area_response.response.rect.contains(pos);
                        if !in_text && !in_toggle && !in_area {
                            popup_open = false;
                        }
                    }
                }
            } else if popup_open && filtered_tags.is_empty() {
                // Close if no tags match
                popup_open = false;
            }

            // Handle tag selection
            if let Some(tag) = selected_tag {
                if !current_tags.contains(&tag) {
                    current_tags.push(tag);
                    platform.set_filter_tags(current_tags.clone());
                }
                platform.set_tag_search_input(String::new());
                popup_open = false;
            }

            // Store popup state
            ui.ctx().memory_mut(|mem| mem.data.insert_temp(popup_open_id, popup_open));

            ui.add_space(8.0);

            // Pills area with subtle background - match height of text input
            let pills_rect = ui.available_rect_before_wrap();
            ui.painter().rect_filled(
                egui::Rect::from_min_size(pills_rect.min, egui::vec2(pills_rect.width(), text_input_height)),
                2.0,
                Color32::from_rgba_unmultiplied(40, 40, 50, 100)
            );

            // Display selected tags as removable chips (inline)
            let mut tags_to_remove: Vec<String> = Vec::new();

            ui.spacing_mut().item_spacing.x = 4.0;
            for tag in &current_tags {
                // Tag chip with X button combined
                let chip_text = format!("{} ×", tag);
                let chip_response = ui.add(
                    egui::Button::new(RichText::new(&chip_text).size(11.0))
                        .small()
                        .fill(Color32::from_rgb(60, 80, 100))
                );

                if chip_response.clicked() || chip_response.secondary_clicked() {
                    tags_to_remove.push(tag.clone());
                }
            }

            // Remove tags that were clicked
            if !tags_to_remove.is_empty() {
                let new_tags: Vec<String> = current_tags.into_iter()
                    .filter(|t| !tags_to_remove.contains(t))
                    .collect();
                platform.set_filter_tags(new_tags);
            }
        });
    }
}
