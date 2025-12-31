//! Index view for the retrieval TUI.
//!
//! Renders the index management interface with:
//! - Statistics panel
//! - Build progress bar
//! - Event log

use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use crate::tui::app::IndexState;
use crate::tui::widgets::EventLog;
use crate::tui::widgets::EventLogState;
use crate::tui::widgets::ProgressBar;
use crate::tui::widgets::StatsPanel;

/// Index view widget.
///
/// Displays index statistics, build progress, and event log.
pub struct IndexView<'a> {
    /// Index state (progress, stats).
    index: &'a IndexState,
    /// Event log state.
    event_log: &'a EventLogState,
}

impl<'a> IndexView<'a> {
    /// Create a new index view.
    pub fn new(index: &'a IndexState, event_log: &'a EventLogState) -> Self {
        Self { index, event_log }
    }
}

impl Widget for IndexView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6), // Stats
                Constraint::Length(6), // Progress
                Constraint::Min(5),    // Event log
            ])
            .split(area);

        // Stats panel
        let stats_panel = StatsPanel::new(&self.index.stats);
        stats_panel.render(chunks[0], buf);

        // Progress bar
        let progress_bar = ProgressBar::new(&self.index.progress).title("Build Progress");
        progress_bar.render(chunks[1], buf);

        // Event log
        let event_log = EventLog::new(self.event_log);
        event_log.render(chunks[2], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_view_creation() {
        let index = IndexState::default();
        let event_log = EventLogState::new();
        let _view = IndexView::new(&index, &event_log);
    }
}
