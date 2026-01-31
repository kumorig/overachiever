///! Steam configuration file parser - reads hidden games from localconfig.vdf

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Log to ttb_log.txt for debugging
fn steam_config_log(msg: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("ttb_log.txt")
    {
        let _ = writeln!(file, "[{}] STEAM_CONFIG: {}", chrono::Local::now().format("%H:%M:%S"), msg);
    }
}

/// Get Steam's userdata path
fn get_steam_userdata_path() -> Option<PathBuf> {
    // Try to find Steam installation
    let steam_paths = [
        "C:\\Program Files (x86)\\Steam",
        "C:\\Program Files\\Steam",
        "D:\\Steam",
        "D:\\Program Files (x86)\\Steam",
        "E:\\Steam",
    ];
    
    for path in &steam_paths {
        let p = PathBuf::from(path);
        if p.exists() {
            let userdata = p.join("userdata");
            if userdata.exists() {
                return Some(userdata);
            }
        }
    }
    
    // Try reading from registry (Windows)
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        
        if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam")
        {
            if let Ok(path) = hklm.get_value::<String, _>("InstallPath") {
                let userdata = PathBuf::from(path).join("userdata");
                if userdata.exists() {
                    return Some(userdata);
                }
            }
        }
        
        if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("SOFTWARE\\Valve\\Steam")
        {
            if let Ok(path) = hkcu.get_value::<String, _>("SteamPath") {
                let userdata = PathBuf::from(path).join("userdata");
                if userdata.exists() {
                    return Some(userdata);
                }
            }
        }
    }
    
    None
}

#[derive(Debug)]
pub struct SteamGameLists {
    pub hidden_games: HashSet<u64>,
    pub private_games: HashSet<u64>,
}

/// Parse localconfig.vdf and extract both hidden and private game AppIDs
/// 
/// Steam stores two types of lists in localconfig.vdf:
/// - "HiddenApps_<userid>"  - Games hidden via "Hide this game"
/// - "PrivateApps_<userid>" - Games marked as private via "Mark as Private"
/// 
/// Format: "PrivateApps_15107825"  "[927890,935560,1025130,...]"
fn parse_steam_game_lists_from_vdf(content: &str, steam_id: &str) -> SteamGameLists {
    let mut lists = SteamGameLists {
        hidden_games: HashSet::new(),
        private_games: HashSet::new(),
    };
    
    steam_config_log(&format!("Searching for HiddenApps and PrivateApps for Steam ID: {}", steam_id));
    
    // Convert Steam64 ID to account ID (Steam3 ID)
    // Steam64 ID = accountID + 76561197960265728
    let steam64_id: u64 = steam_id.parse().unwrap_or(0);
    if steam64_id == 0 {
        steam_config_log("Invalid Steam ID");
        return lists;
    }
    
    let account_id = steam64_id - 76561197960265728;
    let hidden_apps_key = format!("\"HiddenApps_{}\"", account_id);
    let private_apps_key = format!("\"PrivateApps_{}\"", account_id);
    
    steam_config_log(&format!("Looking for keys: {} and {}", hidden_apps_key, private_apps_key));
    
    // Search for both HiddenApps and PrivateApps keys
    for line in content.lines() {
        let trimmed = line.trim();
        
        let (key_name, target_set) = if trimmed.starts_with(&hidden_apps_key) {
            ("HiddenApps", &mut lists.hidden_games)
        } else if trimmed.starts_with(&private_apps_key) {
            ("PrivateApps", &mut lists.private_games)
        } else {
            continue;
        };
        
        steam_config_log(&format!("Found {} line: {}", key_name, trimmed));
        
        // Extract the JSON array part
        // Format: "PrivateApps_15107825"    "[927890,935560,1025130,...]"
        if let Some(json_start) = trimmed.find('[') {
            if let Some(json_end) = trimmed.find(']') {
                let json_str = &trimmed[json_start..=json_end];
                steam_config_log(&format!("{} JSON array: {}", key_name, json_str));
                
                // Parse the JSON array
                if let Ok(app_ids) = serde_json::from_str::<Vec<u64>>(json_str) {
                    steam_config_log(&format!("Parsed {} {} apps", app_ids.len(), key_name));
                    for appid in app_ids {
                        steam_config_log(&format!("  {} game: {}", key_name, appid));
                        target_set.insert(appid);
                    }
                } else {
                    steam_config_log(&format!("Failed to parse {} JSON array", key_name));
                }
            }
        }
    }
    
    lists
}

/// Get lists of hidden and private game AppIDs from Steam's localconfig.vdf
///
/// Steam stores both lists in userdata/<user_id>/config/localconfig.vdf:
/// - "HiddenApps_<accountid>" - Games hidden via "Hide this game"
/// - "PrivateApps_<accountid>" - Games marked as private via "Mark as Private"
pub fn get_steam_game_lists(steam_id: Option<&str>) -> SteamGameLists {
    let empty_lists = SteamGameLists {
        hidden_games: HashSet::new(),
        private_games: HashSet::new(),
    };
    
    let steam_id_str = match steam_id {
        Some(id) => id,
        None => {
            steam_config_log("No Steam ID provided");
            return empty_lists;
        }
    };
    
    let userdata_path = match get_steam_userdata_path() {
        Some(path) => {
            steam_config_log(&format!("Found Steam userdata path: {:?}", path));
            path
        },
        None => {
            steam_config_log("Could not find Steam userdata directory");
            return empty_lists;
        }
    };
    
    // Convert Steam64 ID to account ID to find the right folder
    let steam64_id: u64 = steam_id_str.parse().unwrap_or(0);
    if steam64_id == 0 {
        steam_config_log("Invalid Steam ID");
        return empty_lists;
    }
    
    let account_id = steam64_id - 76561197960265728;
    let user_folder = userdata_path.join(account_id.to_string());
    let localconfig_path = user_folder.join("config").join("localconfig.vdf");
    
    steam_config_log(&format!("Checking: {:?}", localconfig_path));
    
    if !localconfig_path.exists() {
        steam_config_log("  File does not exist");
        return empty_lists;
    }
    
    // Read VDF file
    let content = match fs::read_to_string(&localconfig_path) {
        Ok(c) => {
            steam_config_log(&format!("  Successfully read file ({} bytes)", c.len()));
            c
        },
        Err(e) => {
            steam_config_log(&format!("  Failed to read {:?}: {}", localconfig_path, e));
            return empty_lists;
        }
    };
    
    // Parse and extract game lists
    let lists = parse_steam_game_lists_from_vdf(&content, steam_id_str);
    steam_config_log(&format!("  Found {} hidden games, {} private games", 
        lists.hidden_games.len(), lists.private_games.len()));
    
    lists
}

/// Update steam_hidden and steam_private status for all games in the database based on Steam's localconfig.vdf
pub fn sync_steam_hidden_games(
    conn: &rusqlite::Connection,
    steam_id: &str,
) -> rusqlite::Result<usize> {
    let lists = get_steam_game_lists(Some(steam_id));
    
    steam_config_log(&format!("Found {} hidden games, {} private games from Steam config for user {}", 
        lists.hidden_games.len(), lists.private_games.len(), steam_id));
    if !lists.hidden_games.is_empty() {
        steam_config_log(&format!("Hidden game AppIDs: {:?}", lists.hidden_games));
    }
    if !lists.private_games.is_empty() {
        steam_config_log(&format!("Private game AppIDs: {:?}", lists.private_games));
    }
    
    // First, clear all steam_hidden and steam_private flags for this user
    conn.execute(
        "UPDATE games SET steam_hidden = 0, steam_private = 0 WHERE steam_id = ?1",
        [steam_id],
    )?;
    
    // Then, set steam_hidden = 1 for games that are hidden in Steam
    let mut count = 0;
    for appid in lists.hidden_games {
        let rows = conn.execute(
            "UPDATE games SET steam_hidden = 1 WHERE steam_id = ?1 AND appid = ?2",
            (steam_id, appid as i64),
        )?;
        count += rows;
    }
    
    // Set steam_private = 1 for games that are private in Steam
    for appid in lists.private_games {
        let rows = conn.execute(
            "UPDATE games SET steam_private = 1 WHERE steam_id = ?1 AND appid = ?2",
            (steam_id, appid as i64),
        )?;
        count += rows;
    }
    
    Ok(count)
}
