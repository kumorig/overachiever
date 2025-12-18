use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

const CACHE_DIR: &str = "icon_cache";

/// Icon cache manager that downloads and caches achievement icons locally
pub struct IconCache {
    cache_dir: PathBuf,
    /// Set of URLs currently being downloaded (to avoid duplicate downloads)
    downloading: Arc<Mutex<HashSet<String>>>,
}

impl IconCache {
    pub fn new() -> Self {
        let cache_dir = PathBuf::from(CACHE_DIR);
        
        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            let _ = fs::create_dir_all(&cache_dir);
        }
        
        Self {
            cache_dir,
            downloading: Arc::new(Mutex::new(HashSet::new())),
        }
    }
    
    /// Get the local path for a cached icon, or None if not yet cached
    fn get_cache_path(&self, url: &str) -> PathBuf {
        // Create a safe filename from the URL
        // Steam icon URLs look like: https://steamcdn-a.akamaihd.net/steamcommunity/public/images/apps/APPID/HASH.jpg
        let filename = url
            .rsplit('/')
            .next()
            .unwrap_or("unknown.jpg")
            .to_string();
        
        // Include a hash of the full URL to handle potential filename collisions
        let url_hash = simple_hash(url);
        let safe_filename = format!("{}_{}", url_hash, filename);
        
        self.cache_dir.join(safe_filename)
    }
    
    /// Check if icon is cached and return the local path if so
    pub fn get_cached_path(&self, url: &str) -> Option<PathBuf> {
        if url.is_empty() {
            return None;
        }
        
        let cache_path = self.get_cache_path(url);
        
        if cache_path.exists() {
            Some(cache_path)
        } else {
            // Trigger background download
            self.trigger_download(url.to_string(), cache_path);
            None
        }
    }
    
    /// Load cached icon bytes, or return None and trigger download
    pub fn get_icon_bytes(&self, url: &str) -> Option<Vec<u8>> {
        if let Some(path) = self.get_cached_path(url) {
            fs::read(&path).ok()
        } else {
            None
        }
    }
    
    /// Get the URI for an icon - returns original URL (caching happens in background)
    pub fn get_icon_uri(&self, url: &str) -> String {
        if url.is_empty() {
            return url.to_string();
        }
        
        let cache_path = self.get_cache_path(url);
        
        // If already cached, return file:// URI with proper Windows format
        if cache_path.exists() {
            if let Ok(abs_path) = cache_path.canonicalize() {
                // Windows canonicalize returns \\?\ prefix, need to handle it
                let path_str = abs_path.to_string_lossy();
                let clean_path = path_str
                    .strip_prefix(r"\\?\")
                    .unwrap_or(&path_str)
                    .replace('\\', "/");
                return format!("file:///{}", clean_path);
            }
        }
        
        // Not cached - trigger background download and return original URL for now
        self.trigger_download(url.to_string(), cache_path);
        
        url.to_string()
    }
    
    /// Trigger a background download of an icon
    fn trigger_download(&self, url: String, cache_path: PathBuf) {
        let downloading = self.downloading.clone();
        
        // Check if already downloading
        {
            let mut set = downloading.lock().unwrap();
            if set.contains(&url) {
                return;
            }
            set.insert(url.clone());
        }
        
        // Download in background thread
        thread::spawn(move || {
            if let Ok(response) = reqwest::blocking::get(&url) {
                if let Ok(bytes) = response.bytes() {
                    let _ = fs::write(&cache_path, &bytes);
                }
            }
            
            // Remove from downloading set
            let mut set = downloading.lock().unwrap();
            set.remove(&url);
        });
    }
    
    /// Check if an icon is cached locally
    #[allow(dead_code)]
    pub fn is_cached(&self, url: &str) -> bool {
        self.get_cache_path(url).exists()
    }
}

impl Default for IconCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple hash function for creating unique filenames
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for c in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(c as u64);
    }
    hash
}
