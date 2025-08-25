//! A live status indicator that shows the *latest* log line emitted by the
//! application while the agent is processing a long‑running task.

use std::time::Duration;
use std::time::Instant;

use codex_core::protocol::Op;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::shimmer::shimmer_spans;
use crate::tui::FrameRequester;
use textwrap::Options as TwOptions;
use textwrap::WordSplitter;

pub(crate) struct StatusIndicatorWidget {
    /// Animated header text (defaults to "Working").
    header: String,
    /// Queued user messages to display under the status line.
    queued_messages: Vec<String>,

    start_time: Instant,
    app_event_tx: AppEventSender,
    frame_requester: FrameRequester,
}

impl StatusIndicatorWidget {
    pub(crate) fn new(app_event_tx: AppEventSender, frame_requester: FrameRequester) -> Self {
        Self {
            header: String::from("Working"),
            queued_messages: Vec::new(),
            start_time: Instant::now(),

            app_event_tx,
            frame_requester,
        }
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        // Status line + wrapped queued messages (up to 3 lines per message)
        // + optional ellipsis line per truncated message + 1 spacer line
        let inner_width = width.max(1) as usize;
        let mut total: u16 = 1; // status line
        let text_width = inner_width.saturating_sub(3); // account for " ↳ " prefix
        if text_width > 0 {
            let opts = TwOptions::new(text_width)
                .break_words(false)
                .word_splitter(WordSplitter::NoHyphenation);
            for q in &self.queued_messages {
                let wrapped = textwrap::wrap(q, &opts);
                let lines = wrapped.len().min(3) as u16;
                total = total.saturating_add(lines);
                if wrapped.len() > 3 {
                    total = total.saturating_add(1); // ellipsis line
                }
            }
        } else {
            // At least one line per message if width is extremely narrow
            total = total.saturating_add(self.queued_messages.len() as u16);
        }
        total.saturating_add(1) // spacer line
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

    /// Replace the queued messages displayed beneath the header.
    pub(crate) fn set_queued_messages(&mut self, queued: Vec<String>) {
        self.queued_messages = queued;
        // Ensure a redraw so changes are visible.
        self.frame_requester.schedule_frame();
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
        if area.is_empty() {
            return;
        }

        // Schedule next animation frame.
        self.frame_requester
            .schedule_frame_in(Duration::from_millis(32));
        let elapsed = self.start_time.elapsed().as_secs();

        // Plain rendering: no borders or padding so the live cell is visually indistinguishable from terminal scrollback.
        let mut spans = vec![" ".into()];
        spans.extend(shimmer_spans(&self.header));
        spans.extend(vec![
            " ".into(),
            format!("({elapsed}s • ").dim(),
            "Esc".dim().bold(),
            " to interrupt)".dim(),
        ]);

        // Build lines: status, then queued messages, then spacer.
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(spans));
        // Wrap queued messages using textwrap and show up to the first 3 lines per message.
        let text_width = area.width.saturating_sub(3); // " ↳ " prefix
        let opts = TwOptions::new(text_width as usize)
            .break_words(false)
            .word_splitter(WordSplitter::NoHyphenation);
        for q in &self.queued_messages {
            let wrapped = textwrap::wrap(q, &opts);
            for (i, piece) in wrapped.iter().take(3).enumerate() {
                let prefix = if i == 0 { " ↳ " } else { "   " };
                let content = format!("{prefix}{piece}");
                lines.push(Line::from(content.dim()));
            }
            if wrapped.len() > 3 {
                lines.push(Line::from("   …".dim()));
            }
        }

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
    fn renders_without_left_bar_and_with_margin() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy());

        let area = ratatui::layout::Rect::new(0, 0, 30, 2);
        // Advance animation without sleeping.
        w.test_fast_forward_frames(2);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        w.render_ref(area, &mut buf);

        // Compare the full first-line string (trimmed) to the expected value.
        let mut row0 = String::new();
        for x in 0..area.width {
            row0.push(buf[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        let row0 = row0.trim_end();
        // Width is 30, so the rendered line truncates before the long
        // " to interrupt)" tail and before the log text. Expect this prefix:
        assert_eq!(row0, " Working (0s • Esc");
        // Second line is a blank spacer
        let mut r1 = String::new();
        for x in 0..area.width {
            r1.push(buf[(x, 1)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(
            r1.trim().is_empty(),
            "expected blank spacer line below status: {r1:?}"
        );
    }

    #[test]
    fn working_header_is_present_on_last_line() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy());
        // Advance animation without sleeping.
        w.test_fast_forward_frames(2);

        let area = ratatui::layout::Rect::new(0, 0, 30, 2);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        w.render_ref(area, &mut buf);

        // First line should contain the animated "Working" header.
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
        w.test_fast_forward_frames(2);

        let area = ratatui::layout::Rect::new(0, 0, 30, 2);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        w.render_ref(area, &mut buf);

        // Check the entire rendered first line matches the expected prefix.
        let mut row0 = String::new();
        for x in 0..area.width {
            row0.push(buf[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        let row0 = row0.trim_end();
        assert_eq!(row0, " Working (0s • Esc");
    }
}
