//! UI panel rendering for WasmApp

use eframe::egui;
use egui_phosphor::regular;
use overachiever_core::{
    GdprConsent, SidebarPanel, StatsPanelConfig,
    render_stats_content, render_log_content, render_filter_bar, render_games_table,
    get_filtered_indices,
};

use crate::app::{WasmApp, ConnectionState};
use crate::storage::{get_auth_url, clear_token_from_storage, clear_gdpr_consent_from_storage};

impl WasmApp {
    // ========================================================================
    // Top Panel
    // ========================================================================
    
    pub fn render_top_panel(&mut self, ctx: &egui::Context) {
        let is_busy = self.app_state.is_busy();
        let is_authenticated = matches!(self.connection_state, ConnectionState::Authenticated(_));
        let is_guest_view = self.is_guest_view();
        
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Heading with build info tooltip
                let heading = ui.heading("Overachiever");
                if let Some(build_info) = self.build_info.borrow().as_ref() {
                    heading.on_hover_text(format!(
                        "Build #{}\n{}",
                        build_info.build_number,
                        build_info.build_datetime
                    ));
                }
                ui.separator();
                
                // Guest view mode
                if is_guest_view {
                    if let Some(ref user) = self.viewing_user {
                        ui.colored_label(egui::Color32::from_rgb(100, 180, 255), 
                            format!("{} Viewing {}'s library", regular::EYE, user.display_name));
                    } else {
                        ui.spinner();
                        ui.label("Loading...");
                    }
                } else {
                    // Normal authenticated mode
                    match &self.connection_state {
                        ConnectionState::Disconnected | ConnectionState::Connecting => {
                            ui.spinner();
                            ui.label("Connecting...");
                        }
                        ConnectionState::Connected => {
                            ui.spinner();
                            ui.label("Authenticating...");
                        }
                        ConnectionState::Authenticated(user) => {
                            ui.label(format!("{} {}", regular::USER, user.display_name));
                            ui.separator();
                            
                            // Sync button
                            if ui.add_enabled(!is_busy, egui::Button::new(format!("{} Sync", regular::ARROWS_CLOCKWISE))).clicked() {
                                self.start_sync();
                            }
                            
                            // Full Scan button
                            let needs_scan = self.games_needing_scrape();
                            let scan_label = if needs_scan > 0 {
                                format!("{} Full Scan ({})", regular::GAME_CONTROLLER, needs_scan)
                            } else {
                                format!("{} Full Scan", regular::GAME_CONTROLLER)
                            };
                            let can_scan = (needs_scan > 0 || self.force_full_scan) && self.games_loaded;
                            if ui.add_enabled(!is_busy && can_scan, egui::Button::new(scan_label)).clicked() {
                                self.start_full_scan();
                            }
                            
                            ui.checkbox(&mut self.force_full_scan, "Force");
                        }
                        ConnectionState::Error(e) => {
                            ui.colored_label(egui::Color32::RED, format!("{} {}", regular::WARNING, e));
                            if ui.button("Retry").clicked() {
                                self.connection_state = ConnectionState::Disconnected;
                            }
                        }
                    }
                }
                
                ui.separator();
                
                if is_busy {
                    ui.spinner();
                    if let Some((current, total, _)) = &self.scan_progress {
                        let progress = *current as f32 / *total as f32;
                        ui.add(egui::ProgressBar::new(progress)
                            .text(&self.status)
                            .animate(true));
                    } else {
                        ui.label(&self.status);
                    }
                } else {
                    ui.label(&self.status);
                }
                
                // Right side - different for guest vs authenticated
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if is_guest_view {
                        // Guest view mode - show link to own library
                        if ui.button(format!("{} View your own library", regular::HOUSE))
                            .on_hover_text("Sign in to view your own Steam library")
                            .clicked() 
                        {
                            // Navigate to root
                            if let Some(window) = web_sys::window() {
                                let _ = window.location().set_href("/");
                            }
                        }
                    } else if is_authenticated {
                        if ui.button(format!("{} Logout", regular::SIGN_OUT)).clicked() {
                            self.auth_token = None;
                            clear_token_from_storage();
                            self.connection_state = ConnectionState::Disconnected;
                            self.games.clear();
                            self.games_loaded = false;
                        }
                        
                        // GDPR button - only show if consent has been set
                        if self.gdpr_consent.is_set() {
                            if ui.button(format!("{} Privacy", regular::SHIELD_CHECK))
                                .on_hover_text("Privacy Settings")
                                .clicked() 
                            {
                                // Reset consent to show modal again
                                self.gdpr_consent = GdprConsent::Unset;
                                clear_gdpr_consent_from_storage();
                            }
                        }
                        
                        // User profile button - show shareable link
                        if let ConnectionState::Authenticated(ref user) = self.connection_state {
                            if let Some(ref short_id) = user.short_id {
                                let profile_url = format!("https://overachiever.space/{}", short_id);
                                if ui.button(format!("{}", regular::USER))
                                    .on_hover_text_at_pointer(format!("Copy profile link: {}", profile_url))
                                    .clicked() 
                                {
                                    // Copy to clipboard
                                    ui.ctx().copy_text(profile_url);
                                }
                            }
                        }
                    }
                });
            });
        });
    }
    
    // ========================================================================
    // Stats Panel (Right Sidebar)
    // ========================================================================
    
    pub fn render_stats_panel(&mut self, ctx: &egui::Context) {
        // Show stats panel for authenticated users OR guest viewers with games loaded
        let can_show = matches!(self.connection_state, ConnectionState::Authenticated(_)) 
            || (self.is_guest_view() && self.games_loaded);
        if !can_show {
            return;
        }
        
        // Slightly darker background for the sidebar in dark mode
        let panel_fill = ctx.style().visuals.window_fill();
        let darker_fill = egui::Color32::from_rgb(
            panel_fill.r().saturating_sub(8),
            panel_fill.g().saturating_sub(8),
            panel_fill.b().saturating_sub(8),
        );
        let panel_frame = egui::Frame::side_top_panel(&ctx.style())
            .fill(darker_fill);
        
        if !self.show_stats_panel {
            // Collapsed sidebar - show two buttons (Stats and Log)
            egui::SidePanel::right("stats_panel_collapsed")
                .exact_width(36.0)
                .resizable(false)
                .frame(panel_frame)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    // Stats button
                    if ui.button(regular::CHART_LINE.to_string())
                        .on_hover_text("Open Stats Panel")
                        .clicked() 
                    {
                        self.sidebar_panel = SidebarPanel::Stats;
                        self.show_stats_panel = true;
                    }
                    // Log button
                    if ui.button(regular::SCROLL.to_string())
                        .on_hover_text("Open Log Panel")
                        .clicked()
                    {
                        self.sidebar_panel = SidebarPanel::Log;
                        self.show_stats_panel = true;
                    }
                });
            return;
        }
        
        // Get available width - for mobile (< 600px), use full width
        let available_width = ctx.input(|i| i.viewport().inner_rect.map(|r| r.width()).unwrap_or(800.0));
        let is_mobile = available_width < 600.0;
        
        let panel = egui::SidePanel::right("stats_panel")
            .resizable(!is_mobile)
            .frame(panel_frame);
        
        let panel = if is_mobile {
            panel.exact_width(available_width)
        } else {
            // Use min_width like desktop - panel content will fill this width
            // but won't force the panel to grow larger
            panel.min_width(320.0)
        };
        
        panel.show(ctx, |ui| {
                // Top navigation bar: close button + panel tabs
                ui.horizontal(|ui| {
                    // Close button (chevron right to collapse)
                    if ui.small_button(regular::CARET_RIGHT.to_string())
                        .on_hover_text("Close Panel")
                        .clicked() 
                    {
                        self.show_stats_panel = false;
                    }
                    
                    ui.separator();
                    
                    // Panel navigation tabs
                    let stats_selected = self.sidebar_panel == SidebarPanel::Stats;
                    let log_selected = self.sidebar_panel == SidebarPanel::Log;
                    
                    if ui.selectable_label(stats_selected, format!("{} Stats", regular::CHART_LINE)).clicked() {
                        self.sidebar_panel = SidebarPanel::Stats;
                    }
                    if ui.selectable_label(log_selected, format!("{} Log", regular::SCROLL)).clicked() {
                        self.sidebar_panel = SidebarPanel::Log;
                    }
                });
                ui.separator();
                
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.sidebar_panel {
                        SidebarPanel::Stats => {
                            let config = StatsPanelConfig::wasm();
                            render_stats_content(ui, self, &config);
                        }
                        SidebarPanel::Log => {
                            render_log_content(ui, self);
                        }
                    }
                });
            });
    }
    
    // ========================================================================
    // Games Panel (Center)
    // ========================================================================
    
    pub fn render_games_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Show login prompt only if not authenticated AND not in guest view
            if !matches!(self.connection_state, ConnectionState::Authenticated(_)) && !self.is_guest_view() {
                self.render_login_prompt(ui);
                return;
            }
            
            // In guest view mode, show loading spinner until we have the user info
            if self.is_guest_view() && self.viewing_user.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                    ui.label("Loading library...");
                });
                return;
            }
            
            if self.games.is_empty() {
                if !self.games_loaded {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                        ui.label("Loading games...");
                    });
                } else if self.is_guest_view() {
                    ui.centered_and_justified(|ui| {
                        ui.label("This user hasn't synced any games yet.");
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.label("No games found. Click 'Sync' to load your Steam library.");
                            ui.add_space(12.0);
                            if ui.button(format!("{} Sync from Steam", regular::ARROWS_CLOCKWISE)).clicked() {
                                self.start_sync();
                            }
                        });
                    });
                }
                return;
            }
            
            // Show whose library we're viewing
            let heading_text = if let Some(ref user) = self.viewing_user {
                format!("{}'s Library ({} games)", user.display_name, self.games.len())
            } else {
                format!("Games Library ({} games)", self.games.len())
            };
            ui.heading(heading_text);
            ui.separator();
            
            render_filter_bar(ui, self);
            ui.add_space(4.0);
            
            let filtered_indices = get_filtered_indices(self);
            let filtered_count = filtered_indices.len();
            
            if filtered_count != self.games.len() {
                ui.label(format!("Showing {} of {} games", filtered_count, self.games.len()));
            }
            
            let needs_fetch = render_games_table(ui, self, filtered_indices);
            
            // Fetch achievements for any rows that need them
            if let Some(client) = &self.ws_client {
                for appid in needs_fetch {
                    // Use guest method if viewing someone else's library
                    if let Some(ref short_id) = self.viewing_short_id {
                        client.fetch_guest_achievements(short_id, appid);
                    } else {
                        client.fetch_achievements(appid);
                    }
                }
            }
        });
    }
    
    // ========================================================================
    // Login Prompt
    // ========================================================================
    
    pub fn render_login_prompt(&self, ui: &mut egui::Ui) {
        match &self.connection_state {
            ConnectionState::Connecting | ConnectionState::Disconnected => {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                    ui.label("Connecting to server...");
                });
            }
            ConnectionState::Connected => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        
                        // Explanation text - use LayoutJob for inline formatting without spacing issues
                        let mut job = egui::text::LayoutJob::default();
                        job.append("A ", 0.0, egui::TextFormat::simple(egui::FontId::default(), ui.style().visuals.text_color()));
                        job.append("Steam ID", 0.0, egui::TextFormat::simple(egui::FontId::default(), egui::Color32::WHITE));
                        job.append(" is needed to fetch your game list and to see achievement completion status.", 0.0, egui::TextFormat::simple(egui::FontId::default(), ui.style().visuals.text_color()));
                        job.wrap = egui::text::TextWrapping {
                            max_width: ui.available_width().min(500.0),
                            ..Default::default()
                        };
                        ui.label(job);
                        
                        ui.add_space(12.0);
                        
                        let mut job2 = egui::text::LayoutJob::default();
                        job2.append("You also need to set your game list to ", 0.0, egui::TextFormat::simple(egui::FontId::default(), ui.style().visuals.text_color()));
                        job2.append("public", 0.0, egui::TextFormat::simple(egui::FontId::default(), egui::Color32::WHITE));
                        job2.append(" in Steam privacy settings for this to work.", 0.0, egui::TextFormat::simple(egui::FontId::default(), ui.style().visuals.text_color()));
                        job2.wrap = egui::text::TextWrapping {
                            max_width: ui.available_width().min(500.0),
                            ..Default::default()
                        };
                        ui.label(job2);
                        
                        ui.add_space(8.0);
                        ui.label("If you do not want to share this data, then this site will not accomplish much for you.");
                        
                        ui.add_space(24.0);
                        
                        // Steam Sign In button (clickable image with border)
                        let login_url = get_auth_url();
                        let image = egui::Image::new(egui::include_image!("../../../assets/sits_02.png"))
                            .fit_to_exact_size(egui::vec2(109.0, 66.0));
                        
                        let response = ui.add(
                            egui::Button::image(image)
                                .frame(true)
                        );
                        
                        if response.clicked() {
                            let _ = web_sys::window()
                                .and_then(|w| w.location().set_href(&login_url).ok());
                        }
                        
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        
                        ui.add_space(32.0);
                        ui.separator();
                        ui.add_space(16.0);
                        
                        // Instructions in collapsible section
                        egui::CollapsingHeader::new("How to set your Steam profile to public")
                            .default_open(false)
                            .show(ui, |ui| {
                                ui.add_space(16.0);
                                
                                ui.add(
                                    egui::Image::new(egui::include_image!("../../../assets/step1.png"))
                                        .fit_to_exact_size(egui::vec2(348.0,99.0))
                                        .corner_radius(4.0)
                                );
                                ui.add_space(16.0);
                                
                                ui.add(
                                    egui::Image::new(egui::include_image!("../../../assets/step2.png"))
                                        .fit_to_exact_size(egui::vec2(348.0, 200.0))
                                        .corner_radius(4.0)
                                );
                                ui.add_space(16.0);
                                
                                ui.add(
                                    egui::Image::new(egui::include_image!("../../../assets/step3.png"))
                                        .fit_to_exact_size(egui::vec2(300.0, 600.0))
                                        .corner_radius(4.0)
                                );
                                ui.add_space(16.0);
                                
                                ui.add(
                                    egui::Image::new(egui::include_image!("../../../assets/step4.png"))
                                        .fit_to_exact_size(egui::vec2(500.0, 300.0))
                                        .corner_radius(4.0)
                                );
                            });
                        
                        ui.add_space(20.0);
                    });
                });
            }
            ConnectionState::Error(e) => {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                });
            }
            ConnectionState::Authenticated(_) => {}
        }
    }
}
