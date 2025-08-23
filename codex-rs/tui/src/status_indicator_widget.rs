//! A live status indicator that shows the *latest* log line emitted by the
//! application while the agent is processing a long‑running task.

use std::time::Duration;
use std::time::Instant;

use codex_core::protocol::Op;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use unicode_width::UnicodeWidthStr;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::shimmer::shimmer_spans;
use crate::tui::FrameRequester;

// We render the live text using markdown so it visually matches the history
// cells. Before rendering we strip any ANSI escape sequences to avoid writing
// raw control bytes into the back buffer.
use codex_ansi_escape::ansi_escape_line;

pub(crate) struct StatusIndicatorWidget {
    /// Latest text to display (truncated to the available width at render
    /// time).
    text: String,
    /// Animated header text (defaults to "Working").
    header: String,

    /// Animation state: reveal target `text` progressively like a typewriter.
    /// We compute the currently visible prefix length based on the current
    /// frame index and a constant typing speed.  The `base_frame` and
    /// `reveal_len_at_base` form the anchor from which we advance.
    last_target_len: usize,
    base_frame: usize,
    reveal_len_at_base: usize,
    start_time: Instant,
    app_event_tx: AppEventSender,
    frame_requester: FrameRequester,
}

impl StatusIndicatorWidget {
    pub(crate) fn new(app_event_tx: AppEventSender, frame_requester: FrameRequester) -> Self {
        Self {
            text: String::from("waiting for model"),
            header: String::from("Working"),
            last_target_len: 0,
            base_frame: 0,
            reveal_len_at_base: 0,
            start_time: Instant::now(),

            app_event_tx,
            frame_requester,
        }
    }

    pub fn desired_height(&self, _width: u16) -> u16 {
        1
    }

    /// Update the line that is displayed in the widget.
    pub(crate) fn update_text(&mut self, text: String) {
        // If the text hasn't changed, don't reset the baseline; let the
        // animation continue advancing naturally.
        if text == self.text {
            return;
        }
        // Update the target text, preserving newlines so wrapping matches history cells.
        // Strip ANSI escapes for the character count so the typewriter animation speed is stable.
        let stripped = {
            let line = ansi_escape_line(&text);
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<Vec<_>>()
                .join("")
        };
        let new_len = stripped.chars().count();

        // Compute how many characters are currently revealed so we can carry
        // this forward as the new baseline when target text changes.
        let current_frame = self.current_frame();
        let shown_now = self.current_shown_len(current_frame);

        self.text = text;
        self.last_target_len = new_len;
        self.base_frame = current_frame;
        self.reveal_len_at_base = shown_now.min(new_len);
    }

    pub(crate) fn interrupt(&self) {
        self.app_event_tx.send(AppEvent::CodexOp(Op::Interrupt));
    }

    /// Update the animated header label (left of the brackets).
    pub(crate) fn update_header(&mut self, header: String) {
        if self.header != header {
            self.header = header;
        }
    }

    /// Reset the animation and start revealing `text` from the beginning.
    #[cfg(test)]
    pub(crate) fn restart_with_text(&mut self, text: String) {
        let sanitized = text.replace(['\n', '\r'], " ");
        let stripped = {
            let line = ansi_escape_line(&sanitized);
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<Vec<_>>()
                .join("")
        };

        let new_len = stripped.chars().count();
        let current_frame = self.current_frame();

        self.text = sanitized;
        self.last_target_len = new_len;
        self.base_frame = current_frame;
        // Start from zero revealed characters for a fresh typewriter cycle.
        self.reveal_len_at_base = 0;
    }

    /// Calculate how many characters should currently be visible given the
    /// animation baseline and frame counter.
    fn current_shown_len(&self, current_frame: usize) -> usize {
        // Increase typewriter speed (~5x): reveal more characters per frame.
        const TYPING_CHARS_PER_FRAME: usize = 7;
        let frames = current_frame.saturating_sub(self.base_frame);
        let advanced = self
            .reveal_len_at_base
            .saturating_add(frames.saturating_mul(TYPING_CHARS_PER_FRAME));
        advanced.min(self.last_target_len)
    }

    fn current_frame(&self) -> usize {
        // Derive frame index from wall-clock time. 100ms per frame to match
        // the previous ticker cadence.
        let since_start = self.start_time.elapsed();
        (since_start.as_millis() / 100) as usize
    }

    /// Test-only helper to fast-forward the internal clock so animations
    /// advance without sleeping.
    #[cfg(test)]
    pub(crate) fn test_fast_forward_frames(&mut self, frames: usize) {
        let advance_ms = (frames as u64).saturating_mul(100);
        // Move the start time into the past so `current_frame()` advances.
        self.start_time = std::time::Instant::now() - std::time::Duration::from_millis(advance_ms);
    }
}

impl WidgetRef for StatusIndicatorWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Ensure minimal height
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Schedule next animation frame.
        self.frame_requester
            .schedule_frame_in(Duration::from_millis(32));
        let idx = self.current_frame();
        let elapsed = self.start_time.elapsed().as_secs();
        let shown_now = self.current_shown_len(idx);
        let status_prefix: String = self.text.chars().take(shown_now).collect();
        let animated_spans = shimmer_spans(&self.header);

        // Plain rendering: no borders or padding so the live cell is visually indistinguishable from terminal scrollback.
        let inner_width = area.width as usize;

        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::styled("▌ ", Style::default().fg(Color::Cyan)));

        // Animated header after the left bar
        spans.extend(animated_spans);
        // Space between header and bracket block
        spans.push(Span::raw(" "));
        // Non-animated, dim bracket content, with keys bold
        let bracket_prefix = format!("({elapsed}s • ");
        spans.push(Span::styled(
            bracket_prefix,
            Style::default().add_modifier(Modifier::DIM),
        ));
        spans.push(Span::styled(
            "Esc",
            Style::default().add_modifier(Modifier::DIM | Modifier::BOLD),
        ));
        spans.push(Span::styled(
            " to interrupt)",
            Style::default().add_modifier(Modifier::DIM),
        ));
        // Add a space and then the log text (not animated by the gradient)
        if !status_prefix.is_empty() {
            spans.push(Span::styled(
                " ",
                Style::default().add_modifier(Modifier::DIM),
            ));
            spans.push(Span::styled(
                status_prefix,
                Style::default().add_modifier(Modifier::DIM),
            ));
        }

        // Truncate spans to fit the width.
        let mut acc: Vec<Span<'static>> = Vec::new();
        let mut used = 0usize;
        for s in spans {
            let w = s.content.width();
            if used + w <= inner_width {
                acc.push(s);
                used += w;
            } else {
                break;
            }
        }
        let lines = vec![Line::from(acc)];

        // No-op once full text is revealed; the app no longer reacts to a completion event.

        let paragraph = Paragraph::new(lines);
        paragraph.render_ref(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn renders_without_left_border_or_padding() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy());
        w.restart_with_text("Hello".to_string());

        let area = ratatui::layout::Rect::new(0, 0, 30, 1);
        // Advance animation without sleeping.
        w.test_fast_forward_frames(2);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        w.render_ref(area, &mut buf);

        // Leftmost column has the left bar
        let ch0 = buf[(0, 0)].symbol().chars().next().unwrap_or(' ');
        assert_eq!(ch0, '▌', "expected left bar at col 0: {ch0:?}");
    }

    #[test]
    fn working_header_is_present_on_last_line() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy());
        w.restart_with_text("Hi".to_string());
        // Advance animation without sleeping.
        w.test_fast_forward_frames(2);

        let area = ratatui::layout::Rect::new(0, 0, 30, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        w.render_ref(area, &mut buf);

        // Single line; it should contain the animated "Working" header.
        let mut row = String::new();
        for x in 0..area.width {
            row.push(buf[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(row.contains("Working"), "expected Working header: {row:?}");
    }

    #[test]
    fn header_starts_at_expected_position() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy());
        w.restart_with_text("Hello".to_string());
        w.test_fast_forward_frames(2);

        let area = ratatui::layout::Rect::new(0, 0, 30, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        w.render_ref(area, &mut buf);

        let ch = buf[(2, 0)].symbol().chars().next().unwrap_or(' ');
        assert_eq!(ch, 'W', "expected Working header at col 2: {ch:?}");
    }
}
