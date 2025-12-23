// Hide console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cloud_sync;
mod config;
mod db;
mod icon_cache;
mod steam_api;
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
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_icon(icon),
        ..Default::default()
    };
    
    eframe::run_native(
        "Overachiever v3",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);            
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(SteamOverachieverApp::new()))
        }),
    )
}
