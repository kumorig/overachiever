// Hide console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cloud_sync;
mod config;
mod db;
mod fonts;
mod icon_cache;
mod steam_api;
mod steam_library;
mod steamspy;
mod ttb;
mod ui;

use app::SteamOverachieverApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    // Load icon for window
    let icon_data = include_bytes!("../../../assets/icon.png");
    let icon_image = image::load_from_memory(icon_data).expect("Failed to load icon");
    let icon_rgba = icon_image.to_rgba8();
    let (width, height) = icon_rgba.dimensions();
    let icon = egui::IconData {
        rgba: icon_rgba.into_raw(),
        width,
        height,
    };

    // Load config to get saved window state
    let config = config::Config::load();

    // Build viewport with saved or default size/position
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([
            config.window_width.unwrap_or(1024.0),
            config.window_height.unwrap_or(768.0),
        ])
        .with_icon(icon);

    // Apply saved position if available
    if let (Some(x), Some(y)) = (config.window_x, config.window_y) {
        viewport = viewport.with_position([x, y]);
    }

    // Apply maximized state
    if config.window_maximized {
        viewport = viewport.with_maximized(true);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Overachiever v3",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            // Load config and apply font settings
            let config = config::Config::load();
            apply_font_settings(&cc.egui_ctx, &config);

            Ok(Box::new(SteamOverachieverApp::new()))
        }),
    )
}

/// Apply font settings to the egui context (used on startup)
fn apply_font_settings(ctx: &egui::Context, config: &config::Config) {
    let mut fonts = egui::FontDefinitions::default();

    // Add phosphor icons
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    // If a custom font is selected, try to load it
    if let Some(font_name) = &config.font_family {
        let installed_fonts = fonts::get_installed_fonts();
        if let Some(font_path) = installed_fonts.get(font_name) {
            if let Some(font_data) = fonts::load_font_data(font_path) {
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
