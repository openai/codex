//! A live status indicator that shows the *latest* log line emitted by the
//! application while the agent is processing a long‑running task.

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Padding;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::shimmer_text::shimmer_spans;

use codex_ansi_escape::ansi_escape_line;

pub(crate) struct StatusIndicatorWidget {
    /// Latest text to display (truncated to the available width at render
    /// time).
    text: String,
    // Keep one sender alive for scheduling frames.
    _app_event_tx: AppEventSender,
}

impl StatusIndicatorWidget {
    /// Create a new status indicator and start the animation timer.
    pub(crate) fn new(app_event_tx: AppEventSender) -> Self {
        Self {
            text: String::from("waiting for logs…"),
            _app_event_tx: app_event_tx,
        }
    }

    pub fn desired_height(&self, _width: u16) -> u16 {
        1
    }

    /// Update the line that is displayed in the widget.
    pub(crate) fn update_text(&mut self, text: String) {
        self.text = text.replace(['\n', '\r'], " ");
    }
}

impl WidgetRef for StatusIndicatorWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Schedule the next animation frame.
        self._app_event_tx
            .send(AppEvent::ScheduleFrameIn(Duration::from_millis(100)));

        let widget_style = Style::default();
        let block = Block::default()
            .padding(Padding::new(1, 0, 0, 0))
            .borders(Borders::LEFT)
            .border_type(BorderType::QuadrantOutside)
            .border_style(widget_style.dim());
        let mut header_spans: Vec<Span<'static>> = shimmer_spans("Working");

        header_spans.push(Span::styled(
            " ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));

        // Ensure we do not overflow width.
        let inner_width = block.inner(area).width as usize;

        // Sanitize and colour‑strip the potentially colourful log text.  This
        // ensures that **no** raw ANSI escape sequences leak into the
        // back‑buffer which would otherwise cause cursor jumps or stray
        // artefacts when the terminal is resized.
        let line = ansi_escape_line(&self.text);
        let mut sanitized_tail: String = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("");

        // Truncate *after* stripping escape codes so width calculation is
        // accurate. See UTF‑8 boundary comments above.
        let header_len: usize = header_spans.iter().map(|s| s.content.len()).sum();

        if header_len + sanitized_tail.len() > inner_width {
            let available_bytes = inner_width.saturating_sub(header_len);

            if sanitized_tail.is_char_boundary(available_bytes) {
                sanitized_tail.truncate(available_bytes);
            } else {
                let mut idx = available_bytes;
                while idx < sanitized_tail.len() && !sanitized_tail.is_char_boundary(idx) {
                    idx += 1;
                }
                sanitized_tail.truncate(idx);
            }
        }

        let mut spans = header_spans;

        // Re‑apply the DIM modifier so the tail appears visually subdued
        // irrespective of the colour information preserved by
        // `ansi_escape_line`.
        spans.push(Span::styled(sanitized_tail, Style::default().dim()));

        let paragraph = Paragraph::new(Line::from(spans))
            .block(block)
            .alignment(Alignment::Left);
        paragraph.render_ref(area, buf);
    }
}
