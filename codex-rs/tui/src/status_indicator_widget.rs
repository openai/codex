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
            if !self.queued_messages.is_empty() {
                total = total.saturating_add(1); // keybind hint line
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
                lines.push(Line::from(content.dim().italic()));
            }
            if wrapped.len() > 3 {
                lines.push(Line::from("   …".dim().italic()));
            }
        }
        if !self.queued_messages.is_empty() {
            lines.push(Line::from(vec!["   ".into(), "Alt+↑".cyan(), " edit".into()]).dim());
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
    use insta::assert_snapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn renders_with_working_header() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy());

        // Render into a fixed-size test terminal and snapshot the backend.
        let mut terminal = Terminal::new(TestBackend::new(80, 2)).expect("terminal");
        terminal
            .draw(|f| w.render_ref(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn renders_truncated() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy());

        // Render into a fixed-size test terminal and snapshot the backend.
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).expect("terminal");
        terminal
            .draw(|f| w.render_ref(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn renders_with_queued_messages() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy());
        w.set_queued_messages(vec!["first".to_string(), "second".to_string()]);

        // Render into a fixed-size test terminal and snapshot the backend.
        let mut terminal = Terminal::new(TestBackend::new(80, 8)).expect("terminal");
        terminal
            .draw(|f| w.render_ref(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(terminal.backend());
    }
}
