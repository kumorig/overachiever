//! Shared UI components for desktop and WASM
//! 
//! This module provides platform-agnostic UI rendering using egui.
//! Platform-specific details (like image loading) are abstracted via traits.

mod stats_panel;
mod log_panel;
mod games_table;
mod ttb_dialog;

pub use stats_panel::*;
pub use log_panel::*;
pub use games_table::*;
pub use ttb_dialog::*;

use egui::{Response, RectAlign};
use egui::containers::Popup;

/// Show a tooltip immediately (no delay) positioned to the left
pub fn instant_tooltip(response: &Response, text: impl Into<String>) {
    if response.hovered() {
        let text = text.into();
        Popup::from_response(response)
            .align(RectAlign::LEFT_START)
            .gap(4.0)
            .show(|ui| { ui.label(&text); });
    }
}

/// Which panel is shown in the sidebar
#[derive(Clone, Copy, PartialEq, Default)]
pub enum SidebarPanel {
    #[default]
    Stats,
    Log,
}
