//! Shared UI components for desktop and WASM
//! 
//! This module provides platform-agnostic UI rendering using egui.
//! Platform-specific details (like image loading) are abstracted via traits.

mod stats_panel;
mod log_panel;
mod games_table;

pub use stats_panel::*;
pub use log_panel::*;
pub use games_table::*;

/// Which panel is shown in the sidebar
#[derive(Clone, Copy, PartialEq, Default)]
pub enum SidebarPanel {
    #[default]
    Stats,
    Log,
}
