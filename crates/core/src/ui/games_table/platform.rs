//! Platform trait for games table rendering

use super::types::{SortColumn, SortOrder, TriFilter};
use super::super::StatsPanelPlatform;
use crate::{Game, GameAchievement, TtbTimes};

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
    fn get_cached_achievements(&self, appid: u64) -> Option<&Vec<GameAchievement>>;
    
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
    fn get_ttb_times(&self, _appid: u64) -> Option<&TtbTimes> { None }

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

    /// Request to show TTB reporting dialog (platform-specific implementation)
    fn request_ttb_dialog(&mut self, _appid: u64, _game_name: &str, _game: Option<&Game>, _completion_message: Option<String>) {}

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

    // ============================================================================
    // Hidden Games Methods
    // ============================================================================

    /// Get hidden games filter state (All, Show Only Hidden, Hide Hidden)
    fn filter_hidden(&self) -> TriFilter { TriFilter::Without }  // Default: hide hidden games

    /// Set hidden games filter state
    fn set_filter_hidden(&mut self, _filter: TriFilter) {}

    /// Toggle manual hidden status for a game
    fn toggle_game_hidden(&mut self, _appid: u64) {}

    /// Sync steam_hidden from Steam's sharedconfig.vdf
    fn sync_steam_hidden(&mut self) {}
}
