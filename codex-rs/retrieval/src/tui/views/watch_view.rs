//! Watch view for the retrieval TUI.
//!
//! Renders the file watching interface with:
//! - Watch status indicator
//! - File change event log

use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::tui::widgets::EventLog;
use crate::tui::widgets::EventLogState;

/// Watch view widget.
///
/// Displays the file watcher status and file change events.
pub struct WatchView<'a> {
    /// Whether file watching is active.
    watching: bool,
    /// Event log state.
    event_log: &'a EventLogState,
}

impl<'a> WatchView<'a> {
    /// Create a new watch view.
    pub fn new(watching: bool, event_log: &'a EventLogState) -> Self {
        Self {
            watching,
            event_log,
        }
    }
}

impl Widget for WatchView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Status
                Constraint::Min(5),    // Event log
            ])
            .split(area);

        // Status panel
        let status_text = if self.watching {
            vec![
                Line::from(vec![
                    Span::styled("\u{25c9} ", Style::default().green()),
                    Span::styled("Watching", Style::default().green().bold()),
                ]),
                Line::from(""),
                Line::from("Monitoring for file changes..."),
                Line::from(Span::styled(
                    "Press 'w' to stop watching",
                    Style::default().dim(),
                )),
            ]
        } else {
            vec![
                Line::from(vec![
                    Span::styled("\u{25cb} ", Style::default().dim()),
                    Span::styled("Not Watching", Style::default().dim()),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Press 'w' to start watching",
                    Style::default().dim(),
                )),
            ]
        };

        let status = Paragraph::new(status_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Watch Status "),
        );
        status.render(chunks[0], buf);

        // Event log with file change events
        let event_log = EventLog::new(self.event_log);
        event_log.render(chunks[1], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_view_not_watching() {
        let event_log = EventLogState::new();
        let _view = WatchView::new(false, &event_log);
    }

    #[test]
    fn test_watch_view_watching() {
        let event_log = EventLogState::new();
        let _view = WatchView::new(true, &event_log);
    }
}
