//! Event types for LSP Test TUI.

use super::app::LspResult;
use crossterm::event::KeyEvent;

/// TUI events
#[derive(Debug)]
pub enum Event {
    /// Keyboard input
    Key(KeyEvent),
    /// Terminal resize
    #[allow(dead_code)]
    Resize(u16, u16),
    /// Tick for periodic updates
    Tick,
    /// LSP operation completed
    LspResult(LspResult),
}
