//! A live status indicator that shows the *latest* log line emitted by the
//! application while the agent is processing a long‑running task.
//!
//! It replaces the old spinner animation with real log feedback so users can
//! watch Codex “think” in real‑time. Whenever new text is provided via
//! [`StatusIndicatorWidget::update_text`], the parent widget triggers a
//! redraw so the change is visible immediately.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use crossterm::event::KeyEvent;
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

use codex_ansi_escape::ansi_escape_line;

pub(crate) struct StatusIndicatorWidget {
    /// Latest text to display (truncated to the available width at render
    /// time).
    text: String,

    /// Height in terminal rows – matches the height of the textarea at the
    /// moment the task started so the UI does not jump when we toggle between
    /// input mode and loading mode.
    height: u16,

    frame_idx: std::sync::Arc<AtomicUsize>,
    running: std::sync::Arc<AtomicBool>,
    // Keep one sender alive to prevent the channel from closing while the
    // animation thread is still running. The field itself is currently not
    // accessed anywhere, therefore the leading underscore silences the
    // `dead_code` warning without affecting behavior.
    _app_event_tx: Sender<AppEvent>,
}

impl StatusIndicatorWidget {
    /// Create a new status indicator and start the animation timer.
    pub(crate) fn new(app_event_tx: Sender<AppEvent>, height: u16) -> Self {
        let frame_idx = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicBool::new(true));

        // Animation thread.
        {
            let frame_idx_clone = Arc::clone(&frame_idx);
            let running_clone = Arc::clone(&running);
            let app_event_tx_clone = app_event_tx.clone();
            thread::spawn(move || {
                let mut counter = 0usize;
                while running_clone.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(200));
                    counter = counter.wrapping_add(1);
                    frame_idx_clone.store(counter, Ordering::Relaxed);
                    if app_event_tx_clone.send(AppEvent::Redraw).is_err() {
                        break;
                    }
                }
            });
        }

        Self {
            text: String::from("waiting for logs…"),
            height: height.max(3),
            frame_idx,
            running,
            _app_event_tx: app_event_tx,
        }
    }

    pub(crate) fn handle_key_event(
        &mut self,
        _key: KeyEvent,
    ) -> Result<bool, std::sync::mpsc::SendError<AppEvent>> {
        // The indicator does not handle any input – always return `false`.
        Ok(false)
    }

    /// Preferred height in terminal rows.
    pub(crate) fn get_height(&self) -> u16 {
        self.height
    }

    /// Update the line that is displayed in the widget.
    pub(crate) fn update_text(&mut self, text: String) {
        self.text = text.replace(['\n', '\r'], " ");
    }
}

impl Drop for StatusIndicatorWidget {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        self.running.store(false, Ordering::Relaxed);
    }
}

impl WidgetRef for StatusIndicatorWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let widget_style = Style::default();
        let block = Block::default()
            .padding(Padding::new(1, 0, 0, 0))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(widget_style);
        // Animated 3‑dot pattern inside brackets. The *active* dot is bold
        // white, the others are dim.
        const DOT_COUNT: usize = 3;
        let idx = self.frame_idx.load(std::sync::atomic::Ordering::Relaxed);
        let phase = idx % (DOT_COUNT * 2 - 2);
        let active = if phase < DOT_COUNT {
            phase
        } else {
            (DOT_COUNT * 2 - 2) - phase
        };

        let mut header_spans: Vec<Span<'static>> = Vec::new();

        header_spans.push(Span::styled(
            "Working ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));

        header_spans.push(Span::styled(
            "[",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));

        for i in 0..DOT_COUNT {
            let style = if i == active {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().dim()
            };
            header_spans.push(Span::styled(".", style));
        }

        header_spans.push(Span::styled(
            "] ",
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
