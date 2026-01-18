//! Achievement difficulty rating display functions

use egui::{self, Color32, RichText, Ui};

/// Get difficulty label for rating (with trailing space to avoid border clipping)
pub fn difficulty_label(rating: u8) -> &'static str {
    match rating {
        1 => "Very easy  ",
        2 => "Easy  ",
        3 => "Moderate  ",
        4 => "Hard  ",
        5 => "Extreme  ",
        _ => "",
    }
}

/// Get icon for difficulty rating (single icon per level)
pub fn difficulty_icon(rating: u8) -> &'static str {
    match rating {
        1 => "🐢",  // Turtle - Very easy
        2 => "🐇",  // Rabbit - Easy
        3 => "🏃",  // Runner - Moderate
        4 => "⚡",  // Lightning - Hard
        5 => "🔥",  // Fire - Extreme
        _ => "",
    }
}

/// Get color for difficulty label (green for easy, red for extreme)
pub fn difficulty_color(rating: u8) -> Color32 {
    match rating {
        1 => Color32::from_rgb(80, 200, 80),   // Green - Very easy
        2 => Color32::from_rgb(140, 200, 60),  // Yellow-green - Easy  
        3 => Color32::from_rgb(200, 200, 60),  // Yellow - Moderate
        4 => Color32::from_rgb(230, 140, 50),  // Orange - Hard
        5 => Color32::from_rgb(230, 60, 60),   // Red - Extreme
        _ => Color32::GRAY,
    }
}

/// Render compact average rating display (read-only, no interaction)
/// Shows a single difficulty icon with label and vote count
pub fn render_compact_avg_rating(ui: &mut Ui, avg_rating: Option<u8>, rating_count: Option<i32>) {
    let Some(rating) = avg_rating else {
        return; // Don't show anything if no rating
    };
    
    // Add count in parentheses first (since we're right-to-left)
    if let Some(count) = rating_count {
        ui.label(RichText::new(format!("({})", count)).color(Color32::GRAY).size(10.0));
        ui.add_space(4.0);
    }
    
    // Add difficulty label with gradient color
    ui.label(RichText::new(difficulty_label(rating)).color(difficulty_color(rating)).size(10.0));
    ui.add_space(4.0);
    
    // Single difficulty icon
    ui.label(RichText::new(difficulty_icon(rating)).color(difficulty_color(rating)).size(12.0));
}
