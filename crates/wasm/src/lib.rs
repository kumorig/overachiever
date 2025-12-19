//! Overachiever WASM Frontend
//! 
//! Browser-based client that connects to the backend server via WebSocket.
//! All data is fetched from the server (Remote mode only).

#![cfg(target_arch = "wasm32")]

mod app;
mod ws_client;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen(start)]
pub fn main() {
    // Set up panic hook for better error messages
    console_error_panic_hook::set_once();
    
    // Initialize tracing for WASM
    tracing_wasm::set_as_global_default();
    
    // Get canvas element
    let document = web_sys::window()
        .expect("No window")
        .document()
        .expect("No document");
    let canvas = document
        .get_element_by_id("canvas")
        .expect("No canvas element")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("Canvas is not HtmlCanvasElement");
    
    // Start the eframe app
    let web_options = eframe::WebOptions::default();
    
    wasm_bindgen_futures::spawn_local(async move {
        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| {
                    egui_extras::install_image_loaders(&cc.egui_ctx);
                    let mut fonts = egui::FontDefinitions::default();
                    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
                    cc.egui_ctx.set_fonts(fonts);
                    Ok(Box::new(app::WasmApp::new()))
                }),
            )
            .await
            .expect("Failed to start eframe");
    });
}
