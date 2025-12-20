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
    // Achievement rating and selection (optional - default implementations)
    // ========================================================================
    
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
    
    /// Submit a comment for selected achievements
    fn submit_achievement_comment(&mut self, _comment: String) {}
    
    /// Get the current comment text being edited
    fn pending_comment(&self) -> &str { "" }
    
    /// Set the pending comment text
    fn set_pending_comment(&mut self, _comment: String) {}
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

/// Render the "Games Over Time" graph
pub fn render_games_over_time<P: StatsPanelPlatform>(
    ui: &mut Ui,
    platform: &P,
    config: &StatsPanelConfig,
) {
    ui.heading("Games Over Time");
    ui.separator();
    
    let run_history = platform.run_history();
    
    // Always create PlotPoints - use default if empty (required for WASM rendering)
    let points: PlotPoints = if run_history.is_empty() {
        PlotPoints::default()
    } else {
        run_history
            .iter()
            .enumerate()
            .map(|(i, h)| [i as f64, h.total_games as f64])
            .collect()
    };
    
    let line = Line::new("Total Games", points)
        .color(Color32::from_rgb(100, 180, 255));
    
    // Build plot - use height/width for WASM, view_aspect for desktop
    let mut plot = Plot::new("games_history")
        .auto_bounds(egui::Vec2b::new(true, true));
    
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
    }
}

/// Render the "Achievement Progress" graph with stats below
pub fn render_achievement_progress<P: StatsPanelPlatform>(
    ui: &mut Ui,
    platform: &mut P,
    config: &StatsPanelConfig,
) {
    ui.heading("Achievement Progress");
    ui.separator();
    
    let achievement_history = platform.achievement_history();
    
    // Always create PlotPoints and bounds - use defaults if empty (required for WASM rendering)
    let (avg_completion_points, overall_pct_points, y_min, y_max) = if achievement_history.is_empty() {
        (PlotPoints::default(), PlotPoints::default(), 0.0, 100.0)
    } else {
        // Line 1: Average game completion %
        let avg_points: PlotPoints = achievement_history
            .iter()
            .enumerate()
            .map(|(i, h)| [i as f64, h.avg_completion_percent as f64])
            .collect();
        
        // Line 2: Overall achievement % (unlocked / total)
        let overall_points: PlotPoints = achievement_history
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let pct = if h.total_achievements > 0 {
                    h.unlocked_achievements as f64 / h.total_achievements as f64 * 100.0
                } else {
                    0.0
                };
                [i as f64, pct]
            })
            .collect();
        
        // Calculate Y-axis bounds based on actual data
        let all_values: Vec<f64> = achievement_history
            .iter()
            .flat_map(|h| {
                let overall_pct = if h.total_achievements > 0 {
                    h.unlocked_achievements as f64 / h.total_achievements as f64 * 100.0
                } else {
                    0.0
                };
                vec![h.avg_completion_percent as f64, overall_pct]
            })
            .collect();
        
        let min_y = all_values.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
        let max_y = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max).min(100.0);
        
        // Add some padding (5% of range, minimum 1.0)
        let range = max_y - min_y;
        let padding = (range * 0.05).max(1.0);
        let y_min = (min_y - padding).max(0.0);
        let y_max = (max_y + padding).min(100.0);
        
        (avg_points, overall_points, y_min, y_max)
    };
    
    let avg_line = Line::new("Avg Game Completion %", avg_completion_points)
        .color(Color32::from_rgb(100, 200, 100));
    let overall_line = Line::new("Overall Achievement %", overall_pct_points)
        .color(Color32::from_rgb(100, 150, 255));
    
    // Build plot - use height/width for WASM, view_aspect for desktop
    let mut plot = Plot::new("achievements_history")
        .legend(egui_plot::Legend::default())
        .auto_bounds(egui::Vec2b::new(true, true))
        .include_y(y_min)
        .include_y(y_max);
    
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
        plot_ui.line(avg_line);
        plot_ui.line(overall_line);
    });
    
    if achievement_history.is_empty() {
        ui.label("No achievement data yet. Run a full scan to start tracking!");
    } else {
        // Show current stats below the graph
        render_current_stats(ui, platform);
    }
}

/// Render the current stats (total achievements, avg completion, etc.)
fn render_current_stats<P: StatsPanelPlatform>(ui: &mut Ui, platform: &mut P) {
    let achievement_history = platform.achievement_history();
    let Some(latest) = achievement_history.last() else {
        return;
    };
    
    ui.add_space(8.0);
    
    let yellow = Color32::from_rgb(255, 215, 0);
    
    let overall_pct = if latest.total_achievements > 0 {
        latest.unlocked_achievements as f32 / latest.total_achievements as f32 * 100.0
    } else {
        0.0
    };
    
    ui.horizontal(|ui| {
        ui.label("Total achievements:");
        ui.label(RichText::new(format!("{}", latest.unlocked_achievements)).color(yellow).strong());
        ui.label("/");
        ui.label(RichText::new(format!("{}", latest.total_achievements)).color(yellow).strong());
        ui.label("(");
        ui.label(RichText::new(format!("{:.1}%", overall_pct)).color(yellow).strong());
        ui.label(")");
    });
    
    // Calculate current avg completion based on toggle
    let games = platform.games();
    let games_with_ach: Vec<_> = games.iter()
        .filter(|g| g.achievements_total.map(|t| t > 0).unwrap_or(false))
        .collect();
    
    // Count played vs unplayed games (with achievements)
    let played_count = games_with_ach.iter()
        .filter(|g| g.playtime_forever > 0)
        .count();
    let unplayed_count = games_with_ach.len() - played_count;
    let total_games_with_ach = games_with_ach.len();
    
    let include_unplayed = platform.include_unplayed_in_avg();
    let completion_percents: Vec<f32> = if include_unplayed {
        games_with_ach.iter()
            .filter_map(|g| g.completion_percent())
            .collect()
    } else {
        games_with_ach.iter()
            .filter(|g| g.playtime_forever > 0)
            .filter_map(|g| g.completion_percent())
            .collect()
    };
    
    let current_avg = if completion_percents.is_empty() {
        0.0
    } else {
        completion_percents.iter().sum::<f32>() / completion_percents.len() as f32
    };
    
    // Calculate unplayed percentage before the closure
    let unplayed_pct = if total_games_with_ach > 0 {
        unplayed_count as f32 / total_games_with_ach as f32 * 100.0
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
