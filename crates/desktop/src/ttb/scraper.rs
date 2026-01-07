//! HLTB scraper - fetches time-to-beat data from HowLongToBeat using headless Chrome

use overachiever_core::TtbTimes;
use chrono::Utc;
use headless_chrome::{Browser, LaunchOptions};
use std::ffi::OsStr;
use std::io::Write;
use std::time::Duration;

const HLTB_SEARCH_URL: &str = "https://howlongtobeat.com/?q=";

/// Log to ttb_log.txt for debugging
fn ttb_log(msg: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("ttb_log.txt")
    {
        let _ = writeln!(file, "[{}] {}", chrono::Local::now().format("%H:%M:%S"), msg);
    }
}

// Recent Chrome user-agent (Chrome is what headless_chrome actually is)
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Create a new browser instance with stealth settings
fn create_browser() -> Result<Browser, TtbError> {
    // Store args as Strings so they live long enough
    let args_strings: Vec<String> = vec![
        format!("--user-agent={}", USER_AGENT),
        // Disable automation flags
        "--disable-blink-features=AutomationControlled".to_string(),
        // Make it look more like a real browser
        "--disable-infobars".to_string(),
        "--disable-extensions".to_string(),
        "--disable-dev-shm-usage".to_string(),
        "--disable-gpu".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        // Window size like a real user
        "--window-size=1920,1080".to_string(),
        // Language
        "--lang=en-US".to_string(),
    ];
    
    let args: Vec<&OsStr> = args_strings.iter().map(|s| OsStr::new(s.as_str())).collect();
    
    let options = LaunchOptions {
        headless: true,
        sandbox: false,
        idle_browser_timeout: Duration::from_secs(60),
        args,
        ..Default::default()
    };
    Browser::new(options).map_err(|e| TtbError::Browser(e.to_string()))
}

/// Inject stealth scripts to hide automation detection
fn apply_stealth(tab: &headless_chrome::Tab) -> Result<(), TtbError> {
    // Remove webdriver property and other automation indicators
    let stealth_script = r#"
        // Remove webdriver flag
        Object.defineProperty(navigator, 'webdriver', {
            get: () => undefined
        });
        
        // Mock plugins (headless has none)
        Object.defineProperty(navigator, 'plugins', {
            get: () => [
                { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer' },
                { name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai' },
                { name: 'Native Client', filename: 'internal-nacl-plugin' }
            ]
        });
        
        // Mock languages
        Object.defineProperty(navigator, 'languages', {
            get: () => ['en-US', 'en']
        });
        
        // Hide automation in chrome object
        window.chrome = {
            runtime: {},
            loadTimes: function() {},
            csi: function() {},
            app: {}
        };
        
        // Mock permissions
        const originalQuery = window.navigator.permissions.query;
        window.navigator.permissions.query = (parameters) => (
            parameters.name === 'notifications' ?
                Promise.resolve({ state: Notification.permission }) :
                originalQuery(parameters)
        );
    "#;
    
    tab.evaluate(stealth_script, false)
        .map_err(|e| TtbError::Browser(format!("Stealth injection failed: {:?}", e)))?;
    
    Ok(())
}

/// Error type for TTB scraping
#[derive(Debug)]
pub enum TtbError {
    Browser(String),
    Parse(String),
    NotFound,
}

impl std::fmt::Display for TtbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TtbError::Browser(e) => write!(f, "Browser error: {}", e),
            TtbError::Parse(e) => write!(f, "Parse error: {}", e),
            TtbError::NotFound => write!(f, "Game not found on HLTB"),
        }
    }
}

/// Parsed game result from HLTB
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HltbResult {
    pub name: String,
    pub main: Option<f32>,
    #[serde(rename = "mainExtra")]
    pub main_extra: Option<f32>,
    pub completionist: Option<f32>,
}

/// Clean up game name for HLTB search
/// Removes trademark symbols and other characters that interfere with search
fn sanitize_game_name(name: &str) -> String {
    name.replace('™', "")
        .replace('®', "")
        .replace('©', "")
        .trim()
        .to_string()
}

/// Search HLTB for a game by name using headless Chrome
pub fn search_game(name: &str) -> Result<Vec<HltbResult>, TtbError> {
    let browser = create_browser()?;
    
    let tab = browser.new_tab().map_err(|e| TtbError::Browser(format!("{:?}", e)))?;
    
    // Apply stealth scripts before navigation
    apply_stealth(&tab)?;
    
    // Clean up game name for better search results
    let clean_name = sanitize_game_name(name);
    
    // Navigate to the search page
    // HLTB expects double-encoded URLs
    let encoded_once = urlencoding::encode(&clean_name);
    let encoded_twice = urlencoding::encode(&encoded_once);
    let search_url = format!("{}{}", HLTB_SEARCH_URL, encoded_twice);

    ttb_log(&format!("Original: '{}' -> Clean: '{}' -> URL: {}", name, clean_name, search_url));
    
    tab.navigate_to(&search_url)
        .map_err(|e| TtbError::Browser(format!("{:?}", e)))?;
    
    // Wait for the page to load and results to appear
    tab.wait_until_navigated()
        .map_err(|e| TtbError::Browser(format!("{:?}", e)))?;
    
    // Re-apply stealth after navigation (page load clears it)
    let _ = apply_stealth(&tab);
    
    // Wait longer for JavaScript to render results
    std::thread::sleep(Duration::from_secs(5));
    
    // First, let's see what's on the page
    let debug_script = r#"
        (function() {
            return JSON.stringify({
                title: document.title,
                url: window.location.href,
                bodyLength: document.body?.innerHTML?.length || 0,
                hasResults: document.body?.innerHTML?.includes('/game/') || false
            });
        })()
    "#;
    
    let mut debug_info = String::new();
    if let Ok(debug_result) = tab.evaluate(debug_script, true) {
        if let Some(val) = debug_result.value {
            debug_info = format!("{}", val);
        }
    }
    ttb_log(&format!("Page debug info: {}", debug_info));
    
    // Extract game data from the page using JavaScript
    // HLTB uses a specific structure - let's be more thorough
    let js_script = r#"
        (function() {
            const results = [];
            
            // Find all links that contain /game/ in href
            const allLinks = Array.from(document.querySelectorAll('a'));
            const gameLinks = allLinks.filter(a => a.href && a.href.includes('/game/'));
            
            const seen = new Set();
            
            gameLinks.forEach(link => {
                // Skip if we've seen this game link
                const href = link.href;
                if (seen.has(href)) return;
                seen.add(href);
                
                // Try to find the card container
                let card = link.closest('li') || link.closest('[class*="Card"]') || link.parentElement?.parentElement?.parentElement;
                if (!card) card = link.parentElement;
                
                // Get title - check the link text first, then look for headings
                let title = '';
                const h3 = link.querySelector('h3, h2') || card?.querySelector('h3, h2');
                if (h3) {
                    title = h3.textContent.trim();
                } else {
                    // Use the link text, clean up time values
                    const linkText = link.textContent || '';
                    title = linkText.replace(/\d+½?\s*Hours?/gi, '').replace(/Main Story|Main \+ Extra|Completionist/gi, '').trim();
                }
                
                if (!title || title.length < 2) return;
                
                // Find times - look in the card or nearby elements
                let searchArea = card || link.parentElement;
                const cardText = searchArea?.textContent || '';
                
                // Match patterns like "12½ Hours" or "12 Hours"
                const timeMatches = cardText.match(/(\d+(?:½)?)\s*Hours?/gi) || [];
                const times = timeMatches.map(t => {
                    const numMatch = t.match(/(\d+)(½)?/);
                    if (!numMatch) return 0;
                    let hours = parseInt(numMatch[1], 10);
                    if (numMatch[2] === '½') hours += 0.5;
                    return hours;
                }).filter(t => t > 0);
                
                // Get unique times sorted
                const uniqueTimes = [...new Set(times)].sort((a, b) => a - b);
                
                results.push({
                    name: title,
                    main: uniqueTimes[0] || null,
                    mainExtra: uniqueTimes[1] || null,
                    completionist: uniqueTimes[2] || null
                });
            });
            
            return JSON.stringify(results);
        })()
    "#;
    
    let result = tab.evaluate(js_script, true)
        .map_err(|e| TtbError::Browser(format!("{:?}", e)))?;
    
    let json_str: String = result.value
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| TtbError::Parse(format!("Failed to get search results. Debug: {}", debug_info)))?;
    
    let parsed: Vec<HltbResult> = serde_json::from_str(&json_str)
        .map_err(|e| TtbError::Parse(format!("{} - Raw: {}", e, json_str)))?;
    
    // Close the tab to free resources
    let _ = tab.close(true);
    
    // If no results, include debug info in error
    if parsed.is_empty() {
        return Err(TtbError::Parse(format!("No results found. Debug: {}", debug_info)));
    }
    
    Ok(parsed)
}

/// Calculate string similarity (simple word-based matching)
fn similarity(a: &str, b: &str) -> f32 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Exact match
    if a_lower == b_lower {
        return 1.0;
    }

    // Word-based matching
    let a_words: Vec<&str> = a_lower.split_whitespace().collect();
    let b_words: Vec<&str> = b_lower.split_whitespace().collect();

    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }

    let matches = a_words.iter().filter(|w| b_words.contains(w)).count();
    let total = a_words.len().max(b_words.len());

    matches as f32 / total as f32
}

/// Find best matching game from search results
pub fn find_best_match(game_name: &str, results: &[HltbResult]) -> Option<HltbResult> {
    if results.is_empty() {
        return None;
    }

    // Find the entry with highest similarity
    let mut best_match: Option<&HltbResult> = None;
    let mut best_score: f32 = 0.0;

    for entry in results {
        let score = similarity(game_name, &entry.name);
        if score > best_score {
            best_score = score;
            best_match = Some(entry);
        }
    }

    // Only return if similarity is above threshold
    if best_score >= 0.4 {
        best_match.cloned()
    } else {
        // Fall back to first result if search returned results
        results.first().cloned()
    }
}

/// Fetch TTB times for a game by name
pub fn fetch_ttb_times(appid: u64, game_name: &str) -> Result<TtbTimes, TtbError> {
    fetch_ttb_times_with_query(appid, game_name, game_name)
}

/// Fetch TTB times using a custom search query
pub fn fetch_ttb_times_with_query(appid: u64, match_name: &str, search_query: &str) -> Result<TtbTimes, TtbError> {
    let results = search_game(search_query)?;

    let entry = find_best_match(match_name, &results).ok_or(TtbError::NotFound)?;

    Ok(TtbTimes {
        appid,
        main: entry.main,
        main_extra: entry.main_extra,
        completionist: entry.completionist,
        updated_at: Utc::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_similarity() {
        assert_eq!(similarity("The Witcher 3", "The Witcher 3"), 1.0);
        assert!(similarity("Witcher 3", "The Witcher 3") > 0.5);
        assert!(similarity("completely different", "The Witcher 3") < 0.3);
    }
}
