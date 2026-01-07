//! SteamSpy API integration for fetching game tags
//!
//! SteamSpy API: https://steamspy.com/api.php?request=appdetails&appid={appid}
//! Returns: { "tags": { "Tag Name": vote_count, ... } }
//! Rate limit: ~1 request/second

use std::collections::HashMap;

const STEAMSPY_API_URL: &str = "https://steamspy.com/api.php";

/// SteamSpy app details response (we only care about tags)
#[derive(Debug, serde::Deserialize)]
pub struct SteamSpyResponse {
    /// Tags with vote counts: { "Tag Name": vote_count }
    #[serde(default)]
    pub tags: HashMap<String, i64>,
}

/// Fetch tags for a game from SteamSpy
/// Returns Vec<(tag_name, vote_count)> sorted by vote_count descending
pub fn fetch_tags(appid: u64) -> Result<Vec<(String, u32)>, String> {
    let url = format!("{}?request=appdetails&appid={}", STEAMSPY_API_URL, appid);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .header("User-Agent", "Overachiever/1.0")
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("SteamSpy returned status: {}", response.status()));
    }

    let data: SteamSpyResponse = response
        .json()
        .map_err(|e| format!("Failed to parse SteamSpy response: {}", e))?;

    // Convert to Vec and sort by vote count descending
    let mut tags: Vec<(String, u32)> = data
        .tags
        .into_iter()
        .map(|(name, count)| (name, count.max(0) as u32))
        .collect();

    tags.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires network access
    fn test_fetch_tags() {
        // Test with a well-known game (Portal 2)
        let result = fetch_tags(620);
        assert!(result.is_ok());
        let tags = result.unwrap();
        assert!(!tags.is_empty());
        // Portal 2 should have "Puzzle" tag
        assert!(tags.iter().any(|(name, _)| name.contains("Puzzle")));
    }
}
