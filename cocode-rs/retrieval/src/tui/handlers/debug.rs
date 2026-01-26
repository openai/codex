//! Debug view keyboard handler.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;

use crate::tui::app::App;

/// Debug view keyboard handler trait.
pub trait DebugHandler {
    /// Handle keyboard events in the debug view.
    fn handle_debug_key(&mut self, key: KeyEvent);
}

impl DebugHandler for App {
    fn handle_debug_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::PageUp => {
                self.event_log.scroll_up(10);
            }
            KeyCode::PageDown => {
                self.event_log.scroll_down(10);
            }
            KeyCode::Up => {
                self.event_log.scroll_up(1);
            }
            KeyCode::Down => {
                self.event_log.scroll_down(1);
            }
            KeyCode::Home => {
                self.event_log.scroll_to_top();
            }
            KeyCode::End => {
                self.event_log.scroll_to_bottom();
            }
            KeyCode::Char('c') => {
                // Clear event log
                self.event_log.clear();
            }
            KeyCode::Char('a') => {
                // Toggle auto-scroll
                self.event_log.toggle_auto_scroll();
            }
            _ => {}
        }
    }
}
