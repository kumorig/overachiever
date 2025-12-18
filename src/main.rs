mod app;
mod db;
mod icon_cache;
mod models;
mod steam_api;
mod ui;

use app::SteamOverachieverApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Steam Overachiever v3",
        options,
        Box::new(|cc| {
            // Install image loaders for loading achievement icons from URLs
            egui_extras::install_image_loaders(&cc.egui_ctx);
            
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(SteamOverachieverApp::new()))
        }),
    )
}
