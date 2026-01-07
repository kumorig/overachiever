//! Games table panel - shared between desktop and WASM
//!
//! Renders: Filterable, sortable games list with expandable achievement details
//! Features: Column sorting, tri-state filters, expandable rows with achievements

use egui::{self, Color32, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular;

use crate::Game;
use super::{StatsPanelPlatform, instant_tooltip};

// ============================================================================
// Types
// ============================================================================

#[derive(Clone, Copy, PartialEq, Default)]
pub enum SortColumn {
    #[default]
    Name,
    LastPlayed,
    Playtime,
    AchievementsTotal,
    AchievementsPercent,
    TimeToBeat,
    Votes,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn toggle(&self) -> Self {
        match self {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        }
    }
}

/// Tri-state filter: All, Only With, Only Without
#[derive(Clone, Copy, PartialEq, Default)]
pub enum TriFilter {
    #[default]
    All,
    With,
    Without,
}

impl TriFilter {
    pub fn cycle(&self) -> Self {
        match self {
            TriFilter::All => TriFilter::With,
            TriFilter::With => TriFilter::Without,
            TriFilter::Without => TriFilter::All,
        }
    }
    
    pub fn label(&self, with_text: &str, without_text: &str) -> String {
        match self {
            TriFilter::All => "All".to_string(),
            TriFilter::With => with_text.to_string(),
            TriFilter::Without => without_text.to_string(),
        }
    }
}

// ============================================================================
// Games Table Platform Trait
// ============================================================================

/// Platform abstraction for the games table
/// 
/// This trait allows desktop and WASM to provide platform-specific
/// functionality (like icon loading, achievements fetching) while
/// sharing the table rendering logic.
pub trait GamesTablePlatform: StatsPanelPlatform {
    /// Get the current sort column
    fn sort_column(&self) -> SortColumn;
    
    /// Get the current sort order
    fn sort_order(&self) -> SortOrder;
    
    /// Set sort column and toggle order if same column
    fn set_sort(&mut self, column: SortColumn);
    
    /// Get filter text for name search
    fn filter_name(&self) -> &str;
    
    /// Set filter text for name search
    fn set_filter_name(&mut self, name: String);
    
    /// Get achievements filter state
    fn filter_achievements(&self) -> TriFilter;
    
    /// Set achievements filter state
    fn set_filter_achievements(&mut self, filter: TriFilter);
    
    /// Get playtime filter state
    fn filter_playtime(&self) -> TriFilter;
    
    /// Set playtime filter state
    fn set_filter_playtime(&mut self, filter: TriFilter);
    
    /// Check if a game row is expanded
    fn is_expanded(&self, appid: u64) -> bool;
    
    /// Toggle expanded state for a game
    fn toggle_expanded(&mut self, appid: u64);
    
    /// Get cached achievements for a game (if available)
    fn get_cached_achievements(&self, appid: u64) -> Option<&Vec<crate::GameAchievement>>;
    
    /// Request achievements to be loaded for a game
    fn request_achievements(&mut self, appid: u64);
    
    /// Get flash intensity for a row (for highlighting recently updated games)
    /// Returns 0.0-1.0 intensity, or None if not flashing
    fn get_flash_intensity(&self, _appid: u64) -> Option<f32> {
        None
    }
    
    /// Get the current navigation target (appid, apiname) for scroll-to behavior
    /// Returns None if no navigation is pending
    fn get_navigation_target(&self) -> Option<(u64, String)> {
        None
    }
    
    /// Clear the navigation target after scrolling to it
    fn clear_navigation_target(&mut self) {}
    
    /// Check if we need to scroll to the navigation target (one-time scroll)
    fn needs_scroll_to_target(&self) -> bool { false }
    
    /// Mark that we've scrolled to the target (call after scrolling)
    fn mark_scrolled_to_target(&mut self) {}
    
    /// Check if this platform supports refreshing a single game
    fn can_refresh_single_game(&self) -> bool { false }
    
    /// Request a refresh of achievements for a single game
    /// Returns true if the request was initiated, false if not supported or busy
    fn request_single_game_refresh(&mut self, _appid: u64) -> bool { false }
    
    /// Check if a single game refresh is in progress
    fn is_single_game_refreshing(&self, _appid: u64) -> bool { false }
    
    /// Check if this platform supports launching a Steam game
    fn can_launch_game(&self) -> bool { false }
    
    /// Launch a Steam game by appid
    fn launch_game(&mut self, _appid: u64) {}
    
    /// Check if a game is in launch cooldown (returns intensity 0.0-1.0, or None if not launching)
    fn get_launch_cooldown(&self, _appid: u64) -> Option<f32> { None }
    
    /// Check if this platform can detect installed games (desktop only)
    fn can_detect_installed(&self) -> bool { false }
    
    /// Check if a game is installed locally
    fn is_game_installed(&self, _appid: u64) -> bool { false }
    
    /// Install a Steam game by appid (opens Steam install dialog)
    fn install_game(&self, _appid: u64) {}
    
    /// Get installed games filter state
    fn filter_installed(&self) -> TriFilter { TriFilter::All }
    
    /// Set installed games filter state
    fn set_filter_installed(&mut self, _filter: TriFilter) {}
    
    // ============================================================================
    // TTB (Time To Beat) Methods
    // ============================================================================

    /// Check if TTB column and data should be displayed (always true for desktop)
    fn show_ttb_column(&self) -> bool { false }

    /// Check if this platform supports TTB fetching (requires admin mode on desktop)
    fn can_fetch_ttb(&self) -> bool { false }

    /// Fetch TTB times for a game (immediate, no rate limit)
    fn fetch_ttb(&mut self, _appid: u64, _game_name: &str) {}

    /// Get cached TTB times for a game
    fn get_ttb_times(&self, _appid: u64) -> Option<&crate::TtbTimes> { None }

    /// Check if currently fetching TTB for a game
    fn is_fetching_ttb(&self, _appid: u64) -> bool { false }

    /// Get TTB filter state
    fn filter_ttb(&self) -> TriFilter { TriFilter::All }

    /// Set TTB filter state
    fn set_filter_ttb(&mut self, _filter: TriFilter) {}

    /// Check if a game is in the TTB blacklist
    fn is_ttb_blacklisted(&self, _appid: u64) -> bool { false }

    /// Add a game to the TTB blacklist (admin only)
    fn add_to_ttb_blacklist(&mut self, _appid: u64, _game_name: &str) {}

    /// Remove a game from the TTB blacklist (admin only)
    fn remove_from_ttb_blacklist(&mut self, _appid: u64) {}

    /// Get the persisted name column width (default 400.0)
    fn name_column_width(&self) -> f32 { 400.0 }

    /// Set the name column width for persistence
    fn set_name_column_width(&mut self, _width: f32) {}

    // ============================================================================
    // Tag Methods (SteamSpy data)
    // ============================================================================

    /// Get the currently selected tag filters (empty = all games)
    fn filter_tags(&self) -> &[String] { &[] }

    /// Set the tag filters
    fn set_filter_tags(&mut self, _tags: Vec<String>) {}

    /// Get the tag search input text
    fn tag_search_input(&self) -> &str { "" }

    /// Set the tag search input text
    fn set_tag_search_input(&mut self, _input: String) {}

    /// Get available tags for dropdown
    fn available_tags(&self) -> &[String] { &[] }

    /// Get vote count for a specific tag on a game
    fn get_tag_vote_count(&self, _appid: u64, _tag_name: &str) -> Option<u32> { None }

    /// Check if game has any cached tags
    fn has_cached_tags(&self, _appid: u64) -> bool { false }

    /// Check if platform supports tag features (admin mode required)
    fn can_fetch_tags(&self) -> bool { false }

    /// Fetch tags for a game from SteamSpy
    fn fetch_tags(&mut self, _appid: u64) {}

    /// Check if currently fetching tags for a game
    fn is_fetching_tags(&self, _appid: u64) -> bool { false }
}

// ============================================================================
// Helper Functions
// ============================================================================

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
            SortOrder::Ascending => regular::CARET_UP,
            SortOrder::Descending => regular::CARET_DOWN,
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
                TriFilter::All => {}
                TriFilter::With => if !has_achievements { return false; }
                TriFilter::Without => if has_achievements { return false; }
            }
            // Playtime filter
            let has_playtime = g.rtime_last_played.map(|ts| ts > 0).unwrap_or(false);
            match platform.filter_playtime() {
                TriFilter::All => {}
                TriFilter::With => if !has_playtime { return false; }
                TriFilter::Without => if has_playtime { return false; }
            }
            // Installed filter (desktop only - if platform can detect installed games)
            if platform.can_detect_installed() {
                let is_installed = platform.is_game_installed(g.appid);
                match platform.filter_installed() {
                    TriFilter::All => {}
                    TriFilter::With => if !is_installed { return false; }
                    TriFilter::Without => if is_installed { return false; }
                }
            }
            // TTB filter (desktop only - if platform shows TTB column)
            if platform.show_ttb_column() {
                let has_ttb = platform.get_ttb_times(g.appid).is_some();
                match platform.filter_ttb() {
                    TriFilter::All => {}
                    TriFilter::With => if !has_ttb { return false; }
                    TriFilter::Without => if has_ttb { return false; }
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

// ============================================================================
// Render Functions
// ============================================================================

/// Render the filter bar above the games table
pub fn render_filter_bar<P: GamesTablePlatform>(ui: &mut Ui, platform: &mut P) {
    // First row: name search and other filters
    ui.horizontal(|ui| {
        let mut filter_name = platform.filter_name().to_string();
        let response = ui.add(egui::TextEdit::singleline(&mut filter_name)
            .hint_text("Search by name...")
            .desired_width(150.0));
        if response.changed() {
            platform.set_filter_name(filter_name);
        }

        ui.add_space(10.0);

        // Achievements filter - tri-state toggle button (short label)
        let ach_label = format!("A: {}", platform.filter_achievements().label("Yes", "No"));
        let ach_btn = ui.button(&ach_label);
        if ach_btn.clicked() {
            let next = platform.filter_achievements().cycle();
            platform.set_filter_achievements(next);
        }
        instant_tooltip(&ach_btn, "Achievements");

        // Playtime filter - tri-state toggle button (short label)
        let play_label = format!("P: {}", platform.filter_playtime().label("Yes", "No"));
        let play_btn = ui.button(&play_label);
        if play_btn.clicked() {
            let next = platform.filter_playtime().cycle();
            platform.set_filter_playtime(next);
        }
        instant_tooltip(&play_btn, "Played");

        // Installed filter - only show on desktop (platform that can detect installed games)
        if platform.can_detect_installed() {
            let inst_label = format!("I: {}", platform.filter_installed().label("Yes", "No"));
            let inst_btn = ui.button(&inst_label);
            if inst_btn.clicked() {
                let next = platform.filter_installed().cycle();
                platform.set_filter_installed(next);
            }
            instant_tooltip(&inst_btn, "Installed");
        }

        // TTB filter - only show if platform shows TTB column
        if platform.show_ttb_column() {
            let ttb_label = format!("T: {}", platform.filter_ttb().label("Yes", "No"));
            let ttb_btn = ui.button(&ttb_label);
            if ttb_btn.clicked() {
                let next = platform.filter_ttb().cycle();
                platform.set_filter_ttb(next);
            }
            instant_tooltip(&ttb_btn, "Time to Beat");
        }

        // Clear filters button
        let has_filters = !platform.filter_name().is_empty()
            || platform.filter_achievements() != TriFilter::All
            || platform.filter_playtime() != TriFilter::All
            || (platform.can_detect_installed() && platform.filter_installed() != TriFilter::All)
            || (platform.show_ttb_column() && platform.filter_ttb() != TriFilter::All)
            || !platform.filter_tags().is_empty();

        if !has_filters {
            ui.add_enabled(false, egui::Button::new("Clear"));
        } else if ui.button("Clear").clicked() {
            platform.set_filter_name(String::new());
            platform.set_filter_achievements(TriFilter::All);
            platform.set_filter_playtime(TriFilter::All);
            if platform.can_detect_installed() {
                platform.set_filter_installed(TriFilter::All);
            }
            if platform.show_ttb_column() {
                platform.set_filter_ttb(TriFilter::All);
            }
            platform.set_filter_tags(Vec::new());
            platform.set_tag_search_input(String::new());
        }
    });

    // Second row: Tags filter with searchable dropdown and selected tag chips
    let available_tags: Vec<String> = platform.available_tags().to_vec();
    if !available_tags.is_empty() {
        ui.horizontal(|ui| {
            // Searchable tag dropdown
            let mut search_input = platform.tag_search_input().to_string();
            let current_tags: Vec<String> = platform.filter_tags().to_vec();

            // Filter available tags based on search input and exclude already selected
            let search_lower = search_input.to_lowercase();
            let filtered_tags: Vec<&String> = available_tags.iter()
                .filter(|tag| !current_tags.contains(tag))
                .filter(|tag| search_lower.is_empty() || tag.to_lowercase().contains(&search_lower))
                .take(15) // Limit dropdown size
                .collect();

            // Custom searchable combobox using popup
            let popup_id = ui.make_persistent_id("tag_search_popup");
            let text_response = ui.add(
                egui::TextEdit::singleline(&mut search_input)
                    .hint_text("Search tags...")
                    .desired_width(120.0)
            );

            // Update search input in platform
            if text_response.changed() {
                platform.set_tag_search_input(search_input.clone());
            }

            // Show popup when text field is focused or has text
            let show_popup = text_response.has_focus() && (!filtered_tags.is_empty() || !search_input.is_empty());

            if show_popup {
                let popup_rect = egui::Rect::from_min_size(
                    text_response.rect.left_bottom(),
                    egui::vec2(180.0, 200.0)
                );

                egui::Area::new(popup_id)
                    .order(egui::Order::Foreground)
                    .fixed_pos(popup_rect.min)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(180.0)
                                .show(ui, |ui| {
                                    ui.set_min_width(170.0);
                                    if filtered_tags.is_empty() {
                                        ui.label("No matching tags");
                                    } else {
                                        for tag in filtered_tags {
                                            if ui.selectable_label(false, tag).clicked() {
                                                // Add tag to selection
                                                let mut new_tags = current_tags.clone();
                                                new_tags.push(tag.clone());
                                                platform.set_filter_tags(new_tags);
                                                platform.set_tag_search_input(String::new());
                                            }
                                        }
                                    }
                                });
                        });
                    });
            }

            ui.add_space(8.0);

            // Display selected tags as removable chips
            let mut tags_to_remove: Vec<String> = Vec::new();
            for tag in &current_tags {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;

                    // Tag chip with background
                    let chip_response = ui.add(
                        egui::Button::new(RichText::new(tag).size(11.0))
                            .small()
                            .fill(Color32::from_rgb(60, 80, 100))
                    );

                    // X button to remove
                    if ui.add(
                        egui::Button::new(RichText::new("×").size(11.0))
                            .small()
                            .fill(Color32::from_rgb(100, 60, 60))
                    ).clicked() || chip_response.secondary_clicked() {
                        tags_to_remove.push(tag.clone());
                    }
                });
                ui.add_space(4.0);
            }

            // Remove tags that were clicked
            if !tags_to_remove.is_empty() {
                let new_tags: Vec<String> = current_tags.into_iter()
                    .filter(|t| !tags_to_remove.contains(t))
                    .collect();
                platform.set_filter_tags(new_tags);
            }
        });
    }
}

/// Render the games table
///
/// Returns a list of appids that need their achievements fetched
pub fn render_games_table<P: GamesTablePlatform>(ui: &mut Ui, platform: &mut P, filtered_indices: Vec<usize>) -> Vec<u64> {
    let body_font_size = egui::TextStyle::Body.resolve(ui.style()).size;
    // Add vertical padding (8px base, scaled) to prevent text/button clipping
    let row_padding = 8.0;
    let text_height = body_font_size.max(ui.spacing().interact_size.y) + row_padding;

    // Scale row and header heights based on font size (14.0 is the default)
    let font_scale = body_font_size / 14.0;
    let header_height = (24.0 * font_scale).max(24.0); // Increased from 20.0
    let game_icon_size = 32.0 * font_scale;
    
    let available_height = ui.available_height();
    
    // Calculate row heights for each filtered game (including expanded content)
    // Scale expanded content heights based on font size
    let expanded_ach_height = text_height + 330.0 * font_scale;   // Extra height for achievement list
    let expanded_ttb_height = text_height + 60.0 * font_scale;    // Just TTB row, no achievements
    let expanded_empty_height = text_height + 40.0 * font_scale;  // Expanded but no content yet

    let row_heights: Vec<f32> = filtered_indices.iter().map(|&idx| {
        let game = &platform.games()[idx];
        let appid = game.appid;
        if platform.is_expanded(appid) {
            let has_achievements = game.achievements_total.map(|t| t > 0).unwrap_or(false);
            let has_ttb = platform.get_ttb_times(appid).is_some();
            if has_achievements {
                expanded_ach_height
            } else if has_ttb {
                expanded_ttb_height
            } else {
                expanded_empty_height
            }
        } else {
            text_height
        }
    }).collect();
    
    // Track which rows need achievement fetch
    let mut needs_fetch: Vec<u64> = Vec::new();
    
    // Clone needed data to avoid borrow issues during table rendering
    let games: Vec<_> = filtered_indices.iter()
        .map(|&idx| platform.games()[idx].clone())
        .collect();
    
    // Find navigation target row index if any (only if we need to scroll)
    let nav_row_index = if platform.needs_scroll_to_target() {
        platform.get_navigation_target().and_then(|(nav_appid, _)| {
            games.iter().position(|g| g.appid == nav_appid)
        })
    } else {
        None
    };
    
    let show_ttb_column = platform.show_ttb_column();
    let name_col_width = platform.name_column_width();
    let filter_tags: Vec<String> = platform.filter_tags().to_vec();
    let show_votes_column = !filter_tags.is_empty();

    // Scale fixed column widths based on font size (base widths are for 14pt)
    let last_played_width = (90.0 * font_scale).max(90.0);
    let playtime_width = (80.0 * font_scale).max(80.0);
    let achievements_width = (100.0 * font_scale).max(100.0);
    let percent_width = (60.0 * font_scale).max(60.0);
    let ttb_width = (60.0 * font_scale).max(60.0);
    let votes_width = (60.0 * font_scale).max(60.0);

    let mut table_builder = TableBuilder::new(ui)
        .id_salt("games_table")
        .striped(true)
        .resizable(false) // Table-level resizing disabled
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(name_col_width).at_least(200.0).clip(true).resizable(true)) // Name - resizable
        .column(Column::exact(last_played_width))  // Last Played - scaled
        .column(Column::exact(playtime_width))     // Playtime - scaled
        .column(Column::exact(achievements_width)) // Achievements - scaled
        .column(Column::exact(percent_width));     // Percent - scaled

    // Add TTB column if platform supports it
    if show_ttb_column {
        table_builder = table_builder.column(Column::exact(ttb_width)); // TTB - scaled
    }

    // Add Votes column if tag filter is active
    if show_votes_column {
        table_builder = table_builder.column(Column::exact(votes_width)); // Votes - scaled
    }

    table_builder = table_builder
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height);
    
    // Scroll to navigation target row if present
    // Note: Don't mark as scrolled here - let the achievement-level scroll do that
    // This ensures clicking a different achievement in the same game still scrolls
    if let Some(row_idx) = nav_row_index {
        table_builder = table_builder.scroll_to_row(row_idx, Some(egui::Align::Center));
    }
    
    // Track the actual column width for persistence
    let mut actual_name_col_width = name_col_width;

    table_builder.header(header_height, |mut header| {
            header.col(|ui| {
                // Capture the actual column width (available width in this cell)
                actual_name_col_width = ui.available_width();

                let indicator = sort_indicator(platform, SortColumn::Name);
                let label = if indicator.is_empty() { "Name".to_string() } else { format!("Name {}", indicator) };
                if ui.selectable_label(platform.sort_column() == SortColumn::Name, label).clicked() {
                    platform.set_sort(SortColumn::Name);
                }
            });
            header.col(|ui| {
                let indicator = sort_indicator(platform, SortColumn::LastPlayed);
                let label = if indicator.is_empty() { "Last Played".to_string() } else { format!("Last Played {}", indicator) };
                if ui.selectable_label(platform.sort_column() == SortColumn::LastPlayed, label).clicked() {
                    platform.set_sort(SortColumn::LastPlayed);
                }
            });
            header.col(|ui| {
                let indicator = sort_indicator(platform, SortColumn::Playtime);
                let label = if indicator.is_empty() { "Playtime".to_string() } else { format!("Playtime {}", indicator) };
                if ui.selectable_label(platform.sort_column() == SortColumn::Playtime, label).clicked() {
                    platform.set_sort(SortColumn::Playtime);
                }
            });
            header.col(|ui| {
                let indicator = sort_indicator(platform, SortColumn::AchievementsTotal);
                let label = if indicator.is_empty() { "Achievements".to_string() } else { format!("Achievements {}", indicator) };
                if ui.selectable_label(platform.sort_column() == SortColumn::AchievementsTotal, label).clicked() {
                    platform.set_sort(SortColumn::AchievementsTotal);
                }
            });
            header.col(|ui| {
                let indicator = sort_indicator(platform, SortColumn::AchievementsPercent);
                let label = if indicator.is_empty() { "%".to_string() } else { format!("% {}", indicator) };
                if ui.selectable_label(platform.sort_column() == SortColumn::AchievementsPercent, label).clicked() {
                    platform.set_sort(SortColumn::AchievementsPercent);
                }
            });
            if show_ttb_column {
                header.col(|ui| {
                    let indicator = sort_indicator(platform, SortColumn::TimeToBeat);
                    let label = if indicator.is_empty() { "TTB".to_string() } else { format!("TTB {}", indicator) };
                    let response = ui.selectable_label(platform.sort_column() == SortColumn::TimeToBeat, label);
                    if response.clicked() {
                        platform.set_sort(SortColumn::TimeToBeat);
                    }
                    instant_tooltip(&response, "Time to Beat");
                });
            }
            if show_votes_column {
                header.col(|ui| {
                    let indicator = sort_indicator(platform, SortColumn::Votes);
                    let label = if indicator.is_empty() { "Votes".to_string() } else { format!("Votes {}", indicator) };
                    let response = ui.selectable_label(platform.sort_column() == SortColumn::Votes, label);
                    if response.clicked() {
                        platform.set_sort(SortColumn::Votes);
                    }
                    instant_tooltip(&response, "Tag votes from SteamSpy");
                });
            }
        })
        .body(|body| {
            body.heterogeneous_rows(row_heights.into_iter(), |mut row| {
                let row_idx = row.index();
                let game = &games[row_idx];
                let appid = game.appid;
                let is_expanded = platform.is_expanded(appid);
                let has_achievements = game.achievements_total.map(|t| t > 0).unwrap_or(false);
                
                // Check if this game should be flashing
                let flash_color = platform.get_flash_intensity(appid).map(|intensity| {
                    egui::Color32::from_rgba_unmultiplied(
                        255,  // R
                        215,  // G (gold)
                        0,    // B
                        (intensity * 100.0) as u8
                    )
                });
                
                // Name column with expand/collapse toggle
                row.col(|ui| {
                    if let Some(color) = flash_color {
                        ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                    }
                    
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            // Expand/collapse button for all games
                            let icon = if is_expanded {
                                regular::CARET_DOWN
                            } else {
                                regular::CARET_RIGHT
                            };
                            if ui.small_button(icon.to_string()).clicked() {
                                platform.toggle_expanded(appid);
                                // Load achievements if not cached and expanding (only for games with achievements)
                                if !is_expanded && has_achievements && platform.get_cached_achievements(appid).is_none() {
                                    needs_fetch.push(appid);
                                }
                            }
                            
                            // Show game icon when expanded
                            if is_expanded {
                                if let Some(icon_hash) = &game.img_icon_url {
                                    if !icon_hash.is_empty() {
                                        let img_source = platform.game_icon_source(ui, appid, icon_hash);
                                        ui.add(
                                            egui::Image::new(img_source)
                                                .fit_to_exact_size(egui::vec2(game_icon_size, game_icon_size))
                                                .corner_radius(4.0)
                                        );
                                    }
                                }
                                ui.label(RichText::new(&game.name).strong());
                                
                                // Right-align the action buttons
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // Refresh button for single game update
                                    if platform.can_refresh_single_game() {
                                        let is_refreshing = platform.is_single_game_refreshing(appid);
                                        let btn = ui.add_enabled(
                                            !is_refreshing,
                                            egui::Button::new(regular::ARROWS_CLOCKWISE.to_string()).small()
                                        );
                                        if btn.clicked() {
                                            platform.request_single_game_refresh(appid);
                                        }
                                        super::instant_tooltip(&btn, "Refresh achievements for this game");
                                    }
                                    
                                    // Launch/Install button (desktop only)
                                    if platform.can_launch_game() {
                                        let is_installed = !platform.can_detect_installed() || platform.is_game_installed(appid);
                                        
                                        if is_installed {
                                            // Play button for installed games
                                            let cooldown = platform.get_launch_cooldown(appid);
                                            let is_launching = cooldown.is_some();
                                            
                                            // Highlight color when launching (green fading to normal)
                                            let btn = if let Some(intensity) = cooldown {
                                                let green = Color32::from_rgb(50, 180, 80);
                                                let normal = ui.visuals().widgets.inactive.weak_bg_fill;
                                                let color = Color32::from_rgb(
                                                    (normal.r() as f32 + (green.r() as f32 - normal.r() as f32) * intensity) as u8,
                                                    (normal.g() as f32 + (green.g() as f32 - normal.g() as f32) * intensity) as u8,
                                                    (normal.b() as f32 + (green.b() as f32 - normal.b() as f32) * intensity) as u8,
                                                );
                                                ui.add_enabled(
                                                    false,
                                                    egui::Button::new(regular::PLAY.to_string()).small().fill(color)
                                                )
                                            } else {
                                                ui.add(egui::Button::new(regular::PLAY.to_string()).small())
                                            };
                                            
                                            if btn.clicked() && !is_launching {
                                                platform.launch_game(appid);
                                            }
                                            let tooltip = if is_launching { "Launching..." } else { "Launch game in Steam" };
                                            super::instant_tooltip(&btn, tooltip);
                                        } else {
                                            // Install button for non-installed games
                                            let btn = ui.add(egui::Button::new(regular::DOWNLOAD_SIMPLE.to_string()).small());
                                            if btn.clicked() {
                                                platform.install_game(appid);
                                            }
                                            super::instant_tooltip(&btn, "Install game from Steam");
                                        }
                                    }
                                    
                                    // TTB fetch button (desktop only when ENABLE_TTB)
                                    if platform.can_fetch_ttb() {
                                        if platform.is_fetching_ttb(appid) {
                                            // Show spinner while fetching
                                            ui.spinner();
                                        } else {
                                            // Always show fetch button (allows re-fetching)
                                            let btn = ui.add(egui::Button::new(regular::CLOCK.to_string()).small());
                                            if btn.clicked() {
                                                platform.fetch_ttb(appid, &game.name);
                                            }
                                            let tooltip = if platform.get_ttb_times(appid).is_some() {
                                                "Re-fetch Time To Beat from HowLongToBeat"
                                            } else {
                                                "Fetch Time To Beat from HowLongToBeat"
                                            };
                                            super::instant_tooltip(&btn, tooltip);
                                        }
                                    }

                                    // Tags fetch button (admin mode only)
                                    if platform.can_fetch_tags() {
                                        if platform.is_fetching_tags(appid) {
                                            // Show spinner while fetching
                                            ui.spinner();
                                        } else {
                                            let btn = ui.add(egui::Button::new(regular::TAG.to_string()).small());
                                            if btn.clicked() {
                                                platform.fetch_tags(appid);
                                            }
                                            let tooltip = if platform.has_cached_tags(appid) {
                                                "Re-fetch tags from SteamSpy"
                                            } else {
                                                "Fetch tags from SteamSpy"
                                            };
                                            super::instant_tooltip(&btn, tooltip);
                                        }
                                    }
                                });
                            } else {
                                ui.label(&game.name);
                            }
                        });

                        // Show TTB data row if expanded and platform shows TTB column
                        if is_expanded && platform.show_ttb_column() {
                            let has_ttb = platform.get_ttb_times(appid).is_some();
                            let is_blacklisted = platform.is_ttb_blacklisted(appid);

                            if let Some(ttb) = platform.get_ttb_times(appid) {
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(24.0); // Indent to align with name
                                    ui.label(RichText::new("Time to Beat:").strong());
                                    // Check if we have any time data
                                    let has_data = ttb.main.is_some() || ttb.main_extra.is_some() || ttb.completionist.is_some();
                                    if has_data {
                                        if let Some(main) = ttb.main {
                                            ui.label(format!("Main: {:.0}h", main));
                                        }
                                        if let Some(extra) = ttb.main_extra {
                                            ui.label(format!("| +Extra: {:.0}h", extra));
                                        }
                                        if let Some(comp) = ttb.completionist {
                                            ui.label(format!("| 100%: {:.0}h", comp));
                                        }
                                    } else {
                                        // Scraped from HLTB but no time data available
                                        ui.label(RichText::new("<no data>").weak());
                                    }
                                });
                            }

                            // Show TTB blacklist button in admin mode
                            // Show "Not for TTB" for games without TTB data (to exclude from scan)
                            // Show "Allow TTB" for already blacklisted games (to re-enable)
                            if platform.can_fetch_ttb() && (!has_ttb || is_blacklisted) {
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(24.0); // Indent to align with name
                                    if is_blacklisted {
                                        // Game is blacklisted - offer to remove from blacklist
                                        let btn = ui.add(egui::Button::new(
                                            RichText::new(format!("{} Allow TTB", regular::CHECK))
                                                .color(egui::Color32::from_rgb(100, 180, 100))
                                        ).small());
                                        if btn.clicked() {
                                            platform.remove_from_ttb_blacklist(appid);
                                        }
                                        instant_tooltip(&btn, "Remove from TTB blacklist (allow in scan)");
                                        ui.label(RichText::new("Game is excluded from TTB scan").weak().italics());
                                    } else {
                                        // Game not blacklisted and has no TTB - offer to blacklist
                                        let btn = ui.add(egui::Button::new(
                                            RichText::new(format!("{} Not for TTB", regular::PROHIBIT))
                                                .color(egui::Color32::from_rgb(180, 100, 100))
                                        ).small());
                                        if btn.clicked() {
                                            platform.add_to_ttb_blacklist(appid, &game.name);
                                        }
                                        instant_tooltip(&btn, "Mark as not suitable for TTB (e.g., multiplayer-only games)");
                                    }
                                });
                            }
                        }

                        // Show achievements list if expanded (only for games with achievements)
                        if is_expanded && has_achievements {
                            render_achievements_list(ui, platform, appid);
                        }
                    });
                });
                
                // Only show other columns if not expanded
                row.col(|ui| {
                    if let Some(color) = flash_color {
                        ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                    }
                    if !is_expanded {
                        if let Some(ts) = game.rtime_last_played {
                            if ts > 0 {
                                ui.label(format_timestamp(ts));
                            } else {
                                ui.label("—");
                            }
                        } else {
                            ui.label("—");
                        }
                    }
                });
                
                row.col(|ui| {
                    if let Some(color) = flash_color {
                        ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                    }
                    if !is_expanded {
                        let never_played = game.rtime_last_played.map(|ts| ts == 0).unwrap_or(true);
                        if never_played {
                            ui.label("--");
                        } else {
                            ui.label(format!("{:.1}h", game.playtime_forever as f64 / 60.0));
                        }
                    }
                });
                
                row.col(|ui| {
                    if let Some(color) = flash_color {
                        ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                    }
                    if !is_expanded {
                        ui.label(game.achievements_display());
                    }
                });
                
                row.col(|ui| {
                    if let Some(color) = flash_color {
                        ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                    }
                    if !is_expanded {
                        if let Some(pct) = game.completion_percent() {
                            // Green for 100%, gray otherwise
                            let color = if pct >= 100.0 {
                                Color32::from_rgb(100, 255, 100)
                            } else {
                                Color32::GRAY
                            };
                            ui.label(RichText::new(format!("{:.0}%", pct)).color(color));
                        } else {
                            ui.label("—");
                        }
                    }
                });

                // TTB column (only if platform supports it)
                if show_ttb_column {
                    row.col(|ui| {
                        if let Some(color) = flash_color {
                            ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                        }
                        if !is_expanded {
                            if let Some(ttb) = platform.get_ttb_times(appid) {
                                // Check if any time data exists
                                if let Some(main) = ttb.main {
                                    ui.label(format!("{:.0}h", main));
                                } else if ttb.main_extra.is_some() || ttb.completionist.is_some() {
                                    // Has some other data, just not main
                                    ui.label("—");
                                } else {
                                    // Scraped but HLTB has no data for this game
                                    ui.label(RichText::new("n/a").weak());
                                }
                            } else {
                                // Not yet scraped
                                ui.label("—");
                            }
                        }
                    });
                }

                // Votes column (only if tag filter is active)
                if show_votes_column {
                    row.col(|ui| {
                        if let Some(color) = flash_color {
                            ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                        }
                        if !is_expanded {
                            // Sum votes for all selected tags
                            let total_votes: u32 = filter_tags.iter()
                                .filter_map(|tag| platform.get_tag_vote_count(appid, tag))
                                .sum();
                            if total_votes > 0 {
                                ui.label(format!("{}", total_votes));
                            } else {
                                ui.label("—");
                            }
                        }
                    });
                }
            });
        });

    // Persist column width if it changed significantly (more than 1px difference)
    if (actual_name_col_width - name_col_width).abs() > 1.0 {
        platform.set_name_column_width(actual_name_col_width);
    }

    needs_fetch
}

/// Render the achievements list for an expanded game row
fn render_achievements_list<P: GamesTablePlatform>(ui: &mut Ui, platform: &mut P, appid: u64) {
    // Check if we have a navigation target for this game
    let nav_target = platform.get_navigation_target();
    let target_apiname = nav_target
        .as_ref()
        .filter(|(nav_appid, _)| *nav_appid == appid)
        .map(|(_, apiname)| apiname.clone());

    // Calculate font scale for achievement row heights
    let body_font_size = egui::TextStyle::Body.resolve(ui.style()).size;
    let font_scale = body_font_size / 14.0;
    let ach_row_height = 52.0 * font_scale;
    let ach_icon_size = 48.0 * font_scale;
    let ach_scroll_height = 300.0 * font_scale;

    if let Some(achievements) = platform.get_cached_achievements(appid) {
        ui.add_space(4.0);
        ui.separator();

        // Sort achievements: unlocked first (by unlock time desc), then locked
        let mut sorted_achs: Vec<_> = achievements.iter().collect();
        sorted_achs.sort_by(|a, b| {
            match (a.achieved, b.achieved) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                (true, true) => b.unlocktime.cmp(&a.unlocktime),
                (false, false) => a.name.cmp(&b.name),
            }
        });

        // Collect data we need to avoid borrow issues
        let ach_data: Vec<_> = sorted_achs.iter().map(|ach| {
            (
                ach.apiname.clone(),
                ach.name.clone(),
                ach.achieved,
                if ach.achieved { ach.icon.clone() } else { ach.icon_gray.clone() },
                ach.description.clone(),
                ach.unlocktime,
            )
        }).collect();

        egui::ScrollArea::vertical().max_height(ach_scroll_height).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let is_authenticated = platform.is_authenticated();
            for (i, (apiname, name, achieved, icon_url, description, unlocktime)) in ach_data.iter().enumerate() {
                // Check if this is the navigation target
                let is_target = target_apiname.as_ref().map(|t| t == apiname).unwrap_or(false);

                let image_source = platform.achievement_icon_source(ui, icon_url);
                // Get user's own rating (for display purposes)
                let user_rating = if is_authenticated {
                    platform.get_user_achievement_rating(appid, apiname)
                } else {
                    None
                };
                // Get community average rating
                let avg_rating_data = platform.get_achievement_avg_rating(appid, apiname);

                // Alternate row background, or highlight if target
                let row_rect = ui.available_rect_before_wrap();
                let row_rect = egui::Rect::from_min_size(
                    row_rect.min,
                    egui::vec2(row_rect.width(), ach_row_height)
                );
                if is_target {
                    // Highlight the target achievement with a golden border
                    ui.painter().rect_filled(
                        row_rect,
                        4.0,
                        Color32::from_rgba_unmultiplied(255, 215, 0, 40) // Gold highlight
                    );
                    ui.painter().rect_stroke(
                        row_rect,
                        4.0,
                        egui::Stroke::new(2.0, Color32::from_rgb(255, 215, 0)),
                        egui::epaint::StrokeKind::Inside,
                    );
                    // Scroll to this row only if we haven't scrolled yet
                    if platform.needs_scroll_to_target() {
                        ui.scroll_to_rect(row_rect, Some(egui::Align::Center));
                        platform.mark_scrolled_to_target();
                    }
                } else if i % 2 == 1 {
                    ui.painter().rect_filled(
                        row_rect,
                        0.0,
                        ui.visuals().faint_bg_color
                    );
                }
                
                // Add top padding for the row content
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    // Add left padding so icon doesn't overlap the gold border
                    ui.add_space(4.0);
                    
                    let icon_response = ui.add(
                        egui::Image::new(image_source)
                            .fit_to_exact_size(egui::vec2(ach_icon_size, ach_icon_size))
                            .corner_radius(4.0)
                    );
                    
                    // Show unlock date on hover (instant, no delay)
                    if let Some(unlock_dt) = unlocktime {
                        instant_tooltip(&icon_response, unlock_dt.format("%Y-%m-%d").to_string());
                    }
                    
                    let name_text = if *achieved {
                        RichText::new(name).color(Color32::WHITE)
                    } else {
                        RichText::new(name).color(Color32::DARK_GRAY)
                    };
                    
                    let description_text = description.as_deref().unwrap_or("");
                    let desc_color = if *achieved {
                        Color32::GRAY
                    } else {
                        Color32::from_rgb(80, 80, 80)
                    };
                    
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        // Top row: name and date/stars
                        ui.horizontal(|ui| {
                            ui.label(name_text);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // Show compact average rating (read-only)
                                // Use average if available, otherwise show user's own rating
                                let (display_rating, count) = if let Some((avg, cnt)) = avg_rating_data {
                                    (Some(avg.round() as u8), Some(cnt))
                                } else {
                                    (user_rating, None)
                                };
                                render_compact_avg_rating(ui, display_rating, count);
                            });
                        });
                        // Description below, full width
                        if !description_text.is_empty() {
                            ui.label(RichText::new(description_text).color(desc_color));
                        }
                    });
                });
            }
        });
    } else {
        ui.spinner();
        ui.label("Loading achievements...");
    }
}

/// Get difficulty label for rating (with trailing space to avoid border clipping)
fn difficulty_label(rating: u8) -> &'static str {
    match rating {
        1 => "Very easy  ",
        2 => "Easy  ",
        3 => "Moderate  ",
        4 => "Hard  ",
        5 => "Extreme  ",
        _ => "",
    }
}

/// Get icon for difficulty rating (single icon per level)
fn difficulty_icon(rating: u8) -> &'static str {
    match rating {
        1 => "🐢",  // Turtle - Very easy
        2 => "🐇",  // Rabbit - Easy
        3 => "🏃",  // Runner - Moderate
        4 => "⚡",  // Lightning - Hard
        5 => "🔥",  // Fire - Extreme
        _ => "",
    }
}

/// Get color for difficulty label (green for easy, red for extreme)
fn difficulty_color(rating: u8) -> Color32 {
    match rating {
        1 => Color32::from_rgb(80, 200, 80),   // Green - Very easy
        2 => Color32::from_rgb(140, 200, 60),  // Yellow-green - Easy  
        3 => Color32::from_rgb(200, 200, 60),  // Yellow - Moderate
        4 => Color32::from_rgb(230, 140, 50),  // Orange - Hard
        5 => Color32::from_rgb(230, 60, 60),   // Red - Extreme
        _ => Color32::GRAY,
    }
}

/// Render compact average rating display (read-only, no interaction)
/// Shows a single difficulty icon with label and vote count
fn render_compact_avg_rating(ui: &mut Ui, avg_rating: Option<u8>, rating_count: Option<i32>) {
    let Some(rating) = avg_rating else {
        return; // Don't show anything if no rating
    };
    
    // Add count in parentheses first (since we're right-to-left)
    if let Some(count) = rating_count {
        ui.label(RichText::new(format!("({})", count)).color(Color32::GRAY).size(10.0));
        ui.add_space(4.0);
    }
    
    // Add difficulty label with gradient color
    ui.label(RichText::new(difficulty_label(rating)).color(difficulty_color(rating)).size(10.0));
    ui.add_space(4.0);
    
    // Single difficulty icon
    ui.label(RichText::new(difficulty_icon(rating)).color(difficulty_color(rating)).size(12.0));
}
