//! Stats panel - shared between desktop and WASM
//! 
//! Renders: Games over time graph, achievement progress, breakdown stats

use egui::{self, Color32, RichText, Ui};
use egui_plot::{Line, Plot, PlotPoints};
use egui_phosphor::regular;

use crate::{Game, RunHistory, AchievementHistory, LogEntry};

/// Platform-specific operations needed for the stats panel
pub trait StatsPanelPlatform {
    /// Get the list of games
    fn games(&self) -> &[Game];
    
    /// Get run history data
    fn run_history(&self) -> &[RunHistory];
    
    /// Get achievement history data  
    fn achievement_history(&self) -> &[AchievementHistory];
    
    /// Get log entries
    fn log_entries(&self) -> &[LogEntry];
    
    /// Whether to include unplayed games in average calculation
    fn include_unplayed_in_avg(&self) -> bool;
    
    /// Set the include_unplayed_in_avg toggle
    fn set_include_unplayed_in_avg(&mut self, value: bool);
    
    /// Resolve a game icon URL to an ImageSource
    /// `appid` and `icon_hash` are provided for building the URL
    fn game_icon_source(&self, ui: &Ui, appid: u64, icon_hash: &str) -> egui::ImageSource<'static>;
    
    /// Resolve an achievement icon URL to an ImageSource
    fn achievement_icon_source(&self, ui: &Ui, icon_url: &str) -> egui::ImageSource<'static>;
    
    // ========================================================================
    // Graph tab state (for switching between different graph views)
    // ========================================================================
    
    /// Get the current games graph tab (0 = Total Games, 1 = Unplayed Games)
    fn games_graph_tab(&self) -> usize { 0 }
    
    /// Set the games graph tab
    fn set_games_graph_tab(&mut self, _tab: usize) {}
    
    /// Get the current achievement graph tab (0 = Avg Game Completion %, 1 = Overall Achievement %)
    fn achievements_graph_tab(&self) -> usize { 0 }
    
    /// Set the achievement graph tab
    fn set_achievements_graph_tab(&mut self, _tab: usize) {}
    
    // ========================================================================
    // Achievement rating and selection (optional - default implementations)
    // ========================================================================
    
    /// Check if user is authenticated (has cloud token) - needed for ratings/comments
    fn is_authenticated(&self) -> bool { false }
    
    /// Check if an achievement is selected (for multi-select commenting)
    fn is_achievement_selected(&self, _appid: u64, _apiname: &str) -> bool { false }
    
    /// Toggle achievement selection
    fn toggle_achievement_selection(&mut self, _appid: u64, _apiname: String, _name: String) {}
    
    /// Get all selected achievements as (appid, apiname, name) tuples
    fn selected_achievements(&self) -> Vec<(u64, String, String)> { Vec::new() }
    
    /// Clear all selections
    fn clear_achievement_selections(&mut self) {}
    
    /// Submit an achievement rating (1-5 stars)
    fn submit_achievement_rating(&mut self, _appid: u64, _apiname: String, _rating: u8) {}
    
    /// Get the user's rating for an achievement (1-5 stars, or None if not rated)
    fn get_user_achievement_rating(&self, _appid: u64, _apiname: &str) -> Option<u8> {
        None
    }
    
    /// Set the user's rating for an achievement (stores locally and submits to server)
    fn set_user_achievement_rating(&mut self, _appid: u64, _apiname: String, _rating: u8) {}
    
    /// Submit a comment for selected achievements
    fn submit_achievement_comment(&mut self, _comment: String) {}
    
    /// Get the current comment text being edited
    fn pending_comment(&self) -> &str { "" }
    
    /// Set the pending comment text
    fn set_pending_comment(&mut self, _comment: String) {}
    
    // ========================================================================
    // Navigation (for clicking on achievements in log to scroll to game)
    // ========================================================================
    
    /// Navigate to a specific achievement in the games table
    /// This should: scroll to the game, expand it, and scroll to the achievement
    fn navigate_to_achievement(&mut self, _appid: u64, _apiname: String) {}
    
    /// Get the last clicked achievement in the log (for persistent highlight)
    fn get_log_selected_achievement(&self) -> Option<(u64, String)> { None }
    
    /// Set the last clicked achievement in the log
    fn set_log_selected_achievement(&mut self, _appid: u64, _apiname: String) {}
    
    // ========================================================================
    // Community ratings (average ratings from all users)
    // ========================================================================
    
    /// Get the community average rating for an achievement (avg, count)
    /// Returns None if no community ratings exist
    fn get_achievement_avg_rating(&self, _appid: u64, _apiname: &str) -> Option<(f32, i32)> {
        None
    }
}

/// Configuration for how the stats panel should render
#[derive(Clone, Copy)]
pub struct StatsPanelConfig {
    /// Fixed height for plots (None = use view_aspect)
    pub plot_height: Option<f32>,
    /// Whether to show axes on plots
    pub show_plot_axes: bool,
    /// Whether to allow plot interaction (drag/zoom/scroll)
    pub allow_plot_interaction: bool,
}

impl Default for StatsPanelConfig {
    fn default() -> Self {
        Self {
            plot_height: None,
            show_plot_axes: true,
            allow_plot_interaction: true,
        }
    }
}

impl StatsPanelConfig {
    /// Config suitable for WASM (compact, no interaction)
    pub fn wasm() -> Self {
        Self {
            plot_height: Some(120.0),
            show_plot_axes: false,
            allow_plot_interaction: false,
        }
    }
    
    /// Config suitable for desktop (interactive, aspect-based sizing)
    pub fn desktop() -> Self {
        Self {
            plot_height: None,
            show_plot_axes: true,
            allow_plot_interaction: true,
        }
    }
}

// ============================================================================
// Rendering Functions
// ============================================================================

/// Render the complete stats panel content (inside a scroll area)
pub fn render_stats_content<P: StatsPanelPlatform>(
    ui: &mut Ui,
    platform: &mut P,
    config: &StatsPanelConfig,
) {
    render_games_over_time(ui, platform, config);
    ui.add_space(16.0);
    render_achievement_progress(ui, platform, config);
    ui.add_space(16.0);
    render_breakdown(ui, platform);
}

/// Calculate Y-axis bounds with padding for unbounded values (e.g. game counts)
fn calc_y_bounds_unbounded(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 100.0);
    }
    let min_y = values.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
    let max_y = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    
    // Add some padding (10% of range, minimum 1.0 for game counts)
    let range = max_y - min_y;
    let padding = (range * 0.1).max(1.0);
    ((min_y - padding).max(0.0), max_y + padding)
}

/// Render the "Games Over Time" graph
pub fn render_games_over_time<P: StatsPanelPlatform>(
    ui: &mut Ui,
    platform: &mut P,
    config: &StatsPanelConfig,
) {
    ui.heading("Games Over Time");
    ui.separator();
    
    // Get current tab before any borrows
    let current_tab = platform.games_graph_tab();
    
    // Tab buttons for switching between graph views
    let mut new_tab = current_tab;
    ui.horizontal(|ui| {
        if ui.selectable_label(current_tab == 0, "Total Games").clicked() {
            new_tab = 0;
        }
        if ui.selectable_label(current_tab == 1, "Unplayed Games").clicked() {
            new_tab = 1;
        }
    });
    
    // Apply tab change if needed
    if new_tab != current_tab {
        platform.set_games_graph_tab(new_tab);
    }
    
    let run_history = platform.run_history();
    
    ui.add_space(4.0);
    
    // Build data for the selected tab
    let (points, y_min, y_max, line_name, line_color) = if run_history.is_empty() {
        // Empty plot - still need to show it for WASM layout
        (PlotPoints::default(), 0.0, 100.0, "Total Games", Color32::from_rgb(100, 180, 255))
    } else if new_tab == 0 {
        // Total Games graph
        let values: Vec<f64> = run_history.iter().map(|h| h.total_games as f64).collect();
        let pts: PlotPoints = run_history.iter().enumerate()
            .map(|(i, h)| [i as f64, h.total_games as f64]).collect();
        let (y_min, y_max) = calc_y_bounds_unbounded(&values);
        (pts, y_min, y_max, "Total Games", Color32::from_rgb(100, 180, 255))
    } else {
        // Unplayed Games graph
        let values: Vec<f64> = run_history.iter().map(|h| h.unplayed_games as f64).collect();
        let pts: PlotPoints = run_history.iter().enumerate()
            .map(|(i, h)| [i as f64, h.unplayed_games as f64]).collect();
        let (y_min, y_max) = calc_y_bounds_unbounded(&values);
        (pts, y_min, y_max, "Unplayed Games", Color32::from_rgb(255, 150, 100))
    };
    
    let line = Line::new(line_name, points).color(line_color);
    
    // Reset zoom when switching tabs so the new line fits in view
    let tab_changed = new_tab != current_tab;
    
    // Use consistent plot ID - changing IDs can cause WASM layout issues
    let mut plot = Plot::new("games_history")
        .auto_bounds(egui::Vec2b::new(true, true))
        .include_y(y_min)
        .include_y(y_max);
    
    if tab_changed {
        plot = plot.reset();
    }
    
    if let Some(height) = config.plot_height {
        plot = plot.height(height).width(ui.available_width());
    } else {
        plot = plot.view_aspect(2.0);
    }
    
    if !config.show_plot_axes {
        plot = plot.show_axes([false, true]);
    }
    
    if !config.allow_plot_interaction {
        plot = plot
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false);
    }
    
    plot.show(ui, |plot_ui| {
        plot_ui.line(line);
    });
    
    if run_history.is_empty() {
        ui.label("No history yet. Sync to start tracking!");
    } else {
        // Debug: show data point count
        ui.small(format!("{} data points", run_history.len()));
    }
}

/// Calculate Y-axis bounds with padding for percentage values (0-100 clamped)
fn calc_y_bounds(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 100.0);
    }
    let min_y = values.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
    let max_y = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max).min(100.0);
    
    // Use the full range as padding for tight zoom on flat lines
    let range = max_y - min_y;
    let padding = range.max(0.01);
    ((min_y - padding).max(0.0), (max_y + padding).min(100.0))
}

/// Render the "Achievement Progress" graph with stats below
pub fn render_achievement_progress<P: StatsPanelPlatform>(
    ui: &mut Ui,
    platform: &mut P,
    config: &StatsPanelConfig,
) {
    ui.heading("Achievement Progress");
    ui.separator();
    
    // Get current tab before any borrows
    let current_tab = platform.achievements_graph_tab();
    
    // Tab buttons for switching between graph views
    let mut new_tab = current_tab;
    ui.horizontal(|ui| {
        if ui.selectable_label(current_tab == 0, "Avg Game Completion %").clicked() {
            new_tab = 0;
        }
        if ui.selectable_label(current_tab == 1, "Overall Achievement %").clicked() {
            new_tab = 1;
        }
    });
    
    // Apply tab change if needed
    if new_tab != current_tab {
        platform.set_achievements_graph_tab(new_tab);
    }
    
    let achievement_history = platform.achievement_history();
    
    ui.add_space(4.0);
    
    // Build data for the selected tab
    let (points, y_min, y_max, line_name, line_color) = if achievement_history.is_empty() {
        // Empty plot - still need to show it for WASM layout
        (PlotPoints::default(), 0.0, 100.0, "Avg Game Completion %", Color32::from_rgb(100, 200, 100))
    } else if new_tab == 0 {
        // Avg Game Completion % graph
        let values: Vec<f64> = achievement_history.iter().map(|h| h.avg_completion_percent as f64).collect();
        let pts: PlotPoints = achievement_history.iter().enumerate()
            .map(|(i, h)| [i as f64, h.avg_completion_percent as f64]).collect();
        let (y_min, y_max) = calc_y_bounds(&values);
        (pts, y_min, y_max, "Avg Game Completion %", Color32::from_rgb(100, 200, 100))
    } else {
        // Overall Achievement % graph
        let values: Vec<f64> = achievement_history.iter().map(|h| {
            if h.total_achievements > 0 {
                h.unlocked_achievements as f64 / h.total_achievements as f64 * 100.0
            } else { 0.0 }
        }).collect();
        let pts: PlotPoints = achievement_history.iter().enumerate().map(|(i, h)| {
            let pct = if h.total_achievements > 0 {
                h.unlocked_achievements as f64 / h.total_achievements as f64 * 100.0
            } else { 0.0 };
            [i as f64, pct]
        }).collect();
        let (y_min, y_max) = calc_y_bounds(&values);
        (pts, y_min, y_max, "Overall Achievement %", Color32::from_rgb(100, 150, 255))
    };
    
    let line = Line::new(line_name, points).color(line_color);
    
    // Reset zoom when switching tabs so the new line fits in view
    let tab_changed = new_tab != current_tab;
    
    // Use consistent plot ID - changing IDs can cause WASM layout issues
    let mut plot = Plot::new("achievements_history")
        .auto_bounds(egui::Vec2b::new(true, true))
        .include_y(y_min)
        .include_y(y_max);
    
    if tab_changed {
        plot = plot.reset();
    }
    
    if let Some(height) = config.plot_height {
        plot = plot.height(height).width(ui.available_width());
    } else {
        plot = plot.view_aspect(2.0);
    }
    
    if !config.show_plot_axes {
        plot = plot.show_axes([false, true]);
    }
    
    if !config.allow_plot_interaction {
        plot = plot
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false);
    }
    
    plot.show(ui, |plot_ui| {
        plot_ui.line(line);
    });
    
    if achievement_history.is_empty() {
        ui.label("No achievement data yet. Run a full scan to start tracking!");
    }
}

/// Render the breakdown section with game counts and current stats
pub fn render_breakdown<P: StatsPanelPlatform>(ui: &mut Ui, platform: &mut P) {
    ui.heading(format!("{} Breakdown", regular::GAME_CONTROLLER));
    ui.separator();
    
    // Collect all data we need from games upfront to avoid borrow issues
    let (
        games_len,
        total_with_ach,
        total_achievements,
        unlocked_achievements,
        unplayed_count,
        completion_percents_with_unplayed,
        completion_percents_played_only,
        completed_count,
        needs_scan,
    ) = {
        let games = platform.games();
        
        if games.is_empty() {
            ui.label("Sync your games to see stats.");
            return;
        }
        
        let games_with_ach: Vec<_> = games.iter()
            .filter(|g| g.achievements_total.map(|t| t > 0).unwrap_or(false))
            .collect();
        
        let total_ach: i32 = games_with_ach.iter()
            .filter_map(|g| g.achievements_total)
            .sum();
        let unlocked_ach: i32 = games_with_ach.iter()
            .filter_map(|g| g.achievements_unlocked)
            .sum();
        
        let percents_with_unplayed: Vec<f32> = games_with_ach.iter()
            .filter_map(|g| g.completion_percent())
            .collect();
        let percents_played_only: Vec<f32> = games_with_ach.iter()
            .filter(|g| g.playtime_forever > 0)
            .filter_map(|g| g.completion_percent())
            .collect();
        
        let unplayed = games_with_ach.len() - games_with_ach.iter()
            .filter(|g| g.playtime_forever > 0)
            .count();
        
        let completed = games.iter()
            .filter(|g| g.completion_percent().map(|p| p >= 100.0).unwrap_or(false))
            .count();
        let needs = games.iter().filter(|g| g.achievements_total.is_none()).count();
        
        (
            games.len(),
            games_with_ach.len(),
            total_ach,
            unlocked_ach,
            unplayed,
            percents_with_unplayed,
            percents_played_only,
            completed,
            needs,
        )
    };
    
    let yellow = Color32::from_rgb(255, 215, 0);
    
    // === Current stats (Total achievements, Avg completion, Unplayed) ===
    
    let overall_pct = if total_achievements > 0 {
        unlocked_achievements as f32 / total_achievements as f32 * 100.0
    } else {
        0.0
    };
    
    ui.horizontal(|ui| {
        ui.label("Total achievements:");
        ui.label(RichText::new(format!("{}", unlocked_achievements)).color(yellow).strong());
        ui.label("/");
        ui.label(RichText::new(format!("{}", total_achievements)).color(yellow).strong());
        ui.label("(");
        ui.label(RichText::new(format!("{:.1}%", overall_pct)).color(yellow).strong());
        ui.label(")");
    });
    
    let include_unplayed = platform.include_unplayed_in_avg();
    let completion_percents = if include_unplayed {
        &completion_percents_with_unplayed
    } else {
        &completion_percents_played_only
    };
    
    let current_avg = if completion_percents.is_empty() {
        0.0
    } else {
        completion_percents.iter().sum::<f32>() / completion_percents.len() as f32
    };
    
    // Calculate unplayed percentage
    let unplayed_pct = if total_with_ach > 0 {
        unplayed_count as f32 / total_with_ach as f32 * 100.0
    } else {
        0.0
    };
    
    ui.horizontal(|ui| {
        ui.label("Avg. game completion:");
        ui.label(RichText::new(format!("{:.1}%", current_avg)).color(yellow).strong());
        let mut include = include_unplayed;
        if ui.checkbox(&mut include, "Include unplayed").changed() {
            platform.set_include_unplayed_in_avg(include);
        }
    });
    
    // Show unplayed games count and percentage
    ui.horizontal(|ui| {
        ui.label("Unplayed games:");
        ui.label(RichText::new(format!("{}", unplayed_count)).color(yellow).strong());
        ui.label("(");
        ui.label(RichText::new(format!("{:.1}%", unplayed_pct)).color(yellow).strong());
        ui.label(")");
    });
    
    ui.horizontal(|ui| {
        ui.label("Total games:");
        ui.label(RichText::new(format!("{}", games_len)).color(yellow).strong());
    });
    
    ui.horizontal(|ui| {
        ui.label("Games with achievements:");
        ui.label(RichText::new(format!("{}", total_with_ach)).color(yellow).strong());
    });
    
    ui.horizontal(|ui| {
        ui.label(format!("{} 100% completed:", regular::MEDAL));
        ui.label(RichText::new(format!("{}", completed_count)).color(yellow).strong());
    });
    
    if needs_scan > 0 {
        ui.horizontal(|ui| {
            ui.label("Needs scanning:");
            ui.label(RichText::new(format!("{}", needs_scan)).color(Color32::LIGHT_GRAY));
        });
    }
}
