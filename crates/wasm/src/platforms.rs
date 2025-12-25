//! Platform trait implementations for WasmApp

use eframe::egui;
use overachiever_core::{
    Game, GameAchievement, RunHistory, AchievementHistory, LogEntry,
    StatsPanelPlatform, GamesTablePlatform, SortColumn, SortOrder, TriFilter,
    sort_games,
};

use crate::app::{WasmApp, ConnectionState, AppState};
use crate::steam_images::{game_icon_url, proxy_steam_image_url};

// ============================================================================
// StatsPanelPlatform Implementation
// ============================================================================

impl StatsPanelPlatform for WasmApp {
    fn games(&self) -> &[Game] {
        &self.games
    }
    
    fn run_history(&self) -> &[RunHistory] {
        &self.run_history
    }
    
    fn achievement_history(&self) -> &[AchievementHistory] {
        &self.achievement_history
    }
    
    fn log_entries(&self) -> &[LogEntry] {
        &self.log_entries
    }
    
    fn include_unplayed_in_avg(&self) -> bool {
        self.include_unplayed_in_avg
    }
    
    fn set_include_unplayed_in_avg(&mut self, value: bool) {
        self.include_unplayed_in_avg = value;
    }
    
    fn game_icon_source(&self, _ui: &egui::Ui, appid: u64, icon_hash: &str) -> egui::ImageSource<'static> {
        let url = game_icon_url(appid, icon_hash);
        egui::ImageSource::Uri(url.into())
    }
    
    fn achievement_icon_source(&self, _ui: &egui::Ui, icon_url: &str) -> egui::ImageSource<'static> {
        let proxied = proxy_steam_image_url(icon_url);
        egui::ImageSource::Uri(proxied.into())
    }
    
    fn submit_achievement_rating(&mut self, appid: u64, apiname: String, rating: u8) {
        // Store locally first for immediate UI feedback
        self.user_achievement_ratings.insert((appid, apiname.clone()), rating);
        
        // Submit via REST API (async, fire-and-forget)
        if let Some(token) = &self.auth_token {
            let token = token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match crate::http_client::submit_achievement_rating(&token, appid, &apiname, rating).await {
                    Ok(resp) => {
                        web_sys::console::log_1(&format!("Rating submitted: {} stars for {}/{}", rating, resp.appid, resp.apiname).into());
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Failed to submit rating: {}", e).into());
                    }
                }
            });
        }
    }
    
    fn get_user_achievement_rating(&self, appid: u64, apiname: &str) -> Option<u8> {
        self.user_achievement_ratings.get(&(appid, apiname.to_string())).copied()
    }
    
    fn set_user_achievement_rating(&mut self, appid: u64, apiname: String, rating: u8) {
        self.user_achievement_ratings.insert((appid, apiname.clone()), rating);
        // Also submit to server
        self.submit_achievement_rating(appid, apiname, rating);
    }
    
    fn achievements_graph_tab(&self) -> usize {
        self.achievements_graph_tab
    }
    
    fn set_achievements_graph_tab(&mut self, tab: usize) {
        self.achievements_graph_tab = tab;
    }
    
    fn games_graph_tab(&self) -> usize {
        self.games_graph_tab
    }
    
    fn set_games_graph_tab(&mut self, tab: usize) {
        self.games_graph_tab = tab;
    }
    
    fn is_authenticated(&self) -> bool {
        self.auth_token.is_some()
    }
    
    fn navigate_to_achievement(&mut self, appid: u64, apiname: String) {
        // Clear filters so the game is visible
        self.filter_name.clear();
        self.filter_achievements = TriFilter::All;
        self.filter_playtime = TriFilter::All;
        
        // Expand the game row
        self.expanded_rows.insert(appid);
        
        // Request achievements if not cached
        if !self.achievements_cache.contains_key(&appid) {
            if let Some(client) = &self.ws_client {
                client.fetch_achievements(appid);
            }
        }
        
        // Set navigation target for scroll-to behavior and enable one-time scroll
        self.navigation_target = Some((appid, apiname));
        self.needs_scroll_to_target = true;
    }
    
    fn get_log_selected_achievement(&self) -> Option<(u64, String)> {
        self.log_selected_achievement.clone()
    }
    
    fn set_log_selected_achievement(&mut self, appid: u64, apiname: String) {
        self.log_selected_achievement = Some((appid, apiname));
    }
}

// ============================================================================
// GamesTablePlatform Implementation
// ============================================================================

impl GamesTablePlatform for WasmApp {
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
        if let Some(client) = &self.ws_client {
            client.fetch_achievements(appid);
        }
    }
    
    fn get_navigation_target(&self) -> Option<(u64, String)> {
        self.navigation_target.clone()
    }
    
    fn clear_navigation_target(&mut self) {
        self.navigation_target = None;
        self.needs_scroll_to_target = false;
    }
    
    fn needs_scroll_to_target(&self) -> bool {
        self.needs_scroll_to_target
    }
    
    fn mark_scrolled_to_target(&mut self) {
        self.needs_scroll_to_target = false;
    }
    
    fn can_refresh_single_game(&self) -> bool {
        // WASM can refresh if authenticated and not busy
        matches!(self.connection_state, ConnectionState::Authenticated(_)) 
            && self.app_state == AppState::Idle
    }
    
    fn request_single_game_refresh(&mut self, appid: u64) -> bool {
        if !self.can_refresh_single_game() || self.single_game_refreshing.is_some() {
            return false;
        }
        if let Some(client) = &self.ws_client {
            self.single_game_refreshing = Some(appid);
            self.status = format!("Refreshing game {}...", appid);
            client.refresh_single_game(appid);
            true
        } else {
            false
        }
    }
    
    fn is_single_game_refreshing(&self, appid: u64) -> bool {
        self.single_game_refreshing == Some(appid)
    }
}
