/// TTB formatting utilities and dialog components

use crate::models::Game;

/// Format seconds into hours and minutes display
pub fn format_ttb_time(seconds: i32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    
    if minutes > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}h", hours)
    }
}

/// Parse time input (hours and minutes) into seconds
pub fn parse_time_input(hours: i32, minutes: i32) -> i32 {
    hours * 3600 + minutes * 60
}

/// Get TTB display string for a game with fallback indicator
/// Returns (display_text, is_fallback)
pub fn get_ttb_display(game: &Game, time_type: TtbTimeType) -> Option<(String, bool)> {
    match time_type {
        TtbTimeType::Main => {
            if let Some(seconds) = game.my_ttb_main_seconds {
                Some((format_ttb_time(seconds), false))
            } else if let Some(seconds) = game.avg_user_ttb_main_seconds {
                Some((format!("{} ({})", format_ttb_time(seconds), game.user_ttb_report_count), false))
            } else {
                None // HLTB fallback would be handled by desktop/WASM specific code
            }
        }
        TtbTimeType::Extra => {
            if let Some(seconds) = game.my_ttb_extra_seconds {
                Some((format_ttb_time(seconds), false))
            } else if let Some(seconds) = game.avg_user_ttb_extra_seconds {
                Some((format!("{} ({})", format_ttb_time(seconds), game.user_ttb_report_count), false))
            } else {
                None
            }
        }
        TtbTimeType::Completionist => {
            if let Some(seconds) = game.my_ttb_completionist_seconds {
                Some((format_ttb_time(seconds), false))
            } else if let Some(seconds) = game.avg_user_ttb_completionist_seconds {
                Some((format!("{} ({})", format_ttb_time(seconds), game.user_ttb_report_count), false))
            } else {
                None
            }
        }
    }
}

pub enum TtbTimeType {
    Main,
    Extra,
    Completionist,
}

/// State for the TTB reporting dialog
#[derive(Default, Clone)]
pub struct TtbDialogState {
    pub appid: u64,
    pub game_name: String,
    pub completion_message: Option<String>,
    
    // Input fields (hours and minutes separately for better UX)
    pub main_hours: String,
    pub main_minutes: String,
    pub extra_hours: String,
    pub extra_minutes: String,
    pub completionist_hours: String,
    pub completionist_minutes: String,
    
    pub is_open: bool,
}

impl TtbDialogState {
    pub fn new(appid: u64, game_name: String, completion_message: Option<String>) -> Self {
        Self {
            appid,
            game_name,
            completion_message,
            main_hours: String::new(),
            main_minutes: String::new(),
            extra_hours: String::new(),
            extra_minutes: String::new(),
            completionist_hours: String::new(),
            completionist_minutes: String::new(),
            is_open: true,
        }
    }
    
    pub fn open(&mut self, appid: u64, game_name: String, completion_message: Option<String>) {
        self.appid = appid;
        self.game_name = game_name;
        self.completion_message = completion_message;
        self.is_open = true;
    }
    
    pub fn close(&mut self) {
        self.is_open = false;
        // Clear inputs
        self.main_hours.clear();
        self.main_minutes.clear();
        self.extra_hours.clear();
        self.extra_minutes.clear();
        self.completionist_hours.clear();
        self.completionist_minutes.clear();
    }
    
    /// Prefill the dialog with existing TTB data
    pub fn prefill_from_game(&mut self, game: &Game) {
        if let Some(seconds) = game.my_ttb_main_seconds {
            let hours = seconds / 3600;
            let minutes = (seconds % 3600) / 60;
            self.main_hours = hours.to_string();
            self.main_minutes = minutes.to_string();
        }
        
        if let Some(seconds) = game.my_ttb_extra_seconds {
            let hours = seconds / 3600;
            let minutes = (seconds % 3600) / 60;
            self.extra_hours = hours.to_string();
            self.extra_minutes = minutes.to_string();
        }
        
        if let Some(seconds) = game.my_ttb_completionist_seconds {
            let hours = seconds / 3600;
            let minutes = (seconds % 3600) / 60;
            self.completionist_hours = hours.to_string();
            self.completionist_minutes = minutes.to_string();
        }
    }
    
    /// Get the reported times in seconds (returns None if field is empty)
    pub fn get_times(&self) -> (Option<i32>, Option<i32>, Option<i32>) {
        let main = self.parse_time_field(&self.main_hours, &self.main_minutes);
        let extra = self.parse_time_field(&self.extra_hours, &self.extra_minutes);
        let completionist = self.parse_time_field(&self.completionist_hours, &self.completionist_minutes);
        (main, extra, completionist)
    }
    
    fn parse_time_field(&self, hours: &str, minutes: &str) -> Option<i32> {
        let h = hours.parse::<i32>().unwrap_or(0);
        let m = minutes.parse::<i32>().unwrap_or(0);
        
        if h == 0 && m == 0 {
            None
        } else {
            Some(parse_time_input(h, m))
        }
    }
}

/// Render the TTB reporting dialog
/// Returns true if user clicked Submit, false if Cancel
pub fn show_ttb_dialog(ui: &mut egui::Ui, state: &mut TtbDialogState) -> Option<bool> {
    let mut result = None;
    let mut is_open = state.is_open;
    
    egui::Window::new("Report Time to Beat")
        .open(&mut is_open)
        .resizable(false)
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            ui.set_min_width(400.0);
            
            // Show completion message if present
            if let Some(ref msg) = state.completion_message {
                ui.label(egui::RichText::new(msg).strong());
                ui.add_space(8.0);
            }
            
            ui.label(format!("Game: {}", state.game_name));
            ui.add_space(8.0);
            
            ui.label("Enter your completion times (leave blank if you haven't completed that mode):");
            ui.add_space(8.0);
            
            // Main story
            ui.horizontal(|ui| {
                ui.label("Main Story:");
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut state.main_hours).desired_width(50.0));
                ui.label("h");
                ui.add(egui::TextEdit::singleline(&mut state.main_minutes).desired_width(50.0));
                ui.label("m");
            });
            
            // Main + Extras
            ui.horizontal(|ui| {
                ui.label("Main + Extras:");
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut state.extra_hours).desired_width(50.0));
                ui.label("h");
                ui.add(egui::TextEdit::singleline(&mut state.extra_minutes).desired_width(50.0));
                ui.label("m");
            });
            
            // 100% Completionist
            ui.horizontal(|ui| {
                ui.label("100% Completionist:");
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut state.completionist_hours).desired_width(50.0));
                ui.label("h");
                ui.add(egui::TextEdit::singleline(&mut state.completionist_minutes).desired_width(50.0));
                ui.label("m");
            });
            
            ui.add_space(16.0);
            
            // Buttons
            ui.horizontal(|ui| {
                if ui.button("Submit").clicked() {
                    result = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    result = Some(false);
                }
            });
        });
    
    state.is_open = is_open;
    
    // Close and clear if canceled
    if result == Some(false) {
        state.close();
    }
    
    result
}
