//! Game sorting logic

use overachiever_core::ui::{SortColumn, SortOrder};
use crate::app::SteamOverachieverApp;

impl SteamOverachieverApp {
    /// Sort games in place based on current sort settings
    pub(crate) fn sort_games(&mut self) {
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
            SortColumn::TimeToBeat => {
                let cache = &self.ttb_cache;
                self.games.sort_by(|a, b| {
                    let a_ttb = cache.get(&a.appid).and_then(|t| t.main).unwrap_or(-1.0);
                    let b_ttb = cache.get(&b.appid).and_then(|t| t.main).unwrap_or(-1.0);
                    let cmp = a_ttb.partial_cmp(&b_ttb).unwrap_or(std::cmp::Ordering::Equal);
                    if order == SortOrder::Descending { cmp.reverse() } else { cmp }
                });
            }
            SortColumn::Votes => {
                // Votes sorting is handled in set_sort in games_table.rs (needs filter_tags context)
                // This is just for the initial sort_games call which won't use Votes
            }
        }
    }
}
