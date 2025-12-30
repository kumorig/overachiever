//! Steam library detection - finds installed games by scanning Steam library folders

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Information about an installed game from ACF manifest
#[derive(Debug, Clone)]
pub struct InstalledGameInfo {
    pub appid: u64,
    pub size_on_disk: Option<u64>,
}

/// Get the Steam installation path on Windows
fn get_steam_path() -> Option<PathBuf> {
    // Try common Steam installation paths
    let paths = [
        "C:\\Program Files (x86)\\Steam",
        "C:\\Program Files\\Steam",
        "D:\\Steam",
        "D:\\Program Files (x86)\\Steam",
        "E:\\Steam",
    ];
    
    for path in &paths {
        let p = PathBuf::from(path);
        if p.exists() && p.join("steamapps").exists() {
            return Some(p);
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
                let p = PathBuf::from(path);
                if p.exists() {
                    return Some(p);
                }
            }
        }
        
        if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("SOFTWARE\\Valve\\Steam")
        {
            if let Ok(path) = hkcu.get_value::<String, _>("SteamPath") {
                let p = PathBuf::from(path);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    
    None
}

/// Parse libraryfolders.vdf to get all Steam library paths
fn get_library_folders(steam_path: &PathBuf) -> Vec<PathBuf> {
    let mut folders = vec![steam_path.clone()];
    
    let vdf_path = steam_path.join("steamapps").join("libraryfolders.vdf");
    if let Ok(content) = fs::read_to_string(&vdf_path) {
        // Simple VDF parsing - look for "path" entries
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("\"path\"") {
                // Extract path between quotes: "path"		"D:\\SteamLibrary"
                let parts: Vec<&str> = line.split('"').collect();
                if parts.len() >= 4 {
                    let path = parts[3].replace("\\\\", "\\");
                    let p = PathBuf::from(&path);
                    if p.exists() && !folders.contains(&p) {
                        folders.push(p);
                    }
                }
            }
        }
    }
    
    folders
}

/// Parse an ACF file and extract SizeOnDisk value
fn parse_acf_size_on_disk(content: &str) -> Option<u64> {
    // ACF files are VDF format, look for "SizeOnDisk" key
    // Format: "SizeOnDisk"		"1234567890"
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("\"SizeOnDisk\"") {
            // Extract value between quotes after the key
            let parts: Vec<&str> = line.split('"').collect();
            if parts.len() >= 4 {
                if let Ok(size) = parts[3].parse::<u64>() {
                    return Some(size);
                }
            }
        }
    }
    None
}

/// Scan a steamapps folder for installed game appids
fn scan_steamapps_folder(folder: &PathBuf) -> HashSet<u64> {
    let mut installed = HashSet::new();
    
    let steamapps = folder.join("steamapps");
    if let Ok(entries) = fs::read_dir(&steamapps) {
        for entry in entries.flatten() {
            let filename = entry.file_name();
            let filename = filename.to_string_lossy();
            
            // Look for appmanifest_*.acf files
            if filename.starts_with("appmanifest_") && filename.ends_with(".acf") {
                // Extract appid from filename: appmanifest_12345.acf
                let appid_str = filename
                    .trim_start_matches("appmanifest_")
                    .trim_end_matches(".acf");
                if let Ok(appid) = appid_str.parse::<u64>() {
                    installed.insert(appid);
                }
            }
        }
    }
    
    installed
}

/// Scan a steamapps folder for installed games with size info
fn scan_steamapps_folder_with_sizes(folder: &PathBuf) -> Vec<InstalledGameInfo> {
    let mut games = Vec::new();
    
    let steamapps = folder.join("steamapps");
    if let Ok(entries) = fs::read_dir(&steamapps) {
        for entry in entries.flatten() {
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();
            
            // Look for appmanifest_*.acf files
            if filename_str.starts_with("appmanifest_") && filename_str.ends_with(".acf") {
                // Extract appid from filename: appmanifest_12345.acf
                let appid_str = filename_str
                    .trim_start_matches("appmanifest_")
                    .trim_end_matches(".acf");
                if let Ok(appid) = appid_str.parse::<u64>() {
                    // Read and parse the ACF file
                    let acf_path = entry.path();
                    let size_on_disk = fs::read_to_string(&acf_path)
                        .ok()
                        .and_then(|content| parse_acf_size_on_disk(&content));
                    
                    games.push(InstalledGameInfo {
                        appid,
                        size_on_disk,
                    });
                }
            }
        }
    }
    
    games
}

/// Get all installed Steam game appids
pub fn get_installed_games() -> HashSet<u64> {
    let mut installed = HashSet::new();
    
    if let Some(steam_path) = get_steam_path() {
        let library_folders = get_library_folders(&steam_path);
        
        for folder in library_folders {
            let folder_installed = scan_steamapps_folder(&folder);
            installed.extend(folder_installed);
        }
    }
    
    installed
}

/// Get all installed games with their size information
pub fn get_installed_games_with_sizes() -> Vec<InstalledGameInfo> {
    let mut games = Vec::new();
    
    if let Some(steam_path) = get_steam_path() {
        let library_folders = get_library_folders(&steam_path);
        
        for folder in library_folders {
            let folder_games = scan_steamapps_folder_with_sizes(&folder);
            games.extend(folder_games);
        }
    }
    
    games
}

