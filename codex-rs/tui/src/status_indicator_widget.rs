//! A live status indicator that shows the *latest* log line emitted by the
//! application while the agent is processing a long‑running task.

use std::time::Duration;
use std::time::Instant;

use codex_core::protocol::Op;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use unicode_width::UnicodeWidthStr;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::exec_cell::spinner;
use crate::key_hint;
use crate::render::renderable::Renderable;
use crate::shimmer::shimmer_spans;
use crate::text_formatting::capitalize_first;
use crate::tui::FrameRequester;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

const DETAILS_MAX_LINES: usize = 3;
const DETAILS_PREFIX: &str = "  └ ";

#[derive(Debug, Clone)]
pub(crate) struct StatusSnapshot {
    pub(crate) header: String,
    pub(crate) progress: Option<f32>,
    pub(crate) thinking: Vec<String>,
    pub(crate) tool_calls: Vec<String>,
    pub(crate) logs: Vec<String>,
}

pub(crate) struct StatusIndicatorWidget {
    /// Animated header text (defaults to "Working").
    header: String,
    details: Option<String>,
    /// Percentage progress to display, if available.
    progress: Option<f32>,
    /// Recent reasoning lines emitted by the model.
    thinking_lines: Vec<String>,
    /// Labels of in-flight tool calls.
    tool_calls: Vec<String>,
    /// Recent log messages emitted by long-running tasks.
    logs: Vec<String>,
    /// Whether to show the interrupt key hint.
    show_interrupt_hint: bool,

    elapsed_running: Duration,
    last_resume_at: Instant,
    is_paused: bool,
    app_event_tx: AppEventSender,
    frame_requester: FrameRequester,
    animations_enabled: bool,
}

// Format elapsed seconds into a compact human-friendly form used by the status line.
// Examples: 0s, 59s, 1m 00s, 59m 59s, 1h 00m 00s, 2h 03m 09s
pub fn fmt_elapsed_compact(elapsed_secs: u64) -> String {
    if elapsed_secs < 60 {
        return format!("{elapsed_secs}s");
    }
    if elapsed_secs < 3600 {
        let minutes = elapsed_secs / 60;
        let seconds = elapsed_secs % 60;
        return format!("{minutes}m {seconds:02}s");
    }
    let hours = elapsed_secs / 3600;
    let minutes = (elapsed_secs % 3600) / 60;
    let seconds = elapsed_secs % 60;
    format!("{hours}h {minutes:02}m {seconds:02}s")
}

impl StatusIndicatorWidget {
    pub(crate) fn new(
        app_event_tx: AppEventSender,
        frame_requester: FrameRequester,
        animations_enabled: bool,
    ) -> Self {
        Self {
            header: String::from("Working"),
            details: None,
            progress: None,
            thinking_lines: Vec::new(),
            tool_calls: Vec::new(),
            logs: Vec::new(),
            show_interrupt_hint: true,
            elapsed_running: Duration::ZERO,
            last_resume_at: Instant::now(),
            is_paused: false,

            app_event_tx,
            frame_requester,
            animations_enabled,
        }
    }

    pub(crate) fn interrupt(&self) {
        self.app_event_tx.send(AppEvent::CodexOp(Op::Interrupt));
    }

    /// Update the animated header label (left of the brackets).
    pub(crate) fn update_header(&mut self, header: String) {
        self.header = header;
    }

    /// Update the details text shown below the header.
    pub(crate) fn update_details(&mut self, details: Option<String>) {
        self.details = details
            .filter(|details| !details.is_empty())
            .map(|details| capitalize_first(details.trim_start()));
    }

    #[cfg(test)]
    pub(crate) fn header(&self) -> &str {
        &self.header
    }

    pub(crate) fn update_snapshot(&mut self, snapshot: StatusSnapshot) {
        self.update_header(snapshot.header);
        self.progress = snapshot.progress;
        self.thinking_lines = snapshot.thinking;
        self.tool_calls = snapshot.tool_calls;
        self.logs = snapshot.logs;
        self.frame_requester.schedule_frame();
    }

    #[cfg(test)]
    pub(crate) fn details(&self) -> Option<&str> {
        self.details.as_deref()
    }

    pub(crate) fn set_interrupt_hint_visible(&mut self, visible: bool) {
        self.show_interrupt_hint = visible;
    }

    #[cfg(test)]
    pub(crate) fn interrupt_hint_visible(&self) -> bool {
        self.show_interrupt_hint
    }

    pub(crate) fn set_logs(&mut self, logs: Vec<String>) {
        self.logs = logs;
        self.frame_requester.schedule_frame();
    }

    pub(crate) fn pause_timer(&mut self) {
        self.pause_timer_at(Instant::now());
    }

    pub(crate) fn resume_timer(&mut self) {
        self.resume_timer_at(Instant::now());
    }

    pub(crate) fn pause_timer_at(&mut self, now: Instant) {
        if self.is_paused {
            return;
        }
        self.elapsed_running += now.saturating_duration_since(self.last_resume_at);
        self.is_paused = true;
    }

    pub(crate) fn resume_timer_at(&mut self, now: Instant) {
        if !self.is_paused {
            return;
        }
        self.last_resume_at = now;
        self.is_paused = false;
        self.frame_requester.schedule_frame();
    }

    fn elapsed_duration_at(&self, now: Instant) -> Duration {
        let mut elapsed = self.elapsed_running;
        if !self.is_paused {
            elapsed += now.saturating_duration_since(self.last_resume_at);
        }
        elapsed
    }

    fn elapsed_seconds_at(&self, now: Instant) -> u64 {
        self.elapsed_duration_at(now).as_secs()
    }

    pub fn elapsed_seconds(&self) -> u64 {
        self.elapsed_seconds_at(Instant::now())
    }

    /// Wrap the details text into a fixed width and return the lines, truncating if necessary.
    fn wrapped_details_lines(&self, width: u16) -> Vec<Line<'static>> {
        let Some(details) = self.details.as_deref() else {
            return Vec::new();
        };
        if width == 0 {
            return Vec::new();
        }

        let prefix_width = UnicodeWidthStr::width(DETAILS_PREFIX);
        let opts = RtOptions::new(usize::from(width))
            .initial_indent(Line::from(DETAILS_PREFIX.dim()))
            .subsequent_indent(Line::from(Span::from(" ".repeat(prefix_width)).dim()))
            .break_words(true);

        let mut out = word_wrap_lines(details.lines().map(|line| vec![line.dim()]), opts);

        if out.len() > DETAILS_MAX_LINES {
            out.truncate(DETAILS_MAX_LINES);
            let content_width = usize::from(width).saturating_sub(prefix_width).max(1);
            let max_base_len = content_width.saturating_sub(1);
            if let Some(last) = out.last_mut()
                && let Some(span) = last.spans.last_mut()
            {
                let trimmed: String = span.content.as_ref().chars().take(max_base_len).collect();
                *span = format!("{trimmed}…").dim();
            }
        }

        out
    }
}

impl Renderable for StatusIndicatorWidget {
    fn desired_height(&self, width: u16) -> u16 {
        let inner_width = width.max(1) as usize;
        let mut total: u16 = 1; // status line

        total = total.saturating_add(
            u16::try_from(self.wrapped_details_lines(width).len()).unwrap_or(0),
        );

        // Additional thinking/tool call lines beyond the latest one shown inline.
        let extra_thinking = self
            .thinking_lines
            .len()
            .saturating_sub(usize::from(self.thinking_lines.last().is_some()))
            as u16;
        let extra_tool_calls =
            self.tool_calls
                .len()
                .saturating_sub(usize::from(self.tool_calls.last().is_some())) as u16;
        total = total.saturating_add(extra_thinking);
        total = total.saturating_add(extra_tool_calls);

        let text_width = inner_width.saturating_sub(3); // account for " ↳ " prefix
        if text_width > 0 {
            for log in &self.logs {
                let wrapped = textwrap::wrap(log, text_width);
                total = total.saturating_add(wrapped.len() as u16);
            }
        } else {
            total = total.saturating_add(self.logs.len() as u16);
        }

        total
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        // Schedule next animation frame.
        self.frame_requester
            .schedule_frame_in(Duration::from_millis(32));
        let now = Instant::now();
        let elapsed_duration = self.elapsed_duration_at(now);
        let pretty_elapsed = fmt_elapsed_compact(elapsed_duration.as_secs());

        let latest_thinking = self.thinking_lines.last().map(String::as_str);
        let latest_tool_call = self.tool_calls.last().map(String::as_str);

        let mut spans = Vec::with_capacity(9);
        spans.push(spinner(Some(self.last_resume_at), self.animations_enabled));
        spans.push(" ".into());
        if self.animations_enabled {
            spans.extend(shimmer_spans(&self.header));
        } else if !self.header.is_empty() {
            spans.push(self.header.clone().into());
        }
        if let Some(progress) = self.progress {
            let pct = (progress.clamp(0.0, 1.0) * 100.0).round();
            spans.push(" ".into());
            spans.push(format!("{pct:.0}%").dim());
        }
        if let Some(thinking) = latest_thinking {
            spans.push(" - ".into());
            spans.push(thinking.to_string().magenta());
        }
        if let Some(tool) = latest_tool_call {
            spans.push(" - ".into());
            spans.push(tool.to_string().cyan());
        }
        spans.push(" ".into());
        if self.show_interrupt_hint {
            spans.extend(vec![
                format!("({pretty_elapsed} • ").dim(),
                key_hint::plain(KeyCode::Esc).into(),
                " to interrupt)".dim(),
            ]);
        } else {
            spans.push(format!("({pretty_elapsed})").dim());
        }

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(spans));

        if area.height > 1 {
            // Add wrapped details lines beneath the header when there's space.
            let details = self.wrapped_details_lines(area.width);
            let max_details = usize::from(area.height.saturating_sub(1));
            lines.extend(details.into_iter().take(max_details));
        }

        let extra_thinking = self
            .thinking_lines
            .len()
            .saturating_sub(usize::from(latest_thinking.is_some()));
        if extra_thinking > 0 {
            for thinking in self.thinking_lines.iter().take(extra_thinking) {
                lines.push(vec![" ↺ ".magenta(), thinking.clone().magenta()].into());
            }
        }

        let extra_tool_calls = self
            .tool_calls
            .len()
            .saturating_sub(usize::from(latest_tool_call.is_some()));
        if extra_tool_calls > 0 {
            for call in self.tool_calls.iter().take(extra_tool_calls) {
                lines.push(vec![" ↳ ".cyan(), call.clone().cyan()].into());
            }
        }

        let text_width = area.width.saturating_sub(3); // " ↳ " prefix
        if !self.logs.is_empty() {
            if text_width > 0 {
                for log in &self.logs {
                    let wrapped = textwrap::wrap(log, text_width as usize);
                    for (i, piece) in wrapped.iter().enumerate() {
                        let prefix = if i == 0 { " ↳ ".dim() } else { "   ".dim() };
                        lines.push(vec![prefix, piece.to_string().into()].into());
                    }
                }
            } else {
                for log in &self.logs {
                    lines.push(vec![" ↳ ".dim(), log.clone().into()].into());
                }
            }
        }

        Paragraph::new(Text::from(lines)).render_ref(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Duration;
    use std::time::Instant;
    use tokio::sync::mpsc::unbounded_channel;

    use pretty_assertions::assert_eq;

    #[test]
    fn fmt_elapsed_compact_formats_seconds_minutes_hours() {
        assert_eq!(fmt_elapsed_compact(0), "0s");
        assert_eq!(fmt_elapsed_compact(1), "1s");
        assert_eq!(fmt_elapsed_compact(59), "59s");
        assert_eq!(fmt_elapsed_compact(60), "1m 00s");
        assert_eq!(fmt_elapsed_compact(61), "1m 01s");
        assert_eq!(fmt_elapsed_compact(3 * 60 + 5), "3m 05s");
        assert_eq!(fmt_elapsed_compact(59 * 60 + 59), "59m 59s");
        assert_eq!(fmt_elapsed_compact(3600), "1h 00m 00s");
        assert_eq!(fmt_elapsed_compact(3600 + 60 + 1), "1h 01m 01s");
        assert_eq!(fmt_elapsed_compact(25 * 3600 + 2 * 60 + 3), "25h 02m 03s");
    }

    #[test]
    fn renders_with_working_header() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy(), true);

        // Render into a fixed-size test terminal and snapshot the backend.
        let mut terminal = Terminal::new(TestBackend::new(80, 2)).expect("terminal");
        terminal
            .draw(|f| w.render(f.area(), f.buffer_mut()))
            .expect("draw");
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn renders_truncated() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy(), true);

        // Render into a fixed-size test terminal and snapshot the backend.
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).expect("terminal");
        terminal
            .draw(|f| w.render(f.area(), f.buffer_mut()))
            .expect("draw");
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn renders_wrapped_details_panama_two_lines() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy(), false);
        w.update_details(Some("A man a plan a canal panama".to_string()));
        w.set_interrupt_hint_visible(false);

        // Freeze time-dependent rendering (elapsed + spinner) to keep the snapshot stable.
        w.is_paused = true;
        w.elapsed_running = Duration::ZERO;

        // Prefix is 4 columns, so a width of 30 yields a content width of 26: one column
        // short of fitting the whole phrase (27 cols), forcing exactly one wrap without ellipsis.
        let mut terminal = Terminal::new(TestBackend::new(30, 3)).expect("terminal");
        terminal
            .draw(|f| w.render(f.area(), f.buffer_mut()))
            .expect("draw");
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn timer_pauses_when_requested() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut widget =
            StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy(), true);

        let baseline = Instant::now();
        widget.last_resume_at = baseline;

        let before_pause = widget.elapsed_seconds_at(baseline + Duration::from_secs(5));
        assert_eq!(before_pause, 5);

        widget.pause_timer_at(baseline + Duration::from_secs(5));
        let paused_elapsed = widget.elapsed_seconds_at(baseline + Duration::from_secs(10));
        assert_eq!(paused_elapsed, before_pause);

        widget.resume_timer_at(baseline + Duration::from_secs(10));
        let after_resume = widget.elapsed_seconds_at(baseline + Duration::from_secs(13));
        assert_eq!(after_resume, before_pause + 3);
    }

    #[test]
    fn details_overflow_adds_ellipsis() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(tx, crate::tui::FrameRequester::test_dummy(), true);
        w.update_details(Some("abcd abcd abcd abcd".to_string()));

        let lines = w.wrapped_details_lines(6);
        assert_eq!(lines.len(), DETAILS_MAX_LINES);
        let last = lines.last().expect("expected last details line");
        assert!(
            last.spans[1].content.as_ref().ends_with("…"),
            "expected ellipsis in last line: {last:?}"
        );
    }
}
