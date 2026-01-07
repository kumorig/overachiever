//! Font enumeration for Windows systems

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Get a list of installed TrueType fonts from the Windows registry
/// Returns a map of font name -> font path, sorted by name
pub fn get_installed_fonts() -> BTreeMap<String, PathBuf> {
    let mut fonts = BTreeMap::new();

    // Windows fonts are registered in the registry
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);

    if let Ok(fonts_key) = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts") {
        let fonts_dir = get_windows_fonts_dir();

        for value in fonts_key.enum_values().filter_map(|v| v.ok()) {
            let (name, reg_value) = value;

            // Get the file path from the registry value
            // Windows stores REG_SZ as UTF-16LE with null terminator
            let file_name: String = match reg_value {
                winreg::RegValue { bytes, .. } => {
                    // Convert UTF-16LE bytes to String
                    let u16_vec: Vec<u16> = bytes
                        .chunks_exact(2)
                        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                        .take_while(|&c| c != 0) // Stop at null terminator
                        .collect();
                    String::from_utf16_lossy(&u16_vec)
                }
            };

            // Only include TrueType fonts (.ttf, .ttc, .otf)
            let lower = file_name.to_lowercase();
            if !lower.ends_with(".ttf") && !lower.ends_with(".ttc") && !lower.ends_with(".otf") {
                continue;
            }

            // Clean up the display name (remove file extension info like "(TrueType)")
            let display_name = clean_font_name(&name);

            // Build the full path
            let path = if file_name.contains('\\') || file_name.contains(':') {
                // Absolute path
                PathBuf::from(&file_name)
            } else {
                // Relative to fonts directory
                fonts_dir.join(&file_name)
            };

            // Only add if the font file exists
            if path.exists() {
                fonts.insert(display_name, path);
            }
        }
    }

    fonts
}

/// Get the Windows fonts directory
fn get_windows_fonts_dir() -> PathBuf {
    if let Ok(windir) = std::env::var("WINDIR") {
        PathBuf::from(windir).join("Fonts")
    } else {
        PathBuf::from(r"C:\Windows\Fonts")
    }
}

/// Clean up the font name by removing common suffixes
fn clean_font_name(name: &str) -> String {
    name.trim()
        .trim_end_matches("(TrueType)")
        .trim_end_matches("(OpenType)")
        .trim_end_matches("(All Res)")
        .trim()
        .to_string()
}

/// Load font data from a file path
pub fn load_font_data(path: &PathBuf) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}
