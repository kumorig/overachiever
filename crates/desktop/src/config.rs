//! Configuration management using config.toml

use overachiever_core::{DataMode, GdprConsent};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CONFIG_PATH: &str = "config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Data mode: local, hybrid, or remote
    #[serde(default)]
    pub data_mode: DataMode,
    
    /// Steam Web API key (required for local/hybrid modes)
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_mode: DataMode::Local,
            steam_web_api_key: String::new(),
            steam_id: String::new(),
            server_url: String::new(),
            gdpr_consent: GdprConsent::Unset,
            cloud_token: None,
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
    
    /// Check if config is valid for current mode
    pub fn is_valid(&self) -> bool {
        match self.data_mode {
            DataMode::Local | DataMode::Hybrid => {
                !self.steam_web_api_key.is_empty() && !self.steam_id.is_empty()
            }
            DataMode::Remote => {
                !self.server_url.is_empty()
            }
        }
    }
    
    /// Check if local Steam API config is valid
    pub fn has_steam_credentials(&self) -> bool {
        !self.steam_web_api_key.is_empty() && !self.steam_id.is_empty()
    }
    
    /// Get steam_id as u64 for API calls
    pub fn steam_id_u64(&self) -> Option<u64> {
        self.steam_id.trim().parse().ok()
    }
}
