//! Reusable tag search component with searchable dropdown

use egui::{self, Color32, RichText, Ui};
use egui_phosphor::regular;

/// State for tag search dropdown
#[derive(Clone, Default)]
pub struct TagSearchState {
    /// Current search text
    pub search_text: String,
    /// Whether the dropdown is open
    pub popup_open: bool,
    /// Selected tags
    pub selected_tags: Vec<String>,
}

impl TagSearchState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Render a searchable tag dropdown with selected tag chips
/// 
/// Returns true if the selection changed this frame
pub fn render_tag_search(
    ui: &mut Ui,
    id_source: impl std::hash::Hash,
    state: &mut TagSearchState,
    available_tags: &[String],
    placeholder: &str,
    show_pills_bg: bool,
) -> bool {
    let mut selection_changed = false;
    
    // Filter available tags based on search input and exclude already selected
    let search_lower = state.search_text.to_lowercase();
    let filtered_tags: Vec<String> = available_tags.iter()
        .filter(|tag| !state.selected_tags.contains(tag))
        .filter(|tag| search_lower.is_empty() || tag.to_lowercase().contains(&search_lower))
        .take(20)
        .cloned()
        .collect();

    // Popup state management
    let popup_open_id = ui.make_persistent_id(id_source);
    let mut popup_open = ui.ctx().memory(|mem| mem.data.get_temp::<bool>(popup_open_id).unwrap_or(false));

    // Text input with dropdown button
    let text_response = ui.add(
        egui::TextEdit::singleline(&mut state.search_text)
            .hint_text(placeholder)
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

    // Update search input
    if text_response.changed() {
        popup_open = true; // Open when typing
    }

    // Track if we selected a tag this frame
    let mut selected_tag: Option<String> = None;

    // Show dropdown as Area when open
    if popup_open && !filtered_tags.is_empty() {
        let area_response = egui::Area::new(ui.make_persistent_id((popup_open_id, "dropdown_area")))
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
        if !state.selected_tags.contains(&tag) {
            state.selected_tags.push(tag);
            selection_changed = true;
        }
        state.search_text.clear();
        popup_open = false;
    }

    // Store popup state
    ui.ctx().memory_mut(|mem| mem.data.insert_temp(popup_open_id, popup_open));

    ui.add_space(8.0);

    // Pills area with optional subtle background
    if show_pills_bg {
        let text_input_height = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
        let pills_rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(
            egui::Rect::from_min_size(pills_rect.min, egui::vec2(pills_rect.width(), text_input_height)),
            2.0,
            Color32::from_rgba_unmultiplied(40, 40, 50, 100)
        );
    }

    // Display selected tags as removable chips (inline, with wrapping)
    let mut tags_to_remove: Vec<String> = Vec::new();

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().item_spacing.y = 4.0;
        for tag in &state.selected_tags {
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
    });

    // Remove tags that were clicked
    if !tags_to_remove.is_empty() {
        state.selected_tags.retain(|t| !tags_to_remove.contains(t));
        selection_changed = true;
    }
    
    selection_changed
}
