//! Search view for the retrieval TUI.
//!
//! Renders the search interface with:
//! - Query input with mode selector
//! - Search pipeline visualization
//! - Search results list
//! - Event log

use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use crate::tui::app::SearchState;
use crate::tui::widgets::EventLog;
use crate::tui::widgets::EventLogState;
use crate::tui::widgets::ResultList;
use crate::tui::widgets::SearchInput;
use crate::tui::widgets::SearchPipeline;

/// Search view widget.
///
/// Displays the search interface with input, pipeline, results, and event log.
pub struct SearchView<'a> {
    /// Search state (input, results, pipeline).
    search: &'a SearchState,
    /// Event log state.
    event_log: &'a EventLogState,
}

impl<'a> SearchView<'a> {
    /// Create a new search view.
    pub fn new(search: &'a SearchState, event_log: &'a EventLogState) -> Self {
        Self { search, event_log }
    }
}

impl Widget for SearchView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),  // Query input (mode selector + input)
                Constraint::Length(10), // Search pipeline
                Constraint::Min(5),     // Results
                Constraint::Length(8),  // Event log (increased from 6)
            ])
            .split(area);

        // Query input widget
        let search_input = SearchInput::new(&self.search.input);
        search_input.render(chunks[0], buf);

        // Search pipeline widget
        let pipeline = SearchPipeline::new(&self.search.pipeline);
        pipeline.render(chunks[1], buf);

        // Results widget
        let result_list = ResultList::new(&self.search.results);
        result_list.render(chunks[2], buf);

        // Event log widget
        let event_log = EventLog::new(self.event_log);
        event_log.render(chunks[3], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_view_creation() {
        let search = SearchState::default();
        let event_log = EventLogState::new();
        let _view = SearchView::new(&search, &event_log);
    }
}
