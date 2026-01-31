//! Table rendering for games

use egui::{Color32, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular;

use super::platform::GamesTablePlatform;
use super::helpers::{format_timestamp, sort_indicator};
use super::types::SortColumn;
use super::super::instant_tooltip;

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
                use crate::ui::ttb_dialog::{get_ttb_display, TtbTimeType};
                
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
                                        super::super::instant_tooltip(&btn, "Refresh achievements for this game");
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
                                            super::super::instant_tooltip(&btn, tooltip);
                                        } else {
                                            // Install button for non-installed games
                                            let btn = ui.add(egui::Button::new(regular::DOWNLOAD_SIMPLE.to_string()).small());
                                            if btn.clicked() {
                                                platform.install_game(appid);
                                            }
                                            super::super::instant_tooltip(&btn, "Install game from Steam");
                                        }
                                    }
                                    
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
                                            super::super::instant_tooltip(&btn, tooltip);
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
                                            super::super::instant_tooltip(&btn, tooltip);
                                        }
                                    }
                                });
                            } else {
                                ui.label(&game.name);
                            }
                        });

                        // Show TTB data row if expanded and platform shows TTB column
                        if is_expanded && platform.show_ttb_column() {
                            use crate::ui::ttb_dialog::format_ttb_time;
                            use egui::RichText;
                            
                            let has_ttb = platform.get_ttb_times(appid).is_some();
                            let is_blacklisted = platform.is_ttb_blacklisted(appid);

                            // Dedicated TTB row with subtle background
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⏱ Time to Beat:").strong());
                                
                                // Check if we have user-reported data (show gold when report_count > 0)
                                let has_user_data = game.user_ttb_report_count > 0;
                                
                                if has_user_data {
                                    // Show average user TTB (prioritize user's own report) in gold
                                    let gold = egui::Color32::from_rgb(255, 215, 0);
                                    if let Some((text, _)) = get_ttb_display(&game, TtbTimeType::Main) {
                                        ui.label(RichText::new(format!("Main: {}", text)).color(gold));
                                    }
                                    if let Some((text, _)) = get_ttb_display(&game, TtbTimeType::Extra) {
                                        ui.label(RichText::new(format!("| +Extra: {}", text)).color(gold));
                                    }
                                    if let Some((text, _)) = get_ttb_display(&game, TtbTimeType::Completionist) {
                                        ui.label(RichText::new(format!("| 100%: {}", text)).color(gold));
                                    }
                                } else if let Some(ttb) = platform.get_ttb_times(appid) {
                                    // Fall back to HLTB scraped data in light blue
                                    let light_blue = egui::Color32::from_rgb(120, 180, 255);
                                    let has_data = ttb.main.is_some() || ttb.main_extra.is_some() || ttb.completionist.is_some();
                                    if has_data {
                                        if let Some(main_hours) = ttb.main {
                                            ui.label(RichText::new(format!("Main: {:.0}h", main_hours)).color(light_blue));
                                        }
                                        if let Some(extra_hours) = ttb.main_extra {
                                            ui.label(RichText::new(format!("| +Extra: {:.0}h", extra_hours)).color(light_blue));
                                        }
                                        if let Some(comp_hours) = ttb.completionist {
                                            ui.label(RichText::new(format!("| 100%: {:.0}h", comp_hours)).color(light_blue));
                                        }
                                    } else {
                                        ui.label(RichText::new("<no data>").weak());
                                    }
                                } else {
                                    ui.label(RichText::new("—").weak());
                                }
                                
                                // Show "Your TTB" if user has reported different from average
                                if game.my_ttb_main_seconds.is_some() && game.avg_user_ttb_main_seconds.is_some() {
                                    ui.separator();
                                    ui.label(RichText::new("Your time:").strong());
                                    if let Some(seconds) = game.my_ttb_main_seconds {
                                        ui.label(format!("Main: {}", format_ttb_time(seconds)));
                                    }
                                    if let Some(seconds) = game.my_ttb_extra_seconds {
                                        ui.label(format!("| +Extra: {}", format_ttb_time(seconds)));
                                    }
                                    if let Some(seconds) = game.my_ttb_completionist_seconds {
                                        ui.label(format!("| 100%: {}", format_ttb_time(seconds)));
                                    }
                                }
                                
                                // "Report TTB" button on the right
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let btn = ui.add(egui::Button::new(
                                        RichText::new(format!("{} Report TTB", regular::CLOCK))
                                    ).small());
                                    if btn.clicked() {
                                        platform.request_ttb_dialog(appid, &game.name, Some(&game), None);
                                    }
                                    instant_tooltip(&btn, "Report your time to beat for this game");
                                });
                            });

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
                            super::render_achievements_list(ui, platform, appid);
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
                            // Check if we have user-reported data (gold) or HLTB data (light blue)
                            // Show gold when we have at least 1 user report (my_ttb or avg_user_ttb with count > 0)
                            let has_user_data = game.user_ttb_report_count > 0;
                            
                            if has_user_data {
                                // Show user-reported data in gold
                                let gold = egui::Color32::from_rgb(255, 215, 0);
                                if let Some((text, _)) = get_ttb_display(&game, TtbTimeType::Main) {
                                    ui.label(RichText::new(text).color(gold));
                                } else {
                                    ui.label("—");
                                }
                            } else if let Some(ttb) = platform.get_ttb_times(appid) {
                                // Show HLTB data in light blue
                                let light_blue = egui::Color32::from_rgb(120, 180, 255);
                                if let Some(main) = ttb.main {
                                    ui.label(RichText::new(format!("{:.0}h", main)).color(light_blue));
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
