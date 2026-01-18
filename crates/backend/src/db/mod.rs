//! Database operations for the backend using tokio-postgres

mod error;
mod users;
mod games;
mod achievements;
mod history;
mod ratings;
mod cloud_sync;
mod size_cache;
mod ttb;
mod tags;
mod logging;

// Re-export everything
pub use error::*;
pub use users::*;
pub use games::*;
pub use achievements::*;
pub use history::*;
pub use ratings::*;
pub use cloud_sync::*;
pub use size_cache::*;
pub use ttb::*;
pub use tags::*;
pub use logging::*;
