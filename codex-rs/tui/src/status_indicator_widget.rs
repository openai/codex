//! A live status indicator that shows the *latest* log line emitted by the
//! application while the agent is processing a long‑running task.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;
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

// We render the live text using markdown so it visually matches the history
// cells. Before rendering we strip any ANSI escape sequences to avoid writing
// raw control bytes into the back buffer.
use codex_ansi_escape::ansi_escape_line;

pub(crate) struct StatusIndicatorWidget {
    /// Latest text to display (truncated to the available width at render
    /// time).
    text: String,

    /// Animation state: reveal target `text` progressively like a typewriter.
    /// We compute the currently visible prefix length based on the current
    /// frame index and a constant typing speed.  The `base_frame` and
    /// `reveal_len_at_base` form the anchor from which we advance.
    last_target_len: usize,
    base_frame: usize,
    reveal_len_at_base: usize,

    frame_idx: Arc<AtomicUsize>,
    running: Arc<AtomicBool>,
    start_time: Instant,
    app_event_tx: AppEventSender,
}

impl StatusIndicatorWidget {
    /// Create a new status indicator and start the animation timer.
    pub(crate) fn new(app_event_tx: AppEventSender) -> Self {
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
                    std::thread::sleep(Duration::from_millis(100));
                    counter = counter.wrapping_add(1);
                    frame_idx_clone.store(counter, Ordering::Relaxed);
                    app_event_tx_clone.send(AppEvent::RequestRedraw);
                }
            });
        }

        Self {
            text: String::from("waiting for model"),
            last_target_len: 0,
            base_frame: 0,
            reveal_len_at_base: 0,
            frame_idx,
            running,
            start_time: Instant::now(),

            app_event_tx,
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
        let current_frame = self.frame_idx.load(std::sync::atomic::Ordering::Relaxed);
        let shown_now = self.current_shown_len(current_frame);

        self.text = text;
        self.last_target_len = new_len;
        self.base_frame = current_frame;
        self.reveal_len_at_base = shown_now.min(new_len);
    }

    pub(crate) fn interrupt(&self) {
        self.app_event_tx.send(AppEvent::CodexOp(Op::Interrupt));
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
        let current_frame = self.frame_idx.load(std::sync::atomic::Ordering::Relaxed);

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
}

impl Drop for StatusIndicatorWidget {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        self.running.store(false, Ordering::Relaxed);
    }
}

impl WidgetRef for StatusIndicatorWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Ensure minimal height
        if area.height == 0 || area.width == 0 {
            return;
        }

        let idx = self.frame_idx.load(std::sync::atomic::Ordering::Relaxed);
        let elapsed = self.start_time.elapsed().as_secs();
        let shown_now = self.current_shown_len(idx);
        let status_prefix: String = self.text.chars().take(shown_now).collect();
        let animated_text = "Working";
        let header_chars: Vec<char> = animated_text.chars().collect();
        let padding = 4usize; // virtual padding around the animated segment for smoother loop
        let period = header_chars.len() + padding * 2;
        let pos = idx % period;
        let has_true_color = supports_color::on_cached(supports_color::Stream::Stdout)
            .map(|level| level.has_16m)
            .unwrap_or(false);
        let band_half_width = 2.0; // width of the bright band in characters

        let mut animated_spans: Vec<Span<'static>> = Vec::new();
        for (i, ch) in header_chars.iter().enumerate() {
            let i_pos = i as isize + padding as isize;
            let pos = pos as isize;
            let dist = (i_pos - pos).abs() as f32;

            let t = if dist <= band_half_width {
                let x = std::f32::consts::PI * (dist / band_half_width);
                0.5 * (1.0 + x.cos())
            } else {
                0.0
            };

            let brightness = 0.4 + 0.6 * t;
            let level = (brightness * 255.0).clamp(0.0, 255.0) as u8;
            let style = if has_true_color {
                Style::default()
                    .fg(Color::Rgb(level, level, level))
                    .add_modifier(Modifier::BOLD)
            } else {
                color_for_level(level)
            };

            animated_spans.push(Span::styled(ch.to_string(), style));
        }

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

fn color_for_level(level: u8) -> Style {
    if level < 144 {
        Style::default().add_modifier(Modifier::DIM)
    } else if level < 208 {
        Style::default()
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use std::sync::mpsc::channel;

    #[test]
    fn renders_without_left_border_or_padding() {
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx);
        w.restart_with_text("Hello".to_string());

        let area = ratatui::layout::Rect::new(0, 0, 30, 1);
        // Allow a short delay so the typewriter reveals the first character.
        std::thread::sleep(std::time::Duration::from_millis(120));
        let mut buf = ratatui::buffer::Buffer::empty(area);
        w.render_ref(area, &mut buf);

        // Leftmost column has the left bar
        let ch0 = buf[(0, 0)].symbol().chars().next().unwrap_or(' ');
        assert_eq!(ch0, '▌', "expected left bar at col 0: {ch0:?}");
    }

    #[test]
    fn working_header_is_present_on_last_line() {
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx);
        w.restart_with_text("Hi".to_string());
        // Ensure some frames elapse so we get a stable state.
        std::thread::sleep(std::time::Duration::from_millis(120));

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
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx);
        w.restart_with_text("Hello".to_string());
        std::thread::sleep(std::time::Duration::from_millis(120));

        let area = ratatui::layout::Rect::new(0, 0, 30, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        w.render_ref(area, &mut buf);

        let ch = buf[(2, 0)].symbol().chars().next().unwrap_or(' ');
        assert_eq!(ch, 'W', "expected Working header at col 2: {ch:?}");
    }
}
