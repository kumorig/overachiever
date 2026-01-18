//! REST API routes

pub mod auth;
pub mod games;
pub mod achievements;
pub mod ratings;
pub mod cloud_sync;
pub mod size_cache;
pub mod users;
pub mod ttb;
pub mod tags;

// Re-export all route handlers
pub use games::*;
pub use achievements::*;
pub use ratings::*;
pub use cloud_sync::*;
pub use size_cache::*;
pub use users::*;
pub use ttb::*;
pub use tags::*;
