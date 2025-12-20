//! Games table panel - Central panel with filterable, sortable games list
//! Uses shared implementation from overachiever-core

use eframe::egui;

use crate::app::SteamOverachieverApp;
use crate::db::{open_connection, get_game_achievements};
use crate::ui::{SortColumn, SortOrder, TriFilter};
use overachiever_core::{GamesTablePlatform, GameAchievement, sort_games, get_filtered_indices, render_filter_bar, render_games_table};

/// Implement GamesTablePlatform for the desktop app
impl GamesTablePlatform for SteamOverachieverApp {
    fn sort_column(&self) -> SortColumn {
        self.sort_column
    }
    
    fn sort_order(&self) -> SortOrder {
        self.sort_order
    }
    
    fn set_sort(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.sort_order = self.sort_order.toggle();
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Ascending;
        }
        sort_games(&mut self.games, self.sort_column, self.sort_order);
    }
    
    fn filter_name(&self) -> &str {
        &self.filter_name
    }
    
    fn set_filter_name(&mut self, name: String) {
        self.filter_name = name;
    }
    
    fn filter_achievements(&self) -> TriFilter {
        self.filter_achievements
    }
    
    fn set_filter_achievements(&mut self, filter: TriFilter) {
        self.filter_achievements = filter;
    }
    
    fn filter_playtime(&self) -> TriFilter {
        self.filter_playtime
    }
    
    fn set_filter_playtime(&mut self, filter: TriFilter) {
        self.filter_playtime = filter;
    }
    
    fn is_expanded(&self, appid: u64) -> bool {
        self.expanded_rows.contains(&appid)
    }
    
    fn toggle_expanded(&mut self, appid: u64) {
        if self.expanded_rows.contains(&appid) {
            self.expanded_rows.remove(&appid);
        } else {
            self.expanded_rows.insert(appid);
        }
    }
    
    fn get_cached_achievements(&self, appid: u64) -> Option<&Vec<GameAchievement>> {
        self.achievements_cache.get(&appid)
    }
    
    fn request_achievements(&mut self, appid: u64) {
        // Desktop loads achievements synchronously from local SQLite
        if !self.achievements_cache.contains_key(&appid) {
            if let Ok(conn) = open_connection() {
                if let Ok(achs) = get_game_achievements(&conn, &self.config.steam_id, appid) {
                    self.achievements_cache.insert(appid, achs);
                }
            }
        }
    }
    
    fn get_flash_intensity(&self, appid: u64) -> Option<f32> {
        // Use the existing flash mechanism from desktop app
        SteamOverachieverApp::get_flash_intensity(self, appid)
    }
}

impl SteamOverachieverApp {
    pub(crate) fn render_games_table_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(format!("Games Library ({} games)", self.games.len()));
            ui.separator();
            
            if self.games.is_empty() {
                ui.label("No games loaded. Click 'Update' to load your Steam library.");
                return;
            }
            
            render_filter_bar(ui, self);
            ui.add_space(4.0);
            
            let filtered_indices = get_filtered_indices(self);
            let filtered_count = filtered_indices.len();
            
            if filtered_count != self.games.len() {
                ui.label(format!("Showing {} of {} games", filtered_count, self.games.len()));
            }
            
            let needs_fetch = render_games_table(ui, self, filtered_indices);
            
            // Desktop loads achievements synchronously, so handle any needed fetches
            for appid in needs_fetch {
                self.request_achievements(appid);
            }
        });
    }
}
