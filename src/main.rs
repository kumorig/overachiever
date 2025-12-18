mod db;
mod icon_cache;
mod models;
mod steam_api;

use db::{get_all_games, get_run_history, get_achievement_history, insert_achievement_history, open_connection, get_last_update, get_game_achievements};
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular;
use egui_plot::{Line, Plot, PlotPoints};
use icon_cache::IconCache;
use models::{Game, RunHistory, AchievementHistory, GameAchievement};
use steam_api::{FetchProgress, ScrapeProgress, UpdateProgress};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Instant;

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

#[derive(Clone, PartialEq)]
enum AppState {
    Idle,
    // Fetch states
    FetchRequesting,
    FetchDownloading,
    FetchProcessing,
    FetchSaving,
    // Scrape states
    Scraping { current: i32, total: i32 },
    // Update states
    UpdateFetchingGames,
    UpdateFetchingRecentlyPlayed,
    UpdateScraping { current: i32, total: i32 },
}

impl AppState {
    fn is_busy(&self) -> bool {
        !matches!(self, AppState::Idle)
    }
    
    fn progress(&self) -> f32 {
        match self {
            AppState::Idle => 0.0,
            AppState::FetchRequesting => 0.25,
            AppState::FetchDownloading => 0.50,
            AppState::FetchProcessing => 0.75,
            AppState::FetchSaving => 0.90,
            AppState::Scraping { current, total } => {
                if *total > 0 { *current as f32 / *total as f32 } else { 0.0 }
            }
            AppState::UpdateFetchingGames => 0.10,
            AppState::UpdateFetchingRecentlyPlayed => 0.20,
            AppState::UpdateScraping { current, total } => {
                if *total > 0 { 0.20 + 0.80 * (*current as f32 / *total as f32) } else { 0.20 }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SortColumn {
    Name,
    LastPlayed,
    Playtime,
    AchievementsTotal,
    AchievementsPercent,
}

#[derive(Clone, Copy, PartialEq)]
enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    fn toggle(&self) -> Self {
        match self {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        }
    }
}

/// Tri-state filter: All, Only With, Only Without
#[derive(Clone, Copy, PartialEq, Default)]
enum TriFilter {
    #[default]
    All,
    With,
    Without,
}

impl TriFilter {
    fn cycle(&self) -> Self {
        match self {
            TriFilter::All => TriFilter::With,
            TriFilter::With => TriFilter::Without,
            TriFilter::Without => TriFilter::All,
        }
    }
    
    fn label(&self, with_text: &str, without_text: &str) -> String {
        match self {
            TriFilter::All => "All".to_string(),
            TriFilter::With => with_text.to_string(),
            TriFilter::Without => without_text.to_string(),
        }
    }
}

#[allow(dead_code)]
enum ProgressReceiver {
    Fetch(Receiver<FetchProgress>),
    Scrape(Receiver<ScrapeProgress>),
    Update(Receiver<UpdateProgress>),
}

// Duration for the flash animation in seconds
const FLASH_DURATION: f32 = 2.0;

struct SteamOverachieverApp {
    games: Vec<Game>,
    run_history: Vec<RunHistory>,
    achievement_history: Vec<AchievementHistory>,
    status: String,
    state: AppState,
    receiver: Option<ProgressReceiver>,
    sort_column: SortColumn,
    sort_order: SortOrder,
    // Track recently updated games: appid -> time of update
    updated_games: HashMap<u64, Instant>,
    // Track last update time for 2-week warning
    last_update_time: Option<chrono::DateTime<chrono::Utc>>,
    // Force full scan even when all games have been scraped
    force_full_scan: bool,
    // Include unplayed games (0%) in avg completion calculation
    include_unplayed_in_avg: bool,
    // Track which rows are expanded to show achievements
    expanded_rows: HashSet<u64>,
    // Cache loaded achievements for expanded games
    achievements_cache: HashMap<u64, Vec<GameAchievement>>,
    // Icon cache for achievement icons
    icon_cache: IconCache,
    // Filters
    filter_name: String,
    filter_achievements: TriFilter,
    filter_playtime: TriFilter,
}

impl SteamOverachieverApp {
    fn new() -> Self {
        let conn = open_connection().expect("Failed to open database");
        let games = get_all_games(&conn).unwrap_or_default();
        let run_history = get_run_history(&conn).unwrap_or_default();
        let achievement_history = get_achievement_history(&conn).unwrap_or_default();
        let last_update_time = get_last_update(&conn).unwrap_or(None);
        
        let mut app = Self {
            games,
            run_history,
            achievement_history,
            status: "Ready".to_string(),
            state: AppState::Idle,
            receiver: None,
            sort_column: SortColumn::Name,
            sort_order: SortOrder::Ascending,
            updated_games: HashMap::new(),
            last_update_time,
            force_full_scan: false,
            include_unplayed_in_avg: false,
            expanded_rows: HashSet::new(),
            achievements_cache: HashMap::new(),
            icon_cache: IconCache::new(),
            filter_name: String::new(),
            filter_achievements: TriFilter::All,
            filter_playtime: TriFilter::All,
        };
        
        // Apply consistent sorting after loading from database
        app.sort_games();
        app
    }
    
    fn sort_games(&mut self) {
        let order = self.sort_order;
        match self.sort_column {
            SortColumn::Name => {
                self.games.sort_by(|a, b| {
                    let cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::LastPlayed => {
                self.games.sort_by(|a, b| {
                    let cmp = a.rtime_last_played.unwrap_or(0).cmp(&b.rtime_last_played.unwrap_or(0));
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::Playtime => {
                self.games.sort_by(|a, b| {
                    let cmp = a.playtime_forever.cmp(&b.playtime_forever);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::AchievementsTotal => {
                self.games.sort_by(|a, b| {
                    let a_total = a.achievements_total.unwrap_or(-1);
                    let b_total = b.achievements_total.unwrap_or(-1);
                    let cmp = a_total.cmp(&b_total);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::AchievementsPercent => {
                self.games.sort_by(|a, b| {
                    let a_pct = a.completion_percent().unwrap_or(-1.0);
                    let b_pct = b.completion_percent().unwrap_or(-1.0);
                    let cmp = a_pct.partial_cmp(&b_pct).unwrap_or(std::cmp::Ordering::Equal);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
        }
    }
    
    fn set_sort(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.sort_order = self.sort_order.toggle();
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Ascending;
        }
        self.sort_games();
    }
    
    fn sort_indicator(&self, column: SortColumn) -> String {
        if self.sort_column == column {
            match self.sort_order {
                SortOrder::Ascending => format!(" {}", regular::SORT_ASCENDING),
                SortOrder::Descending => format!(" {}", regular::SORT_DESCENDING),
            }
        } else {
            String::new()
        }
    }
    
    #[allow(dead_code)]
    fn start_fetch(&mut self) {
        if self.state.is_busy() {
            return;
        }
        
        self.state = AppState::FetchRequesting;
        self.status = "Starting fetch...".to_string();
        
        let (tx, rx): (Sender<FetchProgress>, Receiver<FetchProgress>) = channel();
        self.receiver = Some(ProgressReceiver::Fetch(rx));
        
        thread::spawn(move || {
            if let Err(e) = steam_api::fetch_owned_games_with_progress(tx.clone()) {
                let _ = tx.send(FetchProgress::Error(e.to_string()));
            }
        });
    }
    
    fn start_scrape(&mut self) {
        if self.state.is_busy() {
            return;
        }
        
        self.state = AppState::Scraping { current: 0, total: 0 };
        self.status = "Starting achievement scrape...".to_string();
        
        let force = self.force_full_scan;
        let (tx, rx): (Sender<ScrapeProgress>, Receiver<ScrapeProgress>) = channel();
        self.receiver = Some(ProgressReceiver::Scrape(rx));
        
        thread::spawn(move || {
            if let Err(e) = steam_api::scrape_achievements_with_progress(tx.clone(), force) {
                let _ = tx.send(ScrapeProgress::Error(e.to_string()));
            }
        });
    }
    
    fn start_update(&mut self) {
        if self.state.is_busy() {
            return;
        }
        
        self.state = AppState::UpdateFetchingGames;
        self.status = "Starting update...".to_string();
        
        let (tx, rx): (Sender<UpdateProgress>, Receiver<UpdateProgress>) = channel();
        self.receiver = Some(ProgressReceiver::Update(rx));
        
        thread::spawn(move || {
            if let Err(e) = steam_api::run_update_with_progress(tx.clone()) {
                let _ = tx.send(UpdateProgress::Error(e.to_string()));
            }
        });
    }
    
    /// Check if the last update was more than 2 weeks ago
    fn is_update_stale(&self) -> bool {
        match self.last_update_time {
            Some(last_update) => {
                let two_weeks_ago = chrono::Utc::now() - chrono::Duration::weeks(2);
                last_update < two_weeks_ago
            }
            None => true, // Never updated, consider it stale
        }
    }
    
    fn check_progress(&mut self) {
        let receiver = match self.receiver.take() {
            Some(r) => r,
            None => return,
        };
        
        match receiver {
            ProgressReceiver::Fetch(rx) => {
                while let Ok(progress) = rx.try_recv() {
                    match progress {
                        FetchProgress::Requesting => {
                            self.state = AppState::FetchRequesting;
                            self.status = "Requesting...".to_string();
                        }
                        FetchProgress::Downloading => {
                            self.state = AppState::FetchDownloading;
                            self.status = "Downloading...".to_string();
                        }
                        FetchProgress::Processing => {
                            self.state = AppState::FetchProcessing;
                            self.status = "Processing...".to_string();
                        }
                        FetchProgress::Saving => {
                            self.state = AppState::FetchSaving;
                            self.status = "Saving to database...".to_string();
                        }
                        FetchProgress::Done { games, total } => {
                            self.games = games;
                            self.sort_games();
                            if let Ok(conn) = open_connection() {
                                self.run_history = get_run_history(&conn).unwrap_or_default();
                            }
                            self.status = format!("Fetched {} games!", total);
                            self.state = AppState::Idle;
                            return;
                        }
                        FetchProgress::Error(e) => {
                            self.status = format!("Error: {}", e);
                            self.state = AppState::Idle;
                            return;
                        }
                    }
                }
                self.receiver = Some(ProgressReceiver::Fetch(rx));
            }
            ProgressReceiver::Scrape(rx) => {
                while let Ok(progress) = rx.try_recv() {
                    match progress {
                        ScrapeProgress::FetchingGames => {
                            self.state = AppState::FetchRequesting;
                            self.status = "Fetching games...".to_string();
                        }
                        ScrapeProgress::Starting { total } => {
                            self.state = AppState::Scraping { current: 0, total };
                            self.status = format!("Scraping 0 / {} games...", total);
                        }
                        ScrapeProgress::Scraping { current, total, game_name } => {
                            self.state = AppState::Scraping { current, total };
                            self.status = format!("Scraping {} / {}: {}", current, total, game_name);
                        }
                        ScrapeProgress::GameUpdated { appid, unlocked, total } => {
                            // Update the game in our list immediately
                            if let Some(game) = self.games.iter_mut().find(|g| g.appid == appid) {
                                game.achievements_unlocked = Some(unlocked);
                                game.achievements_total = Some(total);
                                game.last_achievement_scrape = Some(chrono::Utc::now());
                            }
                            // Track this game for flash animation
                            self.updated_games.insert(appid, Instant::now());
                            // Re-sort to place updated row in correct position
                            self.sort_games();
                        }
                        ScrapeProgress::Done { games } => {
                            self.games = games;
                            self.sort_games();
                            
                            // Reload run history since we fetched games as well
                            if let Ok(conn) = open_connection() {
                                self.run_history = get_run_history(&conn).unwrap_or_default();
                            }
                            
                            // Calculate and save achievement stats
                            self.save_achievement_history();
                            
                            self.status = "Full scan complete!".to_string();
                            self.state = AppState::Idle;
                            return;
                        }
                        ScrapeProgress::Error(e) => {
                            self.status = format!("Error: {}", e);
                            self.state = AppState::Idle;
                            return;
                        }
                    }
                }
                self.receiver = Some(ProgressReceiver::Scrape(rx));
            }
            ProgressReceiver::Update(rx) => {
                while let Ok(progress) = rx.try_recv() {
                    match progress {
                        UpdateProgress::FetchingGames => {
                            self.state = AppState::UpdateFetchingGames;
                            self.status = "Fetching games...".to_string();
                        }
                        UpdateProgress::FetchingRecentlyPlayed => {
                            self.state = AppState::UpdateFetchingRecentlyPlayed;
                            self.status = "Fetching recently played games...".to_string();
                        }
                        UpdateProgress::ScrapingAchievements { current, total, game_name } => {
                            self.state = AppState::UpdateScraping { current, total };
                            self.status = format!("Updating {} / {}: {}", current, total, game_name);
                        }
                        UpdateProgress::GameUpdated { appid, unlocked, total } => {
                            // Update the game in our list immediately
                            if let Some(game) = self.games.iter_mut().find(|g| g.appid == appid) {
                                game.achievements_unlocked = Some(unlocked);
                                game.achievements_total = Some(total);
                                game.last_achievement_scrape = Some(chrono::Utc::now());
                            }
                            // Track this game for flash animation
                            self.updated_games.insert(appid, Instant::now());
                            // Re-sort to place updated row in correct position
                            self.sort_games();
                        }
                        UpdateProgress::Done { games, updated_count } => {
                            self.games = games;
                            self.sort_games();
                            
                            // Reload run history
                            if let Ok(conn) = open_connection() {
                                self.run_history = get_run_history(&conn).unwrap_or_default();
                                self.last_update_time = get_last_update(&conn).unwrap_or(None);
                            }
                            
                            // Calculate and save achievement stats
                            self.save_achievement_history();
                            
                            self.status = format!("Update complete! {} games updated.", updated_count);
                            self.state = AppState::Idle;
                            return;
                        }
                        UpdateProgress::Error(e) => {
                            self.status = format!("Error: {}", e);
                            self.state = AppState::Idle;
                            return;
                        }
                    }
                }
                self.receiver = Some(ProgressReceiver::Update(rx));
            }
        }
    }
    
    fn games_needing_scrape(&self) -> usize {
        self.games.iter().filter(|g| g.last_achievement_scrape.is_none()).count()
    }
    
    /// Returns the flash intensity (0.0 to 1.0) for a game, or None if not flashing
    fn get_flash_intensity(&self, appid: u64) -> Option<f32> {
        if let Some(update_time) = self.updated_games.get(&appid) {
            let elapsed = update_time.elapsed().as_secs_f32();
            if elapsed < FLASH_DURATION {
                // Fade from 1.0 to 0.0 over FLASH_DURATION seconds
                Some(1.0 - (elapsed / FLASH_DURATION))
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Clean up expired flash entries
    fn cleanup_expired_flashes(&mut self) {
        self.updated_games.retain(|_, update_time| {
            update_time.elapsed().as_secs_f32() < FLASH_DURATION
        });
    }
    
    /// Calculate and save achievement statistics to history
    fn save_achievement_history(&mut self) {
        // Calculate stats from games with achievements
        let games_with_ach: Vec<_> = self.games.iter()
            .filter(|g| g.achievements_total.map(|t| t > 0).unwrap_or(false))
            .collect();
        
        if games_with_ach.is_empty() {
            return;
        }
        
        let total_achievements: i32 = games_with_ach.iter()
            .filter_map(|g| g.achievements_total)
            .sum();
        
        let unlocked_achievements: i32 = games_with_ach.iter()
            .filter_map(|g| g.achievements_unlocked)
            .sum();
        
        // Only count played games (playtime > 0) for avg completion
        let completion_percents: Vec<f32> = games_with_ach.iter()
            .filter(|g| g.playtime_forever > 0)
            .filter_map(|g| g.completion_percent())
            .collect();
        
        let avg_completion: f32 = if completion_percents.is_empty() {
            0.0
        } else {
            completion_percents.iter().sum::<f32>() / completion_percents.len() as f32
        };
        
        if let Ok(conn) = open_connection() {
            let _ = insert_achievement_history(
                &conn,
                total_achievements,
                unlocked_achievements,
                games_with_ach.len() as i32,
                avg_completion,
            );
            self.achievement_history = get_achievement_history(&conn).unwrap_or_default();
        }
    }
}

impl eframe::App for SteamOverachieverApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_progress();
        
        // Clean up expired flash animations
        self.cleanup_expired_flashes();
        
        let is_busy = self.state.is_busy();
        let has_flashing = !self.updated_games.is_empty();
        
        // Request repaint while busy or while animations are active
        if is_busy || has_flashing {
            ctx.request_repaint();
        }
        
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Steam Overachiever v3");
                ui.separator();
                
                // Update button - for recently played games
                let update_button = egui::Button::new(format!("{} Update", regular::ARROWS_CLOCKWISE));
                let update_response = ui.add_enabled(!is_busy, update_button);
                
                // Show warning if update is stale
                if self.is_update_stale() && !is_busy {
                    update_response.clone().on_hover_text(
                        "⚠️ Last update was more than 2 weeks ago.\nThe recently played API only shows games from the last 2 weeks.\nConsider running a Full Scan instead."
                    );
                }
                
                if update_response.clicked() {
                    self.start_update();
                }
                
                // Full Scan button - scrapes achievements for all games not yet scraped
                let needs_scrape = self.games_needing_scrape();
                let full_scan_label = if needs_scrape > 0 {
                    format!("{} Full Scan ({})", regular::GAME_CONTROLLER, needs_scrape)
                } else {
                    format!("{} Full Scan", regular::GAME_CONTROLLER)
                };
                let can_scan = needs_scrape > 0 || self.force_full_scan;
                if ui.add_enabled(!is_busy && can_scan, egui::Button::new(full_scan_label)).clicked() {
                    self.start_scrape();
                }
                
                ui.checkbox(&mut self.force_full_scan, "Force");
                
                ui.separator();
                
                if is_busy {
                    ui.spinner();
                    ui.add(egui::ProgressBar::new(self.state.progress())
                        .text(&self.status)
                        .animate(true));
                } else {
                    ui.label(&self.status);
                }
            });
        });
        
        egui::SidePanel::right("history_panel")
            .min_width(350.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Games Over Time Graph
                    ui.heading("Games Over Time");
                    ui.separator();
                    
                    if self.run_history.is_empty() {
                        ui.label("No history yet. Click 'Update' to start tracking!");
                    } else {
                        let points: PlotPoints = self.run_history
                            .iter()
                            .enumerate()
                            .map(|(i, h)| [i as f64, h.total_games as f64])
                            .collect();
                        
                        let line = Line::new("Total Games", points);
                        
                        Plot::new("games_history")
                            .view_aspect(2.0)
                            .show(ui, |plot_ui| {
                                plot_ui.line(line);
                            });
                    }
                    
                    ui.add_space(16.0);
                    
                    // Achievements Progress Graph
                    ui.heading("Achievement Progress");
                    ui.separator();
                    
                    if self.achievement_history.is_empty() {
                        ui.label("No achievement data yet. Click 'Full Scan' to start tracking!");
                    } else {
                        // Line 1: Average game completion %
                        let avg_completion_points: PlotPoints = self.achievement_history
                            .iter()
                            .enumerate()
                            .map(|(i, h)| [i as f64, h.avg_completion_percent as f64])
                            .collect();
                        
                        // Line 2: Overall achievement % (unlocked / total)
                        let overall_pct_points: PlotPoints = self.achievement_history
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
                        
                        let avg_line = Line::new("Avg Game Completion %", avg_completion_points)
                            .color(egui::Color32::from_rgb(100, 200, 100));
                        let overall_line = Line::new("Overall Achievement %", overall_pct_points)
                            .color(egui::Color32::from_rgb(100, 150, 255));
                        
                        Plot::new("achievements_history")
                            .view_aspect(2.0)
                            .legend(egui_plot::Legend::default())
                            .include_y(0.0)
                            .include_y(100.0)
                            .show(ui, |plot_ui| {
                                plot_ui.line(avg_line);
                                plot_ui.line(overall_line);
                            });
                        
                        // Toggle for including unplayed games
                        ui.checkbox(&mut self.include_unplayed_in_avg, "Include unplayed games in avg");
                        
                        // Show current stats
                        if let Some(latest) = self.achievement_history.last() {
                            ui.add_space(8.0);
                            
                            // Yellow color for prominent numbers
                            let yellow = egui::Color32::from_rgb(255, 215, 0);
                            
                            let overall_pct = if latest.total_achievements > 0 {
                                latest.unlocked_achievements as f32 / latest.total_achievements as f32 * 100.0
                            } else {
                                0.0
                            };
                            
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("{}", latest.unlocked_achievements)).color(yellow).strong());
                                ui.label("/");
                                ui.label(egui::RichText::new(format!("{}", latest.total_achievements)).color(yellow).strong());
                                ui.label("achievements (");
                                ui.label(egui::RichText::new(format!("{:.1}%", overall_pct)).color(yellow).strong());
                                ui.label(")");
                            });
                            
                            // Calculate current avg completion based on toggle
                            let games_with_ach: Vec<_> = self.games.iter()
                                .filter(|g| g.achievements_total.map(|t| t > 0).unwrap_or(false))
                                .collect();
                            
                            // Count played vs unplayed games (with achievements)
                            let played_count = games_with_ach.iter()
                                .filter(|g| g.playtime_forever > 0)
                                .count();
                            let unplayed_count = games_with_ach.len() - played_count;
                            
                            let completion_percents: Vec<f32> = if self.include_unplayed_in_avg {
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
                            
                            ui.horizontal(|ui| {
                                ui.label("Average completion:");
                                ui.label(egui::RichText::new(format!("{:.1}%", current_avg)).color(yellow).strong());
                            });
                            
                            // Show unplayed games count and percentage
                            let total_games_with_ach = games_with_ach.len();
                            let unplayed_pct = if total_games_with_ach > 0 {
                                unplayed_count as f32 / total_games_with_ach as f32 * 100.0
                            } else {
                                0.0
                            };
                            ui.horizontal(|ui| {
                                ui.label("Unplayed games:");
                                ui.label(egui::RichText::new(format!("{}", unplayed_count)).color(yellow).strong());
                                ui.label("(");
                                ui.label(egui::RichText::new(format!("{:.1}%", unplayed_pct)).color(yellow).strong());
                                ui.label(")");
                            });
                        }
                    }
                    
                    ui.add_space(16.0);
                    
                    // Collapsible Run History
                    ui.collapsing(format!("{} Run History", regular::CLOCK_COUNTER_CLOCKWISE), |ui| {
                        if self.run_history.is_empty() {
                            ui.label("No runs recorded yet.");
                        } else {
                            for entry in self.run_history.iter().rev() {
                                ui.horizontal(|ui| {
                                    ui.label(entry.run_at.format("%Y-%m-%d %H:%M").to_string());
                                    ui.label(format!("{} games", entry.total_games));
                                });
                            }
                        }
                    });
                });
            });
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(format!("Games Library ({} games)", self.games.len()));
            ui.separator();
            
            if self.games.is_empty() {
                ui.label("No games loaded. Click 'Update' to load your Steam library.");
            } else {
                // Filter bar
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    ui.add(egui::TextEdit::singleline(&mut self.filter_name)
                        .hint_text("Search by name...")
                        .desired_width(150.0));
                    
                    ui.add_space(10.0);
                    
                    // Achievements filter - tri-state toggle button
                    let ach_label = format!("Achievements: {}", self.filter_achievements.label("With", "Without"));
                    if ui.button(&ach_label).clicked() {
                        self.filter_achievements = self.filter_achievements.cycle();
                    }
                    
                    // Playtime filter - tri-state toggle button
                    let play_label = format!("Played: {}", self.filter_playtime.label("Yes", "No"));
                    if ui.button(&play_label).clicked() {
                        self.filter_playtime = self.filter_playtime.cycle();
                    }
                    
                    // Clear filters button
                    if self.filter_name.is_empty() 
                        && self.filter_achievements == TriFilter::All 
                        && self.filter_playtime == TriFilter::All {
                        // Filters already cleared, show disabled button
                        ui.add_enabled(false, egui::Button::new("Clear"));
                    } else if ui.button("Clear").clicked() {
                        self.filter_name.clear();
                        self.filter_achievements = TriFilter::All;
                        self.filter_playtime = TriFilter::All;
                    }
                });
                
                ui.add_space(4.0);
                
                // Apply filters to get visible games
                let filter_name_lower = self.filter_name.to_lowercase();
                
                // Store indices into self.games for filtered results
                let filtered_indices: Vec<usize> = self.games.iter()
                    .enumerate()
                    .filter(|(_, g)| {
                        // Name filter
                        if !filter_name_lower.is_empty() && !g.name.to_lowercase().contains(&filter_name_lower) {
                            return false;
                        }
                        // Achievements filter
                        let has_achievements = g.achievements_total.map(|t| t > 0).unwrap_or(false);
                        match self.filter_achievements {
                            TriFilter::All => {}
                            TriFilter::With => if !has_achievements { return false; }
                            TriFilter::Without => if has_achievements { return false; }
                        }
                        // Playtime filter
                        let has_playtime = g.rtime_last_played.map(|ts| ts > 0).unwrap_or(false);
                        match self.filter_playtime {
                            TriFilter::All => {}
                            TriFilter::With => if !has_playtime { return false; }
                            TriFilter::Without => if has_playtime { return false; }
                        }
                        true
                    })
                    .map(|(idx, _)| idx)
                    .collect();
                
                let filtered_count = filtered_indices.len();
                if filtered_count != self.games.len() {
                    ui.label(format!("Showing {} of {} games", filtered_count, self.games.len()));
                }
                
                let text_height = egui::TextStyle::Body
                    .resolve(ui.style())
                    .size
                    .max(ui.spacing().interact_size.y);
                
                let available_height = ui.available_height();
                
                // Calculate row heights for each filtered game (including expanded achievements)
                let expanded_rows = self.expanded_rows.clone();
                let row_heights: Vec<f32> = filtered_indices.iter().map(|&idx| {
                    let game = &self.games[idx];
                    if expanded_rows.contains(&game.appid) {
                        // Base row height + scrollable achievement area (max 300px + some padding)
                        text_height + 330.0
                    } else {
                        text_height
                    }
                }).collect();
                
                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::remainder().at_least(100.0).clip(true)) // Name - takes remaining space
                    .column(Column::initial(90.0).at_least(70.0)) // Last Played
                    .column(Column::initial(80.0).at_least(60.0)) // Playtime
                    .column(Column::initial(100.0).at_least(80.0)) // Achievements
                    .column(Column::initial(60.0).at_least(40.0)) // Percent
                    .min_scrolled_height(0.0)
                    .max_scroll_height(available_height)
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            if ui.selectable_label(
                                self.sort_column == SortColumn::Name,
                                format!("Name{}", self.sort_indicator(SortColumn::Name))
                            ).clicked() {
                                self.set_sort(SortColumn::Name);
                            }
                        });
                        header.col(|ui| {
                            if ui.selectable_label(
                                self.sort_column == SortColumn::LastPlayed,
                                format!("Last Played{}", self.sort_indicator(SortColumn::LastPlayed))
                            ).clicked() {
                                self.set_sort(SortColumn::LastPlayed);
                            }
                        });
                        header.col(|ui| {
                            if ui.selectable_label(
                                self.sort_column == SortColumn::Playtime,
                                format!("Playtime{}", self.sort_indicator(SortColumn::Playtime))
                            ).clicked() {
                                self.set_sort(SortColumn::Playtime);
                            }
                        });
                        header.col(|ui| {
                            if ui.selectable_label(
                                self.sort_column == SortColumn::AchievementsTotal,
                                format!("Achievements{}", self.sort_indicator(SortColumn::AchievementsTotal))
                            ).clicked() {
                                self.set_sort(SortColumn::AchievementsTotal);
                            }
                        });
                        header.col(|ui| {
                            if ui.selectable_label(
                                self.sort_column == SortColumn::AchievementsPercent,
                                format!("%{}", self.sort_indicator(SortColumn::AchievementsPercent))
                            ).clicked() {
                                self.set_sort(SortColumn::AchievementsPercent);
                            }
                        });
                    })
                    .body(|body| {
                        body.heterogeneous_rows(row_heights.into_iter(), |mut row| {
                            let row_idx = row.index();
                            let game_idx = filtered_indices[row_idx];
                            let game = &self.games[game_idx];
                            let appid = game.appid;
                            let is_expanded = self.expanded_rows.contains(&appid);
                            
                            // Check if this game should be flashing
                            let flash_color = self.get_flash_intensity(appid).map(|intensity| {
                                // Gold/yellow color that fades out
                                egui::Color32::from_rgba_unmultiplied(
                                    255,  // R
                                    215,  // G (gold)
                                    0,    // B
                                    (intensity * 100.0) as u8  // Alpha fades from 100 to 0
                                )
                            });
                            
                            // Name column with expand/collapse toggle
                            row.col(|ui| {
                                if let Some(color) = flash_color {
                                    ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                                }
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        let has_achievements = game.achievements_total.map(|t| t > 0).unwrap_or(false);
                                        if has_achievements {
                                            let icon = if is_expanded { regular::CARET_DOWN } else { regular::CARET_RIGHT };
                                            if ui.small_button(icon.to_string()).clicked() {
                                                if is_expanded {
                                                    self.expanded_rows.remove(&appid);
                                                } else {
                                                    self.expanded_rows.insert(appid);
                                                    // Load achievements if not cached
                                                    if !self.achievements_cache.contains_key(&appid) {
                                                        if let Ok(conn) = open_connection() {
                                                            if let Ok(achs) = get_game_achievements(&conn, appid) {
                                                                self.achievements_cache.insert(appid, achs);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            ui.add_space(20.0); // Spacer for alignment
                                        }
                                        
                                        // Show game icon when expanded
                                        if is_expanded {
                                            if let Some(icon_hash) = &game.img_icon_url {
                                                if !icon_hash.is_empty() {
                                                    let game_icon_url = format!(
                                                        "https://media.steampowered.com/steamcommunity/public/images/apps/{}/{}.jpg",
                                                        appid, icon_hash
                                                    );
                                                    // Try cache first
                                                    let img_source: egui::ImageSource<'_> = if let Some(bytes) = self.icon_cache.get_icon_bytes(&game_icon_url) {
                                                        let cache_uri = format!("bytes://game/{}", appid);
                                                        ui.ctx().include_bytes(cache_uri.clone(), bytes);
                                                        egui::ImageSource::Uri(cache_uri.into())
                                                    } else {
                                                        egui::ImageSource::Uri(game_icon_url.into())
                                                    };
                                                    ui.add(
                                                        egui::Image::new(img_source)
                                                            .fit_to_exact_size(egui::vec2(32.0, 32.0))
                                                            .corner_radius(4.0)
                                                    );
                                                }
                                            }
                                            ui.label(egui::RichText::new(&game.name).strong());
                                        } else {
                                            ui.label(&game.name);
                                        }
                                    });
                                    
                                    // Show achievements table if expanded
                                    if is_expanded {
                                        if let Some(achievements) = self.achievements_cache.get(&appid) {
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
                                                    
                                                    // Try to load from cache, fall back to URL
                                                    let image_source: egui::ImageSource<'_> = if let Some(bytes) = self.icon_cache.get_icon_bytes(icon_url) {
                                                        // Create a unique URI for cached bytes
                                                        let cache_uri = format!("bytes://ach/{}", icon_url.replace(['/', ':', '.'], "_"));
                                                        ui.ctx().include_bytes(cache_uri.clone(), bytes);
                                                        egui::ImageSource::Uri(cache_uri.into())
                                                    } else {
                                                        // Not cached yet, use HTTP URL
                                                        egui::ImageSource::Uri(icon_url.to_string().into())
                                                    };
                                                    
                                                    // Alternate row background
                                                    let row_rect = ui.available_rect_before_wrap();
                                                    let row_rect = egui::Rect::from_min_size(
                                                        row_rect.min,
                                                        egui::vec2(row_rect.width(), 36.0)
                                                    );
                                                    if i % 2 == 1 {
                                                        ui.painter().rect_filled(
                                                            row_rect,
                                                            0.0,
                                                            ui.visuals().faint_bg_color
                                                        );
                                                    }
                                                    
                                                    ui.horizontal(|ui| {
                                                        // Icon (32x32)
                                                        ui.add(
                                                            egui::Image::new(image_source)
                                                                .fit_to_exact_size(egui::vec2(32.0, 32.0))
                                                                .corner_radius(4.0)
                                                        );
                                                        
                                                        // Name - left aligned, takes remaining space
                                                        let name_text = if ach.achieved {
                                                            egui::RichText::new(&ach.name).color(egui::Color32::WHITE)
                                                        } else {
                                                            egui::RichText::new(&ach.name).color(egui::Color32::DARK_GRAY)
                                                        };
                                                        
                                                        let available = ui.available_width() - 100.0; // Reserve space for date
                                                        ui.allocate_ui_with_layout(
                                                            egui::vec2(available, 32.0),
                                                            egui::Layout::left_to_right(egui::Align::Center),
                                                            |ui| {
                                                                ui.label(name_text).on_hover_text(
                                                                    ach.description.as_deref().unwrap_or("No description")
                                                                );
                                                            }
                                                        );
                                                        
                                                        // Completed date (right-aligned, normal size)
                                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                            if let Some(unlock_dt) = &ach.unlocktime {
                                                                ui.label(
                                                                    egui::RichText::new(unlock_dt.format("%Y-%m-%d").to_string())
                                                                        .color(egui::Color32::from_rgb(100, 200, 100))
                                                                );
                                                            } else {
                                                                ui.label(
                                                                    egui::RichText::new("—")
                                                                        .color(egui::Color32::GRAY)
                                                                );
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
                                });
                            });
                            row.col(|ui| {
                                if let Some(color) = flash_color {
                                    ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, color);
                                }
                                // Only show content in collapsed state
                                if !is_expanded {
                                    if let Some(ts) = game.rtime_last_played {
                                        if ts > 0 {
                                            let dt = chrono::DateTime::from_timestamp(ts as i64, 0)
                                                .map(|d| d.format("%Y-%m-%d").to_string())
                                                .unwrap_or_else(|| "—".to_string());
                                            ui.label(dt);
                                        } else {
                                            ui.label("Never");
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
                                    // Show "--" if never played, otherwise show playtime
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
                                        ui.label(format!("{:.0}%", pct));
                                    } else {
                                        ui.label("—");
                                    }
                                }
                            });
                        });
                    });
            }
        });
    }
}