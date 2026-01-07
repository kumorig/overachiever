//! Configuration management using config.toml

use overachiever_core::GdprConsent;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CONFIG_PATH: &str = "config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Steam Web API key
    #[serde(default)]
    pub steam_web_api_key: String,

    /// Steam ID (required for local/hybrid modes)
    #[serde(default)]
    pub steam_id: String,

    /// Server URL for hybrid/remote modes
    #[serde(default)]
    pub server_url: String,

    /// GDPR consent status (for hybrid/remote modes)
    #[serde(default)]
    pub gdpr_consent: GdprConsent,

    /// Cloud sync JWT token (obtained via Steam OpenID login)
    #[serde(default)]
    pub cloud_token: Option<String>,

    /// Debug: output recently played response to file
    #[serde(default)]
    pub debug_recently_played: bool,

    /// Custom font family name (None = use default)
    #[serde(default)]
    pub font_family: Option<String>,

    /// Font size in points (default: 14.0)
    #[serde(default = "default_font_size")]
    pub font_size: f32,

    /// Window position X (None = system default)
    #[serde(default)]
    pub window_x: Option<f32>,

    /// Window position Y (None = system default)
    #[serde(default)]
    pub window_y: Option<f32>,

    /// Window width (None = default 1024)
    #[serde(default)]
    pub window_width: Option<f32>,

    /// Window height (None = default 768)
    #[serde(default)]
    pub window_height: Option<f32>,

    /// Window maximized state
    #[serde(default)]
    pub window_maximized: bool,

    /// Game name column width in the games table
    #[serde(default = "default_name_column_width")]
    pub name_column_width: f32,

    /// TTB scan delay between games in seconds (default: 60)
    #[serde(default = "default_ttb_scan_delay_secs")]
    pub ttb_scan_delay_secs: u64,

    /// Tags scan delay between games in seconds (default: 5)
    #[serde(default = "default_tags_scan_delay_secs")]
    pub tags_scan_delay_secs: u64,
}

fn default_name_column_width() -> f32 {
    400.0
}

fn default_font_size() -> f32 {
    14.0
}

fn default_ttb_scan_delay_secs() -> u64 {
    60
}

fn default_tags_scan_delay_secs() -> u64 {
    5
}

impl Default for Config {
    fn default() -> Self {
        Self {
            steam_web_api_key: String::new(),
            steam_id: String::new(),
            server_url: String::new(),
            gdpr_consent: GdprConsent::Unset,
            cloud_token: None,
            debug_recently_played: false,
            font_family: None,
            font_size: default_font_size(),
            window_x: None,
            window_y: None,
            window_width: None,
            window_height: None,
            window_maximized: false,
            name_column_width: default_name_column_width(),
            ttb_scan_delay_secs: default_ttb_scan_delay_secs(),
            tags_scan_delay_secs: default_tags_scan_delay_secs(),
        }
    }
}

impl Config {
    /// Load config from file, creating default if it doesn't exist
    pub fn load() -> Self {
        if Path::new(CONFIG_PATH).exists() {
            match fs::read_to_string(CONFIG_PATH) {
                Ok(content) => {
                    match toml::from_str(&content) {
                        Ok(config) => return config,
                        Err(e) => {
                            eprintln!("Error parsing config.toml: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error reading config.toml: {}", e);
                }
            }
        }
        
        // Return default config (will prompt user to fill in)
        let config = Config::default();
        let _ = config.save(); // Try to create the file
        config
    }
    
    /// Save config to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        fs::write(CONFIG_PATH, content)?;
        Ok(())
    }
    
    /// Check if config is valid (steam credentials required)
    pub fn is_valid(&self) -> bool {
        !self.steam_web_api_key.is_empty() && !self.steam_id.is_empty()
    }
    
    /// Check if local Steam API config is valid
    pub fn has_steam_credentials(&self) -> bool {
        !self.steam_web_api_key.is_empty() && !self.steam_id.is_empty()
    }
    
    /// Get steam_id as u64 for API calls
    pub fn steam_id_u64(&self) -> Option<u64> {
        self.steam_id.trim().parse().ok()
    }
    
    /// Extract short_id from the cloud_token JWT (without verification)
    pub fn get_short_id(&self) -> Option<String> {
        let token = self.cloud_token.as_ref()?;
        
        // JWT format: header.payload.signature
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        
        // Decode the payload (second part) from base64
        use base64::Engine;
        let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[1])
            .ok()?;
        
        // Parse as JSON and extract short_id
        let payload: serde_json::Value = serde_json::from_slice(&payload_bytes).ok()?;
        payload.get("short_id")?.as_str().map(String::from)
    }
}
