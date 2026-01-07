//! Time To Beat (TTB) integration - scrapes HowLongToBeat for game completion times

mod scraper;

pub use scraper::*;

/// Fetch the English name for a game from Steam Store API
/// Returns None if the request fails or the game is not found
pub fn fetch_english_name(appid: u64) -> Option<String> {
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}&l=english",
        appid
    );

    let response = reqwest::blocking::get(&url).ok()?;
    let body: serde_json::Value = response.json().ok()?;

    // Response format: { "appid": { "success": true, "data": { "name": "..." } } }
    let app_data = body.get(appid.to_string())?;
    if !app_data.get("success")?.as_bool()? {
        return None;
    }

    app_data
        .get("data")?
        .get("name")?
        .as_str()
        .map(|s| s.to_string())
}

/// Clean a game name for HLTB search:
/// - Remove apostrophe+s (e.g., "Devil's Kiss" → "Devil Kiss")
/// - Remove dashes, colons, and other ASCII symbols (keep UTF-8 like Japanese/Korean)
/// - Normalize multiple spaces to single space
pub fn clean_game_name_for_search(name: &str) -> String {
    // First, remove 's (apostrophe+s)
    let without_apostrophe_s = name.replace("'s", "").replace("'s", "");

    // Remove ASCII symbols but keep letters, digits, spaces, and non-ASCII (UTF-8) characters
    let cleaned: String = without_apostrophe_s
        .chars()
        .map(|c| {
            if c.is_ascii() {
                // For ASCII: keep alphanumeric and space, replace symbols with space
                if c.is_ascii_alphanumeric() || c == ' ' {
                    c
                } else {
                    ' '
                }
            } else {
                // Keep all non-ASCII characters (Japanese, Korean, Chinese, etc.)
                c
            }
        })
        .collect();

    // Normalize multiple spaces to single space and trim
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

