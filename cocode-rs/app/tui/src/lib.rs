//! Terminal UI for cocode.
//!
//! This crate provides a terminal-based user interface using ratatui and crossterm.
//! It follows The Elm Architecture (TEA) pattern with async event handling.
//!
//! ## Architecture
//!
//! - **Model**: Application state ([`state::AppState`])
//! - **Message**: Events that trigger state changes ([`event::TuiEvent`])
//! - **Update**: Pure functions that update state based on messages ([`update`])
//! - **View**: Renders state to terminal using ratatui widgets ([`render`])
//!
//! ## Key Features
//!
//! - Async event handling with tokio
//! - Real-time streaming content display
//! - Tool execution visualization
//! - Permission prompt overlays
//! - Keyboard shortcuts (Tab: plan mode, Ctrl+T: thinking, Ctrl+M: model)
//!
//! ## Example
//!
//! ```ignore
//! use cocode_tui::{App, AppConfig, create_channels};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let (agent_tx, agent_rx, command_tx, command_rx) = create_channels(32);
//!     let config = AppConfig::default();
//!     let mut app = App::new(agent_rx, command_tx, config)?;
//!     app.run().await
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::unwrap_used)]

pub mod app;
pub mod command;
pub mod event;
pub mod render;
pub mod state;
pub mod terminal;
pub mod update;
pub mod widgets;

// Re-export commonly used types
pub use app::App;
pub use app::AppConfig;
pub use app::create_channels;
pub use command::UserCommand;
pub use event::TuiCommand;
pub use event::TuiEvent;
pub use state::AppState;
pub use state::FocusTarget;
pub use state::Overlay;
pub use state::RunningState;
pub use terminal::Tui;
pub use terminal::restore_terminal;
pub use terminal::setup_terminal;

#[cfg(test)]
mod tests {
    #[test]
    fn test_crate_compiles() {
        // Basic smoke test to ensure the crate compiles
        assert!(true);
    }
}
