//! A live task status row rendered above the composer while the agent is busy.
//!
//! The row renders a separately owned clock, the optional interrupt hint, and short inline
//! context (for example, the unified-exec background-process summary). Keeping
//! these pieces on one line avoids vertical layout churn in the bottom pane.
//! Hook activity uses the remaining space or its own line on overflow, so it
//! never displaces background-process controls.

use std::time::Duration;
use std::time::Instant;

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::key_hint::ShortcutHint;
use crate::line_truncation::line_width;
use crate::line_truncation::truncate_line_with_ellipsis;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::motion::MotionMode;
use crate::motion::ReducedMotionIndicator;
use crate::motion::activity_indicator;
use crate::render::renderable::Renderable;
use crate::text_formatting::capitalize_first;
use crate::text_formatting::contains_rtl_or_arabic_number;
use crate::tui::FrameRequester;
use crate::width::display_width;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

mod header;
pub(crate) use header::StatusHeader;
mod timer;
pub(crate) use timer::StatusTimer;

#[path = "summary_shimmer.rs"]
mod summary_shimmer;
use summary_shimmer::summary_shimmer;

pub(crate) const STATUS_DETAILS_DEFAULT_MAX_LINES: usize = 3;
const DETAILS_PREFIX: &str = "  └ ";
const HEADING_INTERVAL: Duration = Duration::from_millis(/*millis*/ 3500);

#[derive(Clone, Debug)]
struct HeaderSample {
    header: StatusHeader,
    outgoing: Option<String>,
    next_sample: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusDetailsCapitalization {
    CapitalizeFirst,
    Preserve,
}

/// Displays a single-line in-progress status with optional wrapped details.
#[derive(Clone, Debug)]
pub(crate) struct StatusIndicatorWidget {
    /// Animated header text (defaults to "Working").
    header: StatusHeader,
    header_started_at: Instant,
    prepared_at: Option<Instant>,
    sample: Option<HeaderSample>,
    details: Option<String>,
    details_max_lines: usize,
    /// Optional suffix rendered after the elapsed/interrupt segment.
    inline_message: Option<String>,
    /// Hook activity may move below the status row when it cannot fit in full.
    hook_status_message: Option<String>,
    show_interrupt_hint: bool,
    interrupt_binding: Option<ShortcutHint>,

    app_event_tx: AppEventSender,
    frame_requester: FrameRequester,
    animations_enabled: bool,
    effects: codex_config::types::TuiEffects,
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
        effects: codex_config::types::TuiEffects,
    ) -> Self {
        Self {
            header: String::from("Working").into(),
            header_started_at: Instant::now(),
            prepared_at: None,
            sample: None,
            details: None,
            details_max_lines: STATUS_DETAILS_DEFAULT_MAX_LINES,
            inline_message: None,
            hook_status_message: None,
            show_interrupt_hint: true,
            interrupt_binding: Some(key_hint::plain(KeyCode::Esc).into()),
            app_event_tx,
            frame_requester,
            animations_enabled,
            effects,
        }
    }

    pub(crate) fn interrupt(&self) {
        self.app_event_tx.interrupt();
    }

    pub(crate) fn set_motion_preferences(&mut self, tui: &codex_config::types::Tui) {
        self.animations_enabled = tui.animations;
        self.effects = tui.effects;
    }

    fn shimmer_motion(&self) -> MotionMode {
        MotionMode::from_animations_enabled(self.animations_enabled && self.effects.shimmer)
    }

    /// Use the same heading and transition time for layout and painting.
    pub(crate) fn prepare_frame(&mut self, now: Instant) {
        self.prepared_at = Some(now);
        if self.shimmer_motion() == MotionMode::Reduced
            || summary_shimmer::palette(self.header()).is_none()
        {
            self.sample = None;
            return;
        }
        let sample = self.sample.get_or_insert_with(|| HeaderSample {
            header: self.header.clone(),
            outgoing: None,
            next_sample: None,
        });
        let due = sample.next_sample.unwrap_or(now);
        if !self.header.is_reasoning() || now < due {
            return;
        }
        // Keep the first heading's clock even when a frame is drawn late.
        let late = (now.duration_since(due).as_nanos() % HEADING_INTERVAL.as_nanos()) as u64;
        sample.next_sample = Some(now + HEADING_INTERVAL - Duration::from_nanos(late));
        if sample.header != self.header {
            let previous = std::mem::replace(&mut sample.header, self.header.clone());
            if previous.text != self.header.text {
                sample.outgoing = Some(previous.text);
                self.header_started_at = now;
            }
        }
    }

    fn header_spans(&self, now: Instant, columns: usize) -> Vec<Span<'static>> {
        summary_shimmer(
            self.header(),
            now.saturating_duration_since(self.header_started_at),
            self.shimmer_motion(),
            columns,
            self.sample.as_ref(),
        )
    }

    /// Update the animated header label (left of the brackets).
    pub(crate) fn update_header(&mut self, header: impl Into<StatusHeader>) {
        let header = header.into();
        if !header.is_reasoning() && self.header != header {
            self.sample = None;
        }
        if self.sample.is_none() && self.header.text != header.text {
            self.header_started_at = Instant::now();
        }
        self.header = header;
    }

    pub(crate) fn extend_reasoning(&mut self, item_id: Option<&str>, part: usize, summary: &str) {
        if let Some(sample) = &mut self.sample {
            sample.header.extend(item_id, part, summary);
        }
    }

    /// Update the details text shown below the header.
    pub(crate) fn update_details(
        &mut self,
        details: Option<String>,
        capitalization: StatusDetailsCapitalization,
        max_lines: usize,
    ) {
        self.details_max_lines = max_lines.max(1);
        self.details = details
            .filter(|details| !details.is_empty())
            .map(|details| {
                let trimmed = details.trim_start();
                match capitalization {
                    StatusDetailsCapitalization::CapitalizeFirst => capitalize_first(trimmed),
                    StatusDetailsCapitalization::Preserve => trimmed.to_string(),
                }
            });
    }

    /// Update the inline suffix text shown after the elapsed/interrupt hint.
    ///
    /// Callers should provide plain, already-contextualized text. Passing
    /// verbose status prose here can cause frequent width truncation and hide
    /// the more important elapsed/interrupt hint.
    pub(crate) fn update_inline_message(&mut self, message: Option<String>) {
        self.inline_message = message
            .map(|message| message.trim().to_string())
            .filter(|message| !message.is_empty());
    }

    pub(crate) fn update_hook_status_message(&mut self, message: Option<String>) {
        self.hook_status_message = message;
    }

    pub(crate) fn header(&self) -> &str {
        &self.header.text
    }

    #[cfg(test)]
    pub(crate) fn details(&self) -> Option<&str> {
        self.details.as_deref()
    }

    pub(crate) fn set_interrupt_hint_visible(&mut self, visible: bool) {
        self.show_interrupt_hint = visible;
    }

    pub(crate) fn set_interrupt_binding(&mut self, binding: Option<ShortcutHint>) {
        self.interrupt_binding = binding;
    }

    pub(crate) fn with_timer<'a>(&'a self, timer: &'a StatusTimer) -> impl Renderable + 'a {
        StatusIndicator { row: self, timer }
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
            .break_words(/*break_words*/ true);

        let mut out = word_wrap_lines(details.lines().map(|line| vec![line.dim()]), opts);

        if out.len() > self.details_max_lines {
            out.truncate(self.details_max_lines);
            if let Some(last) = out.last_mut() {
                *last = truncate_line_with_ellipsis(std::mem::take(last), usize::from(width)).dim();
            }
        }

        out
    }
}

struct StatusIndicator<'a> {
    row: &'a StatusIndicatorWidget,
    timer: &'a StatusTimer,
}

impl StatusIndicator<'_> {
    // Share width decisions between height measurement and rendering, including
    // wide Unicode characters, remapped interrupt hints, and elapsed-time text.
    fn lines(&self, width: u16) -> Vec<Line<'static>> {
        let row = self.row;
        let now = row.prepared_at.unwrap_or_else(Instant::now);
        let elapsed_duration = self.timer.display_elapsed_at(now);
        let pretty_elapsed = fmt_elapsed_compact(elapsed_duration.as_secs());
        let progress =
            MotionMode::from_animations_enabled(row.animations_enabled && row.effects.progress);
        let mut spans = Vec::with_capacity(5);
        let mut label_columns = usize::from(width);
        if let Some(indicator) = activity_indicator(
            Some(self.timer.last_resume_at),
            progress,
            ReducedMotionIndicator::Hidden,
        ) {
            label_columns = label_columns.saturating_sub(indicator.width() + 1);
            spans.push(indicator);
            spans.push(" ".into());
        }
        spans.extend(row.header_spans(now, label_columns));
        if !spans.is_empty() {
            spans.push(" ".into());
        }
        if row.show_interrupt_hint
            && let Some(interrupt_binding) = row.interrupt_binding
        {
            spans.push(format!("({pretty_elapsed} • ").dim());
            spans.extend(interrupt_binding.spans());
            spans.push(" to interrupt)".dim());
        } else {
            spans.push(format!("({pretty_elapsed})").dim());
        }
        if let Some(message) = &row.inline_message {
            // Keep optional context after elapsed/interrupt text so that core
            // interrupt affordances stay in a fixed visual location.
            spans.push(" · ".dim());
            spans.push(message.clone().dim());
        }

        let mut header = Line::from(spans);
        let mut hook_overflow = None;
        if let Some(message) = &row.hook_status_message {
            if line_width(&header) + display_width(" · ") + display_width(message)
                <= usize::from(width)
            {
                header.spans.extend([" · ".dim(), message.clone().dim()]);
            } else {
                hook_overflow = Some(truncate_line_with_ellipsis_if_overflow(
                    Line::from(vec![DETAILS_PREFIX.dim(), message.clone().dim()]),
                    usize::from(width),
                ));
            }
        }
        let mut lines = Vec::new();
        lines.push(truncate_line_with_ellipsis_if_overflow(
            header,
            usize::from(width),
        ));
        lines.extend(hook_overflow);
        lines.extend(row.wrapped_details_lines(width));
        lines
    }
}

impl Renderable for StatusIndicator<'_> {
    fn desired_height(&self, width: u16) -> u16 {
        self.lines(width).len() as u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let animated = self.row.animations_enabled
            && (self.row.effects.progress
                || (self.row.effects.shimmer && !contains_rtl_or_arabic_number(self.row.header())));
        let delay = if animated {
            Some(Duration::from_millis(32))
        } else {
            self.timer.next_tick_delay(Instant::now())
        };
        if let Some(delay) = delay {
            self.row.frame_requester.schedule_frame_in(delay);
        }
        Paragraph::new(Text::from(self.lines(area.width))).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tokio::sync::mpsc::unbounded_channel;

    use pretty_assertions::assert_eq;

    #[test]
    fn changed_summary_restarts_shimmer_but_repeated_summary_keeps_phase() {
        let (tx, _rx) = unbounded_channel();
        let mut row = StatusIndicatorWidget::new(
            AppEventSender::new(tx),
            FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
            Default::default(),
        );
        let previous = Instant::now() - Duration::from_secs(/*secs*/ 1);
        row.header_started_at = previous;
        row.update_header("Working".to_owned());
        assert_eq!(row.header_started_at, previous);
        row.update_header("Mapping the app structure".to_owned());
        assert!(row.header_started_at > previous);
    }

    #[test]
    fn static_headers_schedule_clock_ticks_and_animated_headers_schedule_faster_frames() {
        for (header, animations, progress, expected_ms) in [
            ("Working", false, true, 1_000),
            ("שלום עולם", true, false, 1_000),
            ("Checking lines ١٢-٣٤", true, false, 1_000),
            ("Working", true, false, 32),
            ("שלום עולם", true, true, 32),
        ] {
            let (tx, _rx) = unbounded_channel();
            let (requester, mut requests) = FrameRequester::test_channel();
            let mut row = StatusIndicatorWidget::new(
                AppEventSender::new(tx),
                requester,
                animations,
                codex_config::types::TuiEffects {
                    progress,
                    shimmer: true,
                    ..Default::default()
                },
            );
            row.update_header(header.to_owned());
            let timer = StatusTimer::default();
            let area = Rect::new(0, 0, 80, 1);
            let before = timer.last_resume_at;
            row.with_timer(&timer)
                .render(area, &mut Buffer::empty(area));
            let after = Instant::now();
            let requested = requests.try_recv().expect("scheduled frame");
            let interval = Duration::from_millis(expected_ms);
            assert!((before + interval..=after + interval).contains(&requested));
        }
    }

    #[test]
    fn fmt_elapsed_compact_formats_seconds_minutes_hours() {
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 0), "0s");
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 1), "1s");
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 59), "59s");
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 60), "1m 00s");
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 61), "1m 01s");
        assert_eq!(fmt_elapsed_compact(3 * 60 + 5), "3m 05s");
        assert_eq!(fmt_elapsed_compact(59 * 60 + 59), "59m 59s");
        assert_eq!(fmt_elapsed_compact(/*elapsed_secs*/ 3600), "1h 00m 00s");
        assert_eq!(fmt_elapsed_compact(3600 + 60 + 1), "1h 01m 01s");
        assert_eq!(fmt_elapsed_compact(25 * 3600 + 2 * 60 + 3), "25h 02m 03s");
    }

    #[test]
    fn renders_with_working_header() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let timer = StatusTimer::default();
        let w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
            Default::default(),
        );

        // Render into a fixed-size test terminal and snapshot the backend.
        let mut terminal = Terminal::new(TestBackend::new(80, 2)).expect("terminal");
        terminal
            .draw(|f| w.with_timer(&timer).render(f.area(), f.buffer_mut()))
            .expect("draw");
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn renders_truncated() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let timer = StatusTimer::default();
        let w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
            Default::default(),
        );

        // Render into a fixed-size test terminal and snapshot the backend.
        let mut terminal = Terminal::new(TestBackend::new(20, 2)).expect("terminal");
        terminal
            .draw(|f| w.with_timer(&timer).render(f.area(), f.buffer_mut()))
            .expect("draw");
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn renders_wrapped_details_panama_two_lines() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
            Default::default(),
        );
        w.update_details(
            Some("A man a plan a canal panama".to_string()),
            StatusDetailsCapitalization::CapitalizeFirst,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
        w.set_interrupt_hint_visible(/*visible*/ false);

        // Freeze time-dependent rendering (elapsed + spinner) to keep the snapshot stable.
        let mut timer = StatusTimer::default();
        timer.pause_at(timer.last_resume_at);

        // Prefix is 4 columns, so a width of 30 yields a content width of 26: one column
        // short of fitting the whole phrase (27 cols), forcing exactly one wrap without ellipsis.
        let mut terminal = Terminal::new(TestBackend::new(30, 3)).expect("terminal");
        terminal
            .draw(|f| w.with_timer(&timer).render(f.area(), f.buffer_mut()))
            .expect("draw");
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn renders_without_spinner_when_animations_disabled() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
            Default::default(),
        );
        let mut timer = StatusTimer::default();
        timer.pause_at(timer.last_resume_at + Duration::from_secs(1));

        let mut terminal = Terminal::new(TestBackend::new(80, 1)).expect("terminal");
        terminal
            .draw(|f| w.with_timer(&timer).render(f.area(), f.buffer_mut()))
            .expect("draw");
        let line = terminal.backend().buffer().content()[..80]
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();

        insta::assert_snapshot!(line.trim_end(), @"Working (1s • esc to interrupt)");
    }

    #[test]
    fn renders_remapped_interrupt_hint() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
            Default::default(),
        );
        w.set_interrupt_binding(Some(key_hint::plain(KeyCode::F(12)).into()));
        let mut timer = StatusTimer::default();
        timer.pause_at(timer.last_resume_at);

        let mut terminal = Terminal::new(TestBackend::new(80, 1)).expect("terminal");
        terminal
            .draw(|f| w.with_timer(&timer).render(f.area(), f.buffer_mut()))
            .expect("draw");
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn hook_status_reflows_without_displacing_controls_or_details() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let mut w = StatusIndicatorWidget::new(
            AppEventSender::new(tx),
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ false,
            Default::default(),
        );
        let mut timer = StatusTimer::default();
        timer.pause_at(timer.last_resume_at);
        w.update_hook_status_message(Some("checking 日本語 ｶﾞﾊﾟ policy".to_string()));
        w.update_details(
            Some("existing details".to_string()),
            StatusDetailsCapitalization::Preserve,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );

        for (background, snapshot) in [
            (None, "hook_status_reflows_without_background_activity"),
            (
                Some("1 background terminal running · /ps to view · /stop to close"),
                "hook_status_reflows_with_background_activity",
            ),
        ] {
            w.update_inline_message(background.map(str::to_string));
            let mut expected = "Working (0s • esc to interrupt)".to_string();
            if let Some(background) = background {
                expected.push_str(&format!(" · {background}"));
            }
            expected.push_str(" · checking 日本語 ｶﾞﾊﾟ policy");
            let fit_width = display_width(&expected) as u16;
            let mut frames = Vec::new();
            for width in [fit_width, fit_width - 1, 24, fit_width] {
                let height = w.with_timer(&timer).desired_height(width);
                assert_eq!(height, if width >= fit_width { 2 } else { 3 });
                let mut terminal =
                    Terminal::new(TestBackend::new(width, height)).expect("terminal");
                terminal
                    .draw(|f| w.with_timer(&timer).render(f.area(), f.buffer_mut()))
                    .expect("draw");
                frames.push(format!("{width} columns:\n{}", terminal.backend()));
            }
            insta::assert_snapshot!(snapshot, frames.join("\n"));
        }
    }

    #[test]
    fn details_overflow_adds_ellipsis() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
            Default::default(),
        );
        w.update_details(
            Some("abcd abcd abcd abcd".to_string()),
            StatusDetailsCapitalization::CapitalizeFirst,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );

        let lines = w.wrapped_details_lines(/*width*/ 6);
        assert_eq!(lines.len(), STATUS_DETAILS_DEFAULT_MAX_LINES);
        let last = lines.last().expect("expected last details line");
        assert!(
            last.to_string().ends_with('…'),
            "expected ellipsis in last line: {last:?}"
        );

        w.update_details(
            Some("界".repeat(20)),
            StatusDetailsCapitalization::Preserve,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
        let lines = w.wrapped_details_lines(/*width*/ 10);
        assert!(lines.iter().all(|line| line_width(line) <= 10));
        insta::assert_snapshot!(
            lines.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"),
            @r###"
          └ 界界界
            界界界
            界界…
        "###
        );
    }

    #[test]
    fn details_args_can_disable_capitalization_and_limit_lines() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut w = StatusIndicatorWidget::new(
            tx,
            crate::tui::FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
            Default::default(),
        );
        w.update_details(
            Some("cargo test -p codex-core and then cargo test -p codex-tui".to_string()),
            StatusDetailsCapitalization::Preserve,
            /*max_lines*/ 1,
        );

        assert_eq!(
            w.details(),
            Some("cargo test -p codex-core and then cargo test -p codex-tui")
        );

        let lines = w.wrapped_details_lines(/*width*/ 24);
        assert_eq!(lines.len(), 1);
        let last = lines.last().expect("expected one details line");
        assert!(
            last.spans
                .last()
                .is_some_and(|span| span.content.as_ref().contains('…')),
            "expected one-line details to be ellipsized, got {last:?}"
        );
    }
}

#[cfg(test)]
#[path = "status_indicator_widget/effects_tests.rs"]
mod effects_tests;
