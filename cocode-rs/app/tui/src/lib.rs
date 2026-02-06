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

// Initialize i18n at crate root - this generates the _rust_i18n_t function
rust_i18n::i18n!("locales", fallback = "en");

pub mod app;
pub mod clipboard_paste;
pub mod command;
pub mod editor;
pub mod event;
pub mod file_search;
pub mod i18n;
pub mod paste;
pub mod render;
pub mod skill_search;
pub mod state;
pub mod terminal;
pub mod theme;
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

// File search for autocomplete
pub use file_search::FileSearchEvent;
pub use file_search::FileSearchManager;
pub use file_search::create_file_search_channel;

// Skill search for slash command autocomplete
pub use skill_search::SkillInfo;
pub use skill_search::SkillSearchManager;

// External editor support
pub use editor::edit_in_external_editor;

// Theme system
pub use theme::Theme;
pub use theme::ThemeName;

// Paste management
pub use paste::PasteEntry;
pub use paste::PasteKind;
pub use paste::PasteManager;

// Re-export correlation types from protocol for convenience
pub use cocode_protocol::AgentStatus;
pub use cocode_protocol::CorrelatedEvent;
pub use cocode_protocol::SubmissionId;

#[cfg(test)]
mod tests {
    #[test]
    fn test_crate_compiles() {
        // Basic smoke test to ensure the crate compiles
        assert!(true);
    }
}
