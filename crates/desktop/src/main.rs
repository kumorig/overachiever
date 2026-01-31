// Hide console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cjk_font;
mod cloud_sync;
mod config;
mod db;
mod fonts;
mod icon_cache;
mod steam_api;
mod steam_library;
mod steam_config;
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
            app::panels::top::fonts::apply_font_settings(&cc.egui_ctx, &config);

            Ok(Box::new(SteamOverachieverApp::new()))
        }),
    )
}
