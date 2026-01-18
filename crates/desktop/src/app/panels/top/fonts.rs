//! Font settings application

use eframe::egui;

/// Apply font settings to the egui context
pub fn apply_font_settings(ctx: &egui::Context, config: &crate::config::Config) {
    let mut fonts = egui::FontDefinitions::default();

    // Add phosphor icons
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    // If a custom font is selected, try to load it
    if let Some(font_name) = &config.font_family {
        let installed_fonts = crate::fonts::get_installed_fonts();
        if let Some(font_path) = installed_fonts.get(font_name) {
            if let Some(font_data) = crate::fonts::load_font_data(font_path) {
                // Add the custom font
                fonts.font_data.insert(
                    "custom_font".to_owned(),
                    egui::FontData::from_owned(font_data).into(),
                );

                // Make it the primary font for proportional text
                fonts.families.get_mut(&egui::FontFamily::Proportional)
                    .map(|family| family.insert(0, "custom_font".to_owned()));
            }
        }
    }

    ctx.set_fonts(fonts);

    // Apply font size via style
    let mut style = (*ctx.style()).clone();
    style.text_styles.iter_mut().for_each(|(text_style, font_id)| {
        // Scale font sizes based on the configured size (14.0 is default)
        let scale = config.font_size / 14.0;
        match text_style {
            egui::TextStyle::Small => font_id.size = 10.0 * scale,
            egui::TextStyle::Body => font_id.size = 14.0 * scale,
            egui::TextStyle::Monospace => font_id.size = 14.0 * scale,
            egui::TextStyle::Button => font_id.size = 14.0 * scale,
            egui::TextStyle::Heading => font_id.size = 20.0 * scale,
            egui::TextStyle::Name(_) => {}
        }
    });
    style.interaction.tooltip_delay = 0.0;
    ctx.set_style(style);
}
