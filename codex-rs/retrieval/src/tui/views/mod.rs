//! View implementations for each tab.
//!
//! Each view handles its own rendering and is responsible for layout
//! of its constituent widgets.

mod debug_view;
mod index_view;
mod repomap_view;
mod search_view;
mod watch_view;

pub use debug_view::DebugView;
pub use index_view::IndexView;
pub use repomap_view::RepoMapView;
pub use search_view::SearchView;
pub use watch_view::WatchView;
