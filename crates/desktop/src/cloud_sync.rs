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
    Uploading,
    Downloading,
    Deleting,
    Success(String),
    Error(String),
}

/// Result from the Steam login callback
#[derive(Debug, Clone)]
pub struct AuthResult {
    pub token: String,
    #[allow(dead_code)]
    pub steam_id: String,
}

/// Result from async cloud operations
#[derive(Debug, Clone)]
pub enum CloudOpResult {
    UploadSuccess,
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
                    Ok(_) => ("200 OK", "<html><body><h1>✓ Linked to Cloud!</h1><p>You can close this window and return to Overachiever.</p><script>window.close()</script></body></html>"),
                    Err(e) => ("400 Bad Request", &format!("<html><body><h1>✗ Login Failed</h1><p>{}</p></body></html>", e) as &str),
                };
                
                let response = format!(
                    "HTTP/1.1 {}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
pub fn upload_to_cloud(token: &str, data: &CloudSyncData) -> Result<(), String> {
    use std::error::Error;
    
    let url = format!("{}/api/sync/upload", DEFAULT_SERVER_URL);
    
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120)) // 2 minute timeout for uploads
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(data)
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

/// Start async upload operation
pub fn start_upload(token: String, data: CloudSyncData) -> mpsc::Receiver<Result<CloudOpResult, String>> {
    let (tx, rx) = mpsc::channel();
    
    thread::spawn(move || {
        let result = upload_to_cloud(&token, &data)
            .map(|_| CloudOpResult::UploadSuccess);
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
