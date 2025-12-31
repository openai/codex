//! Debug view for the retrieval TUI.
//!
//! Renders the full-screen event log for debugging.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use crate::tui::widgets::EventLog;
use crate::tui::widgets::EventLogState;

/// Debug view widget.
///
/// Displays the full-screen event log for debugging retrieval events.
pub struct DebugView<'a> {
    /// Event log state.
    event_log: &'a EventLogState,
}

impl<'a> DebugView<'a> {
    /// Create a new debug view.
    pub fn new(event_log: &'a EventLogState) -> Self {
        Self { event_log }
    }
}

impl Widget for DebugView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Full-screen event log
        let event_log = EventLog::new(self.event_log);
        event_log.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_view_creation() {
        let event_log = EventLogState::new();
        let _view = DebugView::new(&event_log);
    }
}
