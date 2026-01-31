//! Cloud sync functionality for desktop app
//! 
//! Uses Steam OpenID for authentication:
//! 1. User clicks "Link to Cloud" 
//! 2. Browser opens Steam login
//! 3. Steam redirects to localhost callback
//! 4. Desktop captures JWT, saves to config
//! 5. All sync operations use JWT

use overachiever_core::{CloudSyncData, CloudSyncStatus};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const DEFAULT_SERVER_URL: &str = "https://overachiever.space";
const CALLBACK_PORT: u16 = 23847; // Random high port for OAuth callback

#[derive(Debug, Clone, PartialEq)]
pub enum CloudSyncState {
    Idle,
    NotLinked,
    Linking,
    #[allow(dead_code)]
    Checking,
    Uploading(UploadProgress),
    Downloading,
    Deleting,
    Success(String),
    Error(String),
}

/// Progress information for uploads
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UploadProgress {
    /// Bytes sent so far
    pub bytes_sent: usize,
    /// Total bytes to send
    pub total_bytes: usize,
}

/// Result from the Steam login callback
#[derive(Debug, Clone)]
pub struct AuthResult {
    pub token: String,
    pub steam_id: String,
}

/// Result from async cloud operations
#[derive(Debug, Clone)]
pub enum CloudOpResult {
    UploadSuccess,
    UploadProgress(UploadProgress),
    DownloadSuccess(CloudSyncData),
    DeleteSuccess,
    StatusChecked(CloudSyncStatus),
}

/// Start the Steam OpenID login flow
/// Returns a channel that will receive the auth result
pub fn start_steam_login() -> Result<mpsc::Receiver<Result<AuthResult, String>>, String> {
    let (tx, rx) = mpsc::channel();
    
    // Start local callback server in background thread
    thread::spawn(move || {
        match run_callback_server() {
            Ok(result) => { let _ = tx.send(Ok(result)); }
            Err(e) => { let _ = tx.send(Err(e)); }
        }
    });
    
    // Give server a moment to start
    thread::sleep(Duration::from_millis(100));
    
    // Open browser to Steam login
    let callback_url = format!("http://localhost:{}/callback", CALLBACK_PORT);
    let login_url = format!(
        "{}/auth/steam?redirect_uri={}",
        DEFAULT_SERVER_URL,
        urlencoding::encode(&callback_url)
    );
    
    if let Err(e) = open::that(&login_url) {
        return Err(format!("Failed to open browser: {}", e));
    }
    
    Ok(rx)
}

/// Run a temporary local HTTP server to capture the OAuth callback
fn run_callback_server() -> Result<AuthResult, String> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", CALLBACK_PORT))
        .map_err(|e| format!("Failed to start callback server: {}", e))?;
    
    // Set timeout so we don't wait forever
    listener.set_nonblocking(false).ok();
    
    // Wait for connection (with timeout via accept loop)
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(120); // 2 minute timeout
    
    loop {
        if start.elapsed() > timeout {
            return Err("Login timed out - please try again".to_string());
        }
        
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).ok();
                
                // Parse the GET request to extract query params
                // Format: GET /callback?token=xxx&steam_id=yyy HTTP/1.1
                let result = parse_callback_request(&request_line);
                
                // Send response to browser
                let (status, body) = match &result {
                    Ok(_) => ("200 OK", "<html><head><meta charset=\"utf-8\"></head><body><h1>&#10003; Log in successful!</h1><p>You can close this window and return to Overachiever.</p><script>window.close()</script></body></html>"),
                    Err(e) => ("400 Bad Request", &format!("<html><head><meta charset=\"utf-8\"></head><body><h1>&#10007; Login Failed</h1><p>{}</p></body></html>", e) as &str),
                };
                
                let response = format!(
                    "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                
                return result;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => {
                return Err(format!("Callback server error: {}", e));
            }
        }
    }
}

/// Parse the OAuth callback URL to extract token and steam_id
fn parse_callback_request(request: &str) -> Result<AuthResult, String> {
    // Extract path from "GET /callback?params HTTP/1.1"
    let path = request
        .split_whitespace()
        .nth(1)
        .ok_or("Invalid request")?;
    
    // Check for error
    if path.contains("error=") {
        let error = path
            .split('?')
            .nth(1)
            .and_then(|q| q.split('&').find(|p| p.starts_with("error=")))
            .map(|p| p.strip_prefix("error=").unwrap_or("unknown"))
            .unwrap_or("unknown");
        return Err(format!("Steam login failed: {}", error));
    }
    
    // Extract token and steam_id
    let query = path.split('?').nth(1).ok_or("Missing query params")?;
    
    let mut token = None;
    let mut steam_id = None;
    
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("token=") {
            token = Some(value.to_string());
        } else if let Some(value) = param.strip_prefix("steam_id=") {
            steam_id = Some(value.to_string());
        }
    }
    
    match (token, steam_id) {
        (Some(t), Some(s)) => Ok(AuthResult { token: t, steam_id: s }),
        _ => Err("Missing token or steam_id in callback".to_string()),
    }
}

/// Check if user has data in the cloud
pub fn check_cloud_status(token: &str) -> Result<CloudSyncStatus, String> {
    let url = format!("{}/api/sync/status", DEFAULT_SERVER_URL);
    
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }
    
    response.json::<CloudSyncStatus>()
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Upload all local data to cloud (overwrites existing)
/// The progress callback is called with (bytes_sent, total_bytes)
pub fn upload_to_cloud<F>(token: &str, data: &CloudSyncData, progress_callback: F) -> Result<(), String> 
where
    F: Fn(usize, usize) + Send + 'static,
{
    use std::error::Error;
    
    let url = format!("{}/api/sync/upload", DEFAULT_SERVER_URL);
    
    // Serialize data first to get total size
    let json_bytes = serde_json::to_vec(data)
        .map_err(|e| format!("Failed to serialize data: {}", e))?;
    let total_bytes = json_bytes.len();
    
    // Report initial progress (0%)
    progress_callback(0, total_bytes);
    
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120)) // 2 minute timeout for uploads
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    // Use body directly instead of .json() to have serialization control
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(json_bytes)
        .send()
        .map_err(|e| {
            let mut msg = format!("Network error: {}", e);
            if let Some(source) = e.source() {
                msg.push_str(&format!(" (cause: {})", source));
                if let Some(inner) = source.source() {
                    msg.push_str(&format!(" (inner: {})", inner));
                }
            }
            msg
        })?;
    
    // Report completion (since blocking client doesn't give us streaming progress)
    progress_callback(total_bytes, total_bytes);
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }
    
    Ok(())
}

/// Download all data from cloud
pub fn download_from_cloud(token: &str) -> Result<CloudSyncData, String> {
    let url = format!("{}/api/sync/download", DEFAULT_SERVER_URL);
    
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }
    
    response.json::<CloudSyncData>()
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Delete all data from cloud
pub fn delete_from_cloud(token: &str) -> Result<(), String> {
    let url = format!("{}/api/sync/data", DEFAULT_SERVER_URL);
    
    let client = reqwest::blocking::Client::new();
    let response = client
        .delete(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }
    
    Ok(())
}

// ============================================================================
// Async versions of cloud operations (run in background thread, don't block UI)
// ============================================================================

/// Start async upload operation with progress reporting
#[allow(dead_code)]
pub fn start_upload(token: String, data: CloudSyncData) -> mpsc::Receiver<Result<CloudOpResult, String>> {
    start_upload_with_sizes(token, data, vec![])
}

/// Start async upload operation with progress reporting and install sizes
pub fn start_upload_with_sizes(
    token: String, 
    data: CloudSyncData, 
    install_sizes: Vec<(u64, u64)>
) -> mpsc::Receiver<Result<CloudOpResult, String>> {
    let (tx, rx) = mpsc::channel();
    
    thread::spawn(move || {
        let tx_progress = tx.clone();
        let progress_callback = move |bytes_sent: usize, total_bytes: usize| {
            let _ = tx_progress.send(Ok(CloudOpResult::UploadProgress(UploadProgress {
                bytes_sent,
                total_bytes,
            })));
        };
        
        let result = upload_to_cloud(&token, &data, progress_callback)
            .map(|_| CloudOpResult::UploadSuccess);
        
        // After successful upload, also submit install sizes (best effort, don't fail upload)
        if result.is_ok() && !install_sizes.is_empty() {
            if let Err(e) = submit_size_on_disk(&token, &install_sizes) {
                eprintln!("Failed to submit install sizes: {}", e);
            }
        }
        
        let _ = tx.send(result);
    });
    
    rx
}

/// Start async download operation
pub fn start_download(token: String) -> mpsc::Receiver<Result<CloudOpResult, String>> {
    let (tx, rx) = mpsc::channel();
    
    thread::spawn(move || {
        let result = download_from_cloud(&token)
            .map(CloudOpResult::DownloadSuccess);
        let _ = tx.send(result);
    });
    
    rx
}

/// Start async delete operation
pub fn start_delete(token: String) -> mpsc::Receiver<Result<CloudOpResult, String>> {
    let (tx, rx) = mpsc::channel();
    
    thread::spawn(move || {
        let result = delete_from_cloud(&token)
            .map(|_| CloudOpResult::DeleteSuccess);
        let _ = tx.send(result);
    });
    
    rx
}

/// Start async status check
pub fn start_status_check(token: String) -> mpsc::Receiver<Result<CloudOpResult, String>> {
    let (tx, rx) = mpsc::channel();
    
    thread::spawn(move || {
        let result = check_cloud_status(&token)
            .map(CloudOpResult::StatusChecked);
        let _ = tx.send(result);
    });
    
    rx
}

// ============================================================================
// Achievement Rating API
// ============================================================================

/// Submit an achievement rating to the server (fire-and-forget)
pub fn submit_achievement_rating(token: &str, appid: u64, apiname: &str, rating: u8) {
    let url = format!("{}/api/achievement/rating", DEFAULT_SERVER_URL);
    let token = token.to_string();
    let apiname = apiname.to_string();
    
    // Fire-and-forget in background thread
    thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let body = serde_json::json!({
            "appid": appid,
            "apiname": apiname,
            "rating": rating
        });
        
        match client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
        {
            Ok(resp) if resp.status().is_success() => {
                // Success - rating submitted
            }
            Ok(resp) => {
                eprintln!("Failed to submit rating: HTTP {}", resp.status());
            }
            Err(e) => {
                eprintln!("Failed to submit rating: {}", e);
            }
        }
    });
}

/// Fetch all achievement ratings for the user from the server
pub fn fetch_user_achievement_ratings(token: &str) -> Result<Vec<(u64, String, u8)>, String> {
    let url = format!("{}/api/achievement/ratings", DEFAULT_SERVER_URL);
    
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }
    
    #[derive(serde::Deserialize)]
    struct RatingItem {
        appid: u64,
        apiname: String,
        rating: u8,
    }
    
    #[derive(serde::Deserialize)]
    struct RatingsResponse {
        ratings: Vec<RatingItem>,
    }
    
    let result: RatingsResponse = response.json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    Ok(result.ratings.into_iter().map(|r| (r.appid, r.apiname, r.rating)).collect())
}

// ============================================================================
// Size on Disk Sync
// ============================================================================

/// Submit install sizes to the server
/// This helps build a community database of game install sizes
pub fn submit_size_on_disk(token: &str, sizes: &[(u64, u64)]) -> Result<usize, String> {
    if sizes.is_empty() {
        return Ok(0);
    }
    
    let url = format!("{}/api/size-on-disk", DEFAULT_SERVER_URL);
    
    #[derive(serde::Serialize)]
    struct SizeInfo {
        appid: u64,
        size_bytes: u64,
    }
    
    #[derive(serde::Serialize)]
    struct SubmitRequest {
        sizes: Vec<SizeInfo>,
    }
    
    let request = SubmitRequest {
        sizes: sizes.iter().map(|(appid, size_bytes)| SizeInfo {
            appid: *appid,
            size_bytes: *size_bytes,
        }).collect(),
    };
    
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&request)
        .send()
        .map_err(|e| format!("Network error: {}", e))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }
    
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct SubmitResponse {
        success: bool,
        count: usize,
    }
    
    let result: SubmitResponse = response.json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    Ok(result.count)
}

// ============================================================================
// TTB Blacklist API
// ============================================================================

/// Fetch the TTB blacklist from the server (public, no auth required)
pub fn fetch_ttb_blacklist() -> Result<Vec<u64>, String> {
    let url = format!("{}/api/ttb/blacklist", DEFAULT_SERVER_URL);

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }

    #[derive(serde::Deserialize)]
    struct BlacklistResponse {
        appids: Vec<u64>,
    }

    let result: BlacklistResponse = response.json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result.appids)
}

/// Add a game to the TTB blacklist (admin only)
pub fn add_to_ttb_blacklist(token: &str, appid: u64, game_name: &str, reason: Option<&str>) -> Result<(), String> {
    let url = format!("{}/api/ttb/blacklist", DEFAULT_SERVER_URL);

    let client = reqwest::blocking::Client::new();
    let body = serde_json::json!({
        "appid": appid,
        "game_name": game_name,
        "reason": reason
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&body)
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }

    Ok(())
}

/// Remove a game from the TTB blacklist (admin only)
pub fn remove_from_ttb_blacklist(token: &str, appid: u64) -> Result<(), String> {
    let url = format!("{}/api/ttb/blacklist/{}", DEFAULT_SERVER_URL, appid);

    let client = reqwest::blocking::Client::new();
    let response = client
        .delete(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }

    Ok(())
}

// ============================================================================
// Game Tags API (SteamSpy data)
// ============================================================================

/// Fetch all available tag names from the server
pub fn fetch_tag_names() -> Result<Vec<String>, String> {
    let url = format!("{}/api/tags", DEFAULT_SERVER_URL);

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }

    #[derive(serde::Deserialize)]
    struct TagNamesResponse {
        tags: Vec<String>,
    }

    let result: TagNamesResponse = response.json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result.tags)
}

/// Fetch tags for a batch of games from the server
pub fn fetch_tags_batch(appids: &[u64]) -> Result<Vec<overachiever_core::GameTag>, String> {
    if appids.is_empty() {
        return Ok(vec![]);
    }

    let url = format!("{}/api/tags/batch", DEFAULT_SERVER_URL);

    #[derive(serde::Serialize)]
    struct BatchRequest {
        appids: Vec<u64>,
    }

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&url)
        .json(&BatchRequest { appids: appids.to_vec() })
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }

    #[derive(serde::Deserialize)]
    struct BatchResponse {
        tags: Vec<overachiever_core::GameTag>,
    }

    let result: BatchResponse = response.json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result.tags)
}

/// Submit tags for a game to the server (admin only)
pub fn submit_tags(token: &str, appid: u64, tags: &[(String, u32)]) -> Result<usize, String> {
    let url = format!("{}/api/tags", DEFAULT_SERVER_URL);

    #[derive(serde::Serialize)]
    struct SubmitRequest {
        appid: u64,
        tags: Vec<(String, u32)>,
    }

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&SubmitRequest { appid, tags: tags.to_vec() })
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }

    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct SubmitResponse {
        success: bool,
        count: usize,
    }

    let result: SubmitResponse = response.json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result.count)
}

/// Fetch TTB times for a batch of games from the server
pub fn fetch_ttb_batch(appids: &[u64]) -> Result<Vec<overachiever_core::TtbTimes>, String> {
    if appids.is_empty() {
        return Ok(vec![]);
    }

    let url = format!("{}/api/ttb/batch", DEFAULT_SERVER_URL);

    #[derive(serde::Serialize)]
    struct BatchRequest {
        appids: Vec<u64>,
    }

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&url)
        .json(&BatchRequest { appids: appids.to_vec() })
        .send()
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Server error {}: {}", status, body));
    }

    let times: Vec<overachiever_core::TtbTimes> = response.json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(times)
}
