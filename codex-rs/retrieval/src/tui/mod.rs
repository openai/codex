//! TUI module for the retrieval system.
//!
//! Provides an interactive terminal user interface for:
//! - Searching code with live results
//! - Managing index builds
//! - Exploring RepoMap rankings
//! - Monitoring file changes
//! - Debugging search pipeline
//!
//! # Architecture
//!
//! The TUI follows the same patterns as `codex-tui2`:
//! - Event-driven state management (`AppEvent`)
//! - View-based organization
//! - Composable widgets
//!
//! # Usage
//!
//! ```ignore
//! use codex_retrieval::tui::run_tui;
//! use codex_retrieval::config::RetrievalConfig;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = RetrievalConfig::load(&std::env::current_dir()?)?;
//!     run_tui(config).await
//! }
//! ```

pub mod app;
pub mod app_event;
pub mod app_state;
pub mod constants;
mod handlers;
mod render;
pub mod terminal;

// View modules
pub mod views;

// Widget modules
pub mod widgets;

// Re-exports
pub use app::App;
pub use app_event::AppEvent;
pub use app_event::ViewMode;
pub use terminal::TerminalGuard;
pub use terminal::Tui;
pub use terminal::init as init_terminal;
pub use terminal::restore as restore_terminal;

use std::sync::Arc;

use crate::config::RetrievalConfig;
use crate::service::RetrievalService;

/// Run the TUI application with the given configuration.
///
/// This is the main entry point for the TUI mode.
///
/// # Arguments
/// * `config` - Retrieval configuration
/// * `service` - Optional pre-initialized RetrievalService. If None, the TUI
///   will be display-only and cannot perform operations.
pub async fn run_tui(
    config: RetrievalConfig,
    service: Option<Arc<RetrievalService>>,
) -> anyhow::Result<()> {
    // Initialize terminal
    let mut terminal = init_terminal()?;

    // Create app with optional service
    let mut app = App::new(config, service);

    // Run the app
    let result = app.run(&mut terminal).await;

    // Restore terminal (always, even on error)
    restore_terminal()?;

    result
}

/// Run the TUI with default configuration.
///
/// Loads configuration from the current directory.
/// Note: This creates a display-only TUI without a service.
/// Use `run_tui()` with a service parameter for full functionality.
pub async fn run_tui_default() -> anyhow::Result<()> {
    let workdir = std::env::current_dir()?;
    let config = RetrievalConfig::load(&workdir)?;
    run_tui(config, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_mode_exports() {
        // Ensure exports work
        let _ = ViewMode::Search;
        let _ = ViewMode::Index;
        let _ = ViewMode::RepoMap;
    }
}
