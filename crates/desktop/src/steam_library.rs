//! Steam library detection - finds installed games by scanning Steam library folders

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

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

