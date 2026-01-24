//! Keyboard event handlers for each view.
//!
//! This module extracts view-specific keyboard handling logic from `app.rs`
//! to improve maintainability and separation of concerns.

mod debug;
mod index;
mod repomap;
mod search;
mod watch;

pub use debug::DebugHandler;
pub use index::IndexHandler;
pub use repomap::RepoMapHandler;
pub use search::SearchHandler;
pub use watch::WatchHandler;
