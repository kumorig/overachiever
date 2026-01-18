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
                
                // Try to find the card container - go up several levels to get the full card
                let card = link.closest('li') || link.closest('[class*="Card"]');
                if (!card) {
                    // Go up the DOM tree to find the containing element
                    let el = link;
                    for (let i = 0; i < 5; i++) {
                        el = el.parentElement;
                        if (!el) break;
                        // Check if this element contains the heading and time info
                        if (el.textContent.includes('Main Story') || el.textContent.includes('Hours')) {
                            card = el;
                            break;
                        }
                    }
                }
                if (!card) card = link.parentElement;
                
                // Get title - check the link text first, then look for headings
                let title = '';
                const h3 = link.querySelector('h3, h2') || card?.querySelector('h3, h2');
                if (h3) {
                    title = h3.textContent.trim();
                } else {
                    // Use the link text, but exclude common labels
                    const linkText = link.textContent || '';
                    title = linkText.split('\n')[0].trim(); // Take first line
                    // Remove common time-related text
                    title = title.replace(/Main Story|Main \+ Extra|Completionist|--/gi, '').trim();
                }
                
                if (!title || title.length < 2) return;
                
                // Find times - look in the card text
                const cardText = card?.textContent || link.parentElement?.textContent || '';
                
                // Parse times from text like "Main Story3½ Hours" (note: no space before number)
                // Use word boundary before "Main Story" to avoid matching mid-title
                // Don't use word boundaries for other patterns since text is concatenated
                let main = null;
                let mainExtra = null;
                let completionist = null;
                
                // Match "Main StoryX Hours"
                const mainMatch = cardText.match(/\bMain Story\s*(\d+(?:½)?)\s*Hours?/i);
                if (mainMatch && mainMatch[1]) {
                    const numMatch = mainMatch[1].match(/(\d+)(½)?/);
                    if (numMatch) {
                        main = parseInt(numMatch[1], 10);
                        if (numMatch[2] === '½') main += 0.5;
                    }
                }
                
                // Match "Main + ExtraX Hours" - no word boundary since text is concatenated
                const extraMatch = cardText.match(/Main\s*\+\s*Extra\s*(\d+(?:½)?)\s*Hours?/i);
                if (extraMatch && extraMatch[1]) {
                    const numMatch = extraMatch[1].match(/(\d+)(½)?/);
                    if (numMatch) {
                        mainExtra = parseInt(numMatch[1], 10);
                        if (numMatch[2] === '½') mainExtra += 0.5;
                    }
                }
                
                // Match "CompletionistX Hours" - no word boundary since text is concatenated
                const compMatch = cardText.match(/Completionist\s*(\d+(?:½)?)\s*Hours?/i);
                if (compMatch && compMatch[1]) {
                    const numMatch = compMatch[1].match(/(\d+)(½)?/);
                    if (numMatch) {
                        completionist = parseInt(numMatch[1], 10);
                        if (numMatch[2] === '½') completionist += 0.5;
                    }
                }
                
                results.push({
                    name: title,
                    main: main,
                    mainExtra: mainExtra,
                    completionist: completionist
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
    
    ttb_log(&format!("Raw extraction results: {}", json_str));
    
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
#[allow(dead_code)]


/// Find best matching game from search results
pub fn find_best_match(_game_name: &str, results: &[HltbResult]) -> Option<HltbResult> {
    // HLTB search orders results by relevance, so just use the first one
    results.first().cloned()
}

/// Fetch TTB times for a game by name
pub fn fetch_ttb_times(appid: u64, game_name: &str) -> Result<TtbTimes, TtbError> {
    fetch_ttb_times_with_query(appid, game_name, game_name)
}

/// Fetch TTB times using a custom search query
pub fn fetch_ttb_times_with_query(appid: u64, match_name: &str, search_query: &str) -> Result<TtbTimes, TtbError> {
    let results = search_game(search_query)?;

    let entry = find_best_match(match_name, &results).ok_or(TtbError::NotFound)?;
    
    ttb_log(&format!("Best match for '{}': '{}' (main={:?}, extra={:?}, comp={:?})", 
        match_name, entry.name, entry.main, entry.main_extra, entry.completionist));

    Ok(TtbTimes {
        appid,
        main: entry.main,
        main_extra: entry.main_extra,
        completionist: entry.completionist,
        updated_at: Utc::now(),
    })
}

