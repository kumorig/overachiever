//! Games table panel - shared between desktop and WASM
//!
//! Renders: Filterable, sortable games list with expandable achievement details
//! Features: Column sorting, tri-state filters, expandable rows with achievements

use egui::{self, Color32, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular;

use crate::Game;
use super::StatsPanelPlatform;

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
    let filter_name_lower = platform.filter_name().to_lowercase();
    
    platform.games().iter()
        .enumerate()
        .filter(|(_, g)| {
            // Name filter
            if !filter_name_lower.is_empty() && !g.name.to_lowercase().contains(&filter_name_lower) {
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
    }
}

// ============================================================================
// Render Functions
// ============================================================================

/// Render the filter bar above the games table
pub fn render_filter_bar<P: GamesTablePlatform>(ui: &mut Ui, platform: &mut P) {
    ui.horizontal(|ui| {
        ui.label("Filter:");
        
        let mut filter_name = platform.filter_name().to_string();
        let response = ui.add(egui::TextEdit::singleline(&mut filter_name)
            .hint_text("Search by name...")
            .desired_width(150.0));
        if response.changed() {
            platform.set_filter_name(filter_name);
        }
        
        ui.add_space(10.0);
        
        // Achievements filter - tri-state toggle button
        let ach_label = format!("Achievements: {}", platform.filter_achievements().label("With", "Without"));
        if ui.button(&ach_label).clicked() {
            let next = platform.filter_achievements().cycle();
            platform.set_filter_achievements(next);
        }
        
        // Playtime filter - tri-state toggle button
        let play_label = format!("Played: {}", platform.filter_playtime().label("Yes", "No"));
        if ui.button(&play_label).clicked() {
            let next = platform.filter_playtime().cycle();
            platform.set_filter_playtime(next);
        }
        
        // Clear filters button
        let has_filters = !platform.filter_name().is_empty() 
            || platform.filter_achievements() != TriFilter::All 
            || platform.filter_playtime() != TriFilter::All;
        
        if !has_filters {
            ui.add_enabled(false, egui::Button::new("Clear"));
        } else if ui.button("Clear").clicked() {
            platform.set_filter_name(String::new());
            platform.set_filter_achievements(TriFilter::All);
            platform.set_filter_playtime(TriFilter::All);
        }
    });
}

/// Render the games table
/// 
/// Returns a list of appids that need their achievements fetched
pub fn render_games_table<P: GamesTablePlatform>(ui: &mut Ui, platform: &mut P, filtered_indices: Vec<usize>) -> Vec<u64> {
    let text_height = egui::TextStyle::Body
        .resolve(ui.style())
        .size
        .max(ui.spacing().interact_size.y);
    
    let available_height = ui.available_height();
    
    // Calculate row heights for each filtered game (including expanded achievements)
    let row_heights: Vec<f32> = filtered_indices.iter().map(|&idx| {
        let appid = platform.games()[idx].appid;
        if platform.is_expanded(appid) {
            text_height + 330.0 // Extra height for achievement list
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
    
    TableBuilder::new(ui)
        .striped(true)
        .resizable(false) // Table-level resizing disabled
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::remainder().at_least(200.0).clip(true).resizable(true)) // Name - resizable
        .column(Column::exact(90.0))  // Last Played - fixed
        .column(Column::exact(80.0))  // Playtime - fixed
        .column(Column::exact(100.0)) // Achievements - fixed
        .column(Column::exact(60.0))  // Percent - fixed
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(20.0, |mut header| {
            header.col(|ui| {
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
                            // Expand/collapse button for games with achievements
                            if has_achievements {
                                let icon = if is_expanded { 
                                    regular::CARET_DOWN 
                                } else { 
                                    regular::CARET_RIGHT 
                                };
                                if ui.small_button(icon.to_string()).clicked() {
                                    platform.toggle_expanded(appid);
                                    // Load achievements if not cached and expanding
                                    if !is_expanded && platform.get_cached_achievements(appid).is_none() {
                                        needs_fetch.push(appid);
                                    }
                                }
                            } else {
                                ui.add_space(20.0);
                            }
                            
                            // Show game icon when expanded
                            if is_expanded {
                                if let Some(icon_hash) = &game.img_icon_url {
                                    if !icon_hash.is_empty() {
                                        let img_source = platform.game_icon_source(ui, appid, icon_hash);
                                        ui.add(
                                            egui::Image::new(img_source)
                                                .fit_to_exact_size(egui::vec2(32.0, 32.0))
                                                .corner_radius(4.0)
                                        );
                                    }
                                }
                                ui.label(RichText::new(&game.name).strong());
                            } else {
                                ui.label(&game.name);
                            }
                        });
                        
                        // Show achievements list if expanded
                        if is_expanded {
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
            });
        });
    
    needs_fetch
}

/// Render the achievements list for an expanded game row
fn render_achievements_list<P: GamesTablePlatform>(ui: &mut Ui, platform: &P, appid: u64) {
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
        
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (i, ach) in sorted_achs.iter().enumerate() {
                let icon_url = if ach.achieved {
                    &ach.icon
                } else {
                    &ach.icon_gray
                };
                
                let image_source = platform.achievement_icon_source(ui, icon_url);
                
                // Alternate row background
                let row_rect = ui.available_rect_before_wrap();
                let row_rect = egui::Rect::from_min_size(
                    row_rect.min,
                    egui::vec2(row_rect.width(), 52.0)
                );
                if i % 2 == 1 {
                    ui.painter().rect_filled(
                        row_rect,
                        0.0,
                        ui.visuals().faint_bg_color
                    );
                }
                
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::new(image_source)
                            .fit_to_exact_size(egui::vec2(48.0, 48.0))
                            .corner_radius(4.0)
                    );
                    
                    let name_text = if ach.achieved {
                        RichText::new(&ach.name).color(Color32::WHITE)
                    } else {
                        RichText::new(&ach.name).color(Color32::DARK_GRAY)
                    };
                    
                    let description_text = ach.description.as_deref().unwrap_or("");
                    let desc_color = if ach.achieved {
                        Color32::GRAY
                    } else {
                        Color32::from_rgb(80, 80, 80)
                    };
                    
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        // Top row: name and date
                        ui.horizontal(|ui| {
                            ui.label(name_text);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if let Some(unlock_dt) = &ach.unlocktime {
                                    ui.label(
                                        RichText::new(unlock_dt.format("%Y-%m-%d").to_string())
                                            .color(Color32::from_rgb(100, 200, 100))
                                    );
                                }
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
