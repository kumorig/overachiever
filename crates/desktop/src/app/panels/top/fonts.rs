//! Font settings application

use eframe::egui;
use std::sync::Arc;

const SOURCE_HAN_SANS_PATH: &str = "assets/SourceHanSans.ttc";
const SOURCE_HAN_SANS_ID: &str = "source-han-sans";
const CUSTOM_FONT_ID: &str = "custom-ui-font";

/// Load Source Han Sans font data from file
fn get_source_han_sans_bytes() -> Option<Vec<u8>> {
    match std::fs::read(SOURCE_HAN_SANS_PATH) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            eprintln!("Failed to load {}: {}", SOURCE_HAN_SANS_PATH, err);
            None
        }
    }
}

/// Apply font settings to the egui context
pub fn apply_font_settings(ctx: &egui::Context, config: &crate::config::Config) {
    use crate::config::FontSource;
    
    let mut fonts = egui::FontDefinitions::default();

    // Add phosphor icons
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    // Apply font based on source
    match config.font_source {
        FontSource::BuiltIn => {
            // Use egui's built-in font (already set by default)
        }
        FontSource::Cjk => {
            // Use Source Han Sans with specified weight
            if let Some(bytes) = get_source_han_sans_bytes() {
                let font_data = egui::FontData {
                    font: std::borrow::Cow::Owned(bytes),
                    index: config.cjk_font_weight.index() as u32,
                    tweak: Default::default(),
                };
                
                fonts.font_data.insert(
                    SOURCE_HAN_SANS_ID.to_owned(),
                    Arc::new(font_data),
                );
                
                // Make it the primary font
                promote_font(&mut fonts, egui::FontFamily::Proportional, SOURCE_HAN_SANS_ID);
                promote_font(&mut fonts, egui::FontFamily::Monospace, SOURCE_HAN_SANS_ID);
            }
        }
        FontSource::System => {
            // Use system font if available
            if let Some(font_name) = &config.system_font_name {
                let installed_fonts = crate::fonts::get_installed_fonts();
                if let Some(font_path) = installed_fonts.get(font_name) {
                    if let Some(font_data) = crate::fonts::load_font_data(font_path) {
                        fonts.font_data.insert(
                            CUSTOM_FONT_ID.to_owned(),
                            Arc::new(egui::FontData::from_owned(font_data)),
                        );
                        
                        // Make it the primary font
                        promote_font(&mut fonts, egui::FontFamily::Proportional, CUSTOM_FONT_ID);
                        promote_font(&mut fonts, egui::FontFamily::Monospace, CUSTOM_FONT_ID);
                    }
                }
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

/// Promote a font to the front of the family list
fn promote_font(fonts: &mut egui::FontDefinitions, family: egui::FontFamily, font_id: &str) {
    let entries = fonts.families.entry(family).or_default();
    if let Some(pos) = entries.iter().position(|name| name == font_id) {
        if pos == 0 {
            return;
        }
        entries.remove(pos);
    }
    entries.insert(0, font_id.into());
}
