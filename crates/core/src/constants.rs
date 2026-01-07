//! Feature flags and constants

/// Enable Time To Beat (HLTB) integration - desktop only
/// Note: This is now controlled by admin_mode at runtime. This constant is kept for reference.
pub const ENABLE_TTB: bool = true;

/// Enable the Admin Mode toggle button in the UI
/// When true, users can toggle admin mode to access TTB scanning and per-game TTB fetching
pub const ENABLE_ADMIN_MODE: bool = true;
