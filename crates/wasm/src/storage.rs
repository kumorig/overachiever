//! Browser storage helpers for tokens and URL parsing

use overachiever_core::GdprConsent;

// ============================================================================
// Token Management
// ============================================================================

pub fn get_token_from_url() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|search| {
            search.strip_prefix('?')
                .and_then(|s| {
                    s.split('&')
                        .find(|p| p.starts_with("token="))
                        .map(|p| p.strip_prefix("token=").unwrap_or("").to_string())
                })
        })
        .filter(|t| !t.is_empty())
}

/// Get short_id from URL path (e.g., /IHh1wBke -> Some("IHh1wBke"))
/// Returns None for root path or paths that don't look like short_ids
pub fn get_short_id_from_url() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .and_then(|path| {
            // Strip leading slash and get the first path segment
            let path = path.strip_prefix('/').unwrap_or(&path);
            
            // Ignore known paths that aren't short_ids
            if path.is_empty() || path.starts_with("auth") || path.starts_with("api") 
               || path.starts_with("ws") || path.starts_with("pkg") || path.starts_with("steam-media") {
                return None;
            }
            
            // Short IDs are 8 characters, alphanumeric
            let short_id = path.split('/').next().unwrap_or("");
            if short_id.len() == 8 && short_id.chars().all(|c| c.is_ascii_alphanumeric()) {
                Some(short_id.to_string())
            } else {
                None
            }
        })
}

pub fn get_token_from_storage() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|storage| storage.get_item("overachiever_token").ok())
        .flatten()
}

pub fn save_token_to_storage(token: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("overachiever_token", token);
    }
}

pub fn clear_token_from_storage() {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.remove_item("overachiever_token");
    }
}

// ============================================================================
// GDPR Consent Storage
// ============================================================================

pub fn get_gdpr_consent_from_storage() -> GdprConsent {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|storage| storage.get_item("overachiever_gdpr_consent").ok())
        .flatten()
        .map(|s| match s.as_str() {
            "accepted" => GdprConsent::Accepted,
            "declined" => GdprConsent::Declined,
            _ => GdprConsent::Unset,
        })
        .unwrap_or(GdprConsent::Unset)
}

pub fn save_gdpr_consent_to_storage(consent: GdprConsent) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let value = match consent {
            GdprConsent::Accepted => "accepted",
            GdprConsent::Declined => "declined",
            GdprConsent::Unset => "unset",
        };
        let _ = storage.set_item("overachiever_gdpr_consent", value);
    }
}

pub fn clear_gdpr_consent_from_storage() {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.remove_item("overachiever_gdpr_consent");
    }
}

// ============================================================================
// URL Helpers
// ============================================================================

pub fn get_ws_url_from_location() -> String {
    web_sys::window()
        .and_then(|w| {
            let location = w.location();
            let protocol = location.protocol().ok()?;
            let host = location.host().ok()?;
            let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };
            Some(format!("{}//{}/ws", ws_protocol, host))
        })
        .unwrap_or_else(|| "wss://overachiever.space/ws".to_string())
}

pub fn get_auth_url() -> String {
    web_sys::window()
        .and_then(|w| {
            let location = w.location();
            let origin = location.origin().ok()?;
            Some(format!("{}/auth/steam", origin))
        })
        .unwrap_or_else(|| "/auth/steam".to_string())
}

// ============================================================================
// Tag Caching
// ============================================================================

const TAG_CACHE_KEY: &str = "overachiever_available_tags";
const TAG_CACHE_VERSION_KEY: &str = "overachiever_tag_cache_version";
const TAG_CACHE_VERSION: &str = "1"; // Increment to invalidate cache

/// Get cached available tags from localStorage
pub fn get_cached_tags() -> Option<Vec<String>> {
    let storage = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()?;
    
    // Check cache version
    let cached_version = storage.get_item(TAG_CACHE_VERSION_KEY).ok().flatten();
    if cached_version.as_deref() != Some(TAG_CACHE_VERSION) {
        // Cache is outdated, clear it
        let _ = storage.remove_item(TAG_CACHE_KEY);
        let _ = storage.remove_item(TAG_CACHE_VERSION_KEY);
        return None;
    }
    
    // Get cached tags
    let json = storage.get_item(TAG_CACHE_KEY).ok().flatten()?;
    serde_json::from_str(&json).ok()
}

/// Save available tags to localStorage
pub fn save_tags_to_cache(tags: &[String]) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        if let Ok(json) = serde_json::to_string(tags) {
            let _ = storage.set_item(TAG_CACHE_KEY, &json);
            let _ = storage.set_item(TAG_CACHE_VERSION_KEY, TAG_CACHE_VERSION);
        }
    }
}

/// Clear cached tags (useful when forcing a refresh)
#[allow(dead_code)]
pub fn clear_cached_tags() {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.remove_item(TAG_CACHE_KEY);
        let _ = storage.remove_item(TAG_CACHE_VERSION_KEY);
    }
}
