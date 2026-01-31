//! CJK font download and management

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

const FONT_DIR: &str = "assets";
const SOURCE_HAN_SANS_FILENAME: &str = "SourceHanSans.ttc";
const SOURCE_HAN_SANS_URL: &str = "https://github.com/adobe-fonts/source-han-sans/releases/download/2.005R/01_SourceHanSans.ttc.zip";
const SOURCE_HAN_SANS_ARCHIVE_PATH: &str = "SourceHanSans.ttc";

#[derive(Clone, Debug)]
pub enum DownloadProgress {
    Starting,
    Downloading { bytes_downloaded: u64, total_bytes: Option<u64> },
    Extracting,
    Complete,
    Error(String),
}

/// Get the full path to the Source Han Sans font file
pub fn get_font_path() -> PathBuf {
    PathBuf::from(FONT_DIR).join(SOURCE_HAN_SANS_FILENAME)
}

/// Check if Source Han Sans font is already downloaded
pub fn is_font_downloaded() -> bool {
    get_font_path().exists()
}

/// Download and extract Source Han Sans font
pub fn download_source_han_sans<F>(progress_callback: F) -> Result<(), String>
where
    F: Fn(DownloadProgress) + Send + 'static,
{
    progress_callback(DownloadProgress::Starting);

    // Create assets directory if it doesn't exist
    fs::create_dir_all(FONT_DIR).map_err(|e| format!("Failed to create assets directory: {}", e))?;

    // Download the ZIP file
    let mut response = reqwest::blocking::get(SOURCE_HAN_SANS_URL)
        .map_err(|e| format!("Failed to download font: {}", e))?;

    let total_size = response.content_length();
    
    // Read the response into a buffer with progress tracking
    let mut buffer = Vec::new();
    let mut bytes_downloaded = 0u64;
    let mut chunk = vec![0u8; 8192];

    loop {
        let bytes_read = response.read(&mut chunk)
            .map_err(|e| format!("Failed to read response: {}", e))?;
        
        if bytes_read == 0 {
            break;
        }

        buffer.extend_from_slice(&chunk[..bytes_read]);
        bytes_downloaded += bytes_read as u64;

        progress_callback(DownloadProgress::Downloading {
            bytes_downloaded,
            total_bytes: total_size,
        });
    }

    progress_callback(DownloadProgress::Extracting);

    // Extract the font file from the ZIP
    let cursor = io::Cursor::new(buffer);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| format!("Failed to open ZIP archive: {}", e))?;

    // Find and extract the font file
    let mut font_file = archive.by_name(SOURCE_HAN_SANS_ARCHIVE_PATH)
        .map_err(|e| format!("Font file not found in archive: {}", e))?;

    let output_path = get_font_path();
    let mut output_file = fs::File::create(&output_path)
        .map_err(|e| format!("Failed to create output file: {}", e))?;

    io::copy(&mut font_file, &mut output_file)
        .map_err(|e| format!("Failed to extract font: {}", e))?;

    progress_callback(DownloadProgress::Complete);

    Ok(())
}

/// Delete the downloaded font file
#[allow(dead_code)]
pub fn delete_font() -> Result<(), String> {
    let path = get_font_path();
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("Failed to delete font: {}", e))?;
    }
    Ok(())
}

/// Get the license URL for Source Han Sans
pub fn get_license_url() -> &'static str {
    "https://github.com/adobe-fonts/source-han-sans/blob/master/LICENSE.txt"
}
