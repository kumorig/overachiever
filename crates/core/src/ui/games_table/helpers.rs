//! Helper functions for games table

use super::platform::GamesTablePlatform;
use super::types::{SortColumn, SortOrder};
use crate::Game;

/// Format a Unix timestamp as YYYY-MM-DD
pub fn format_timestamp(ts: u32) -> String {
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".to_string())
}

/// Format TTB times as a compact string
pub fn format_ttb_times(ttb: &crate::TtbTimes) -> String {
    let mut parts = Vec::new();
    if let Some(main) = ttb.main {
        parts.push(format!("Main: {:.0}h", main));
    }
    if let Some(extra) = ttb.main_extra {
        parts.push(format!("+Extra: {:.0}h", extra));
    }
    if let Some(comp) = ttb.completionist {
        parts.push(format!("100%: {:.0}h", comp));
    }
    if parts.is_empty() {
        "<no data>".to_string()
    } else {
        parts.join(" | ")
    }
}

/// Get sort indicator icon for a column
pub fn sort_indicator(platform: &impl GamesTablePlatform, column: SortColumn) -> &'static str {
    if platform.sort_column() == column {
        match platform.sort_order() {
            SortOrder::Ascending => egui_phosphor::regular::CARET_UP,
            SortOrder::Descending => egui_phosphor::regular::CARET_DOWN,
        }
    } else {
        ""
    }
}

/// Get filtered indices based on current filters
pub fn get_filtered_indices(platform: &impl GamesTablePlatform) -> Vec<usize> {
    let filter_text = platform.filter_name();
    let filter_tags = platform.filter_tags();

    // Check if filtering by appid (starts with #)
    let appid_filter: Option<u64> = if filter_text.starts_with('#') {
        filter_text[1..].trim().parse().ok()
    } else {
        None
    };
    let filter_name_lower = filter_text.to_lowercase();

    platform.games().iter()
        .enumerate()
        .filter(|(_, g)| {
            // Name or AppID filter
            if let Some(appid) = appid_filter {
                // Filter by appid - must match exactly or be a prefix
                let appid_str = g.appid.to_string();
                let filter_str = appid.to_string();
                if !appid_str.starts_with(&filter_str) {
                    return false;
                }
            } else if !filter_name_lower.is_empty() && !g.name.to_lowercase().contains(&filter_name_lower) {
                return false;
            }
            // Achievements filter
            let has_achievements = g.achievements_total.map(|t| t > 0).unwrap_or(false);
            match platform.filter_achievements() {
                super::types::TriFilter::All => {}
                super::types::TriFilter::With => if !has_achievements { return false; }
                super::types::TriFilter::Without => if has_achievements { return false; }
            }
            // Playtime filter
            let has_playtime = g.rtime_last_played.map(|ts| ts > 0).unwrap_or(false);
            match platform.filter_playtime() {
                super::types::TriFilter::All => {}
                super::types::TriFilter::With => if !has_playtime { return false; }
                super::types::TriFilter::Without => if has_playtime { return false; }
            }
            // Installed filter (desktop only - if platform can detect installed games)
            if platform.can_detect_installed() {
                let is_installed = platform.is_game_installed(g.appid);
                match platform.filter_installed() {
                    super::types::TriFilter::All => {}
                    super::types::TriFilter::With => if !is_installed { return false; }
                    super::types::TriFilter::Without => if is_installed { return false; }
                }
            }
            // TTB filter (desktop only - if platform shows TTB column)
            if platform.show_ttb_column() {
                let has_ttb = platform.get_ttb_times(g.appid).is_some();
                match platform.filter_ttb() {
                    super::types::TriFilter::All => {}
                    super::types::TriFilter::With => if !has_ttb { return false; }
                    super::types::TriFilter::Without => if has_ttb { return false; }
                }
            }
            // Tag filter - only show games that have ALL selected tags
            if !filter_tags.is_empty() {
                for tag in filter_tags {
                    if platform.get_tag_vote_count(g.appid, tag).is_none() {
                        return false;
                    }
                }
            }

            // Hidden filter - hide games that are hidden (manually or from Steam)
            let is_hidden = g.hidden || g.steam_hidden;
            match platform.filter_hidden() {
                super::types::TriFilter::All => {}  // Show all
                super::types::TriFilter::With => if !is_hidden { return false; }  // Show only hidden
                super::types::TriFilter::Without => if is_hidden { return false; }  // Hide hidden (default)
            }

            true
        })
        .map(|(idx, _)| idx)
        .collect()
}

/// Sort games in place based on current sort settings
pub fn sort_games(games: &mut [Game], sort_column: SortColumn, sort_order: SortOrder) {
    match sort_column {
        SortColumn::Name => {
            games.sort_by(|a, b| {
                let cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
                if sort_order == SortOrder::Descending { cmp.reverse() } else { cmp }
            });
        }
        SortColumn::LastPlayed => {
            games.sort_by(|a, b| {
                let cmp = b.rtime_last_played.cmp(&a.rtime_last_played);
                if sort_order == SortOrder::Descending { cmp.reverse() } else { cmp }
            });
        }
        SortColumn::Playtime => {
            games.sort_by(|a, b| {
                let cmp = b.playtime_forever.cmp(&a.playtime_forever);
                if sort_order == SortOrder::Descending { cmp.reverse() } else { cmp }
            });
        }
        SortColumn::AchievementsTotal => {
            games.sort_by(|a, b| {
                let cmp = b.achievements_total.cmp(&a.achievements_total);
                if sort_order == SortOrder::Descending { cmp.reverse() } else { cmp }
            });
        }
        SortColumn::AchievementsPercent => {
            games.sort_by(|a, b| {
                let a_pct = a.completion_percent().unwrap_or(-1.0);
                let b_pct = b.completion_percent().unwrap_or(-1.0);
                let cmp = b_pct.partial_cmp(&a_pct).unwrap_or(std::cmp::Ordering::Equal);
                if sort_order == SortOrder::Descending { cmp.reverse() } else { cmp }
            });
        }
        SortColumn::TimeToBeat => {
            // TTB sorting requires access to platform cache, handled by platform-specific code
            // This is a no-op here; desktop overrides set_sort to handle TTB
        }
        SortColumn::Votes => {
            // Votes sorting requires access to tags cache, handled by platform-specific code
            // This is a no-op here; desktop overrides set_sort to handle Votes
        }
    }
}
