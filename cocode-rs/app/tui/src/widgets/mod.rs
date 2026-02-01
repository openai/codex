//! TUI widgets.
//!
//! This module provides the main UI components:
//! - [`StatusBar`]: Displays model, thinking level, plan mode, and token usage
//! - [`ChatWidget`]: Displays the conversation history
//! - [`InputWidget`]: Multi-line input field
//! - [`ToolPanel`]: Shows tool execution progress

mod chat;
mod input;
mod status_bar;
mod tool_panel;

pub use chat::ChatWidget;
pub use input::InputWidget;
pub use status_bar::StatusBar;
pub use tool_panel::ToolPanel;
