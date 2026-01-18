//! Games table panel - shared between desktop and WASM
//!
//! Renders: Filterable, sortable games list with expandable achievement details
//! Features: Column sorting, tri-state filters, expandable rows with achievements

mod types;
mod platform;
mod helpers;
mod filters;
mod table;
mod achievements;
mod ratings;

pub use types::{SortColumn, SortOrder, TriFilter};
pub use platform::GamesTablePlatform;
pub use helpers::{format_timestamp, format_ttb_times, sort_indicator, get_filtered_indices, sort_games};
pub use filters::render_filter_bar;
pub use table::render_games_table;
pub use achievements::render_achievements_list;
pub use ratings::{difficulty_label, difficulty_icon, difficulty_color, render_compact_avg_rating};
