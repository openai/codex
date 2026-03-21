use crate::history_cell::HistoryCell;
use crate::history_cell::{self};
use crate::markdown_stream::MarkdownStreamCollector;
use crate::render::line_utils::prefix_lines;
use crate::style::proposed_plan_style;
use ratatui::prelude::Stylize;
use ratatui::text::Line;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug)]
struct PendingSnapshot {
    lines: Vec<Line<'static>>,
    enqueued_at: Instant,
}

/// Controller that manages newline-gated assistant-message streaming as a
/// mutable full-message snapshot.
pub(crate) struct StreamController {
    collector: MarkdownStreamCollector,
    pending_snapshot: Option<PendingSnapshot>,
    current_lines: Vec<Line<'static>>,
    has_seen_delta: bool,
}

impl StreamController {
    /// Create a controller whose markdown renderer shortens local file links relative to `cwd`.
    ///
    /// The controller snapshots the path into stream state so later commit ticks and finalization
    /// render against the same session cwd that was active when streaming started.
    pub(crate) fn new(width: Option<usize>, cwd: &Path) -> Self {
        Self {
            collector: MarkdownStreamCollector::new(width, cwd),
            pending_snapshot: None,
            current_lines: Vec::new(),
            has_seen_delta: false,
        }
    }

    fn clear(&mut self) {
        self.collector.clear();
        self.pending_snapshot = None;
        self.current_lines.clear();
        self.has_seen_delta = false;
    }

    fn set_snapshot(&mut self, lines: Vec<Line<'static>>) -> bool {
        if lines.is_empty() || lines == self.current_lines {
            return false;
        }
        let enqueued_at = self
            .pending_snapshot
            .as_ref()
            .map(|snapshot| snapshot.enqueued_at)
            .unwrap_or_else(Instant::now);
        self.current_lines = lines.clone();
        self.pending_snapshot = Some(PendingSnapshot { lines, enqueued_at });
        true
    }

    /// Push a delta; if it contains a newline, stage the latest committed stream snapshot.
    pub(crate) fn push(&mut self, delta: &str) -> bool {
        if !delta.is_empty() {
            self.has_seen_delta = true;
        }
        self.collector.push_delta(delta);
        if delta.contains('\n') {
            let rendered = self.collector.commit_complete_lines();
            return self.set_snapshot(rendered);
        }
        false
    }

    /// Finalize the active stream and emit the final full-message snapshot.
    pub(crate) fn finalize(&mut self) -> Option<Box<dyn HistoryCell>> {
        let rendered = self.collector.finalize_and_drain();
        if !rendered.is_empty() {
            self.current_lines = rendered;
        }
        let out = if self.current_lines.is_empty() && !self.has_seen_delta {
            None
        } else {
            self.emit(self.current_lines.clone())
        };
        self.clear();
        out
    }

    /// Step animation: emit the latest pending full-message snapshot.
    pub(crate) fn on_commit_tick(&mut self) -> (Option<Box<dyn HistoryCell>>, bool) {
        let step = self.pending_snapshot.take().map(|snapshot| snapshot.lines);
        let is_idle = self.pending_snapshot.is_none();
        (self.emit(step.unwrap_or_default()), is_idle)
    }

    /// Batch drains collapse to a single snapshot because the controller is snapshot-based.
    pub(crate) fn on_commit_tick_batch(
        &mut self,
        _max_lines: usize,
    ) -> (Option<Box<dyn HistoryCell>>, bool) {
        self.on_commit_tick()
    }

    /// Returns the current number of staged snapshots waiting to be displayed.
    pub(crate) fn queued_lines(&self) -> usize {
        usize::from(self.pending_snapshot.is_some())
    }

    /// Returns the age of the current staged snapshot.
    pub(crate) fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.pending_snapshot
            .as_ref()
            .map(|snapshot| now.saturating_duration_since(snapshot.enqueued_at))
    }

    fn emit(&mut self, lines: Vec<Line<'static>>) -> Option<Box<dyn HistoryCell>> {
        if lines.is_empty() {
            return None;
        }
        Some(Box::new(history_cell::AgentMessageCell::new(lines, true)))
    }
}

/// Controller that streams proposed plan markdown into a mutable styled plan block.
pub(crate) struct PlanStreamController {
    collector: MarkdownStreamCollector,
    pending_snapshot: Option<PendingSnapshot>,
    current_lines: Vec<Line<'static>>,
    has_seen_delta: bool,
}

impl PlanStreamController {
    /// Create a plan-stream controller whose markdown renderer shortens local file links relative
    /// to `cwd`.
    ///
    /// The controller snapshots the path into stream state so later commit ticks and finalization
    /// render against the same session cwd that was active when streaming started.
    pub(crate) fn new(width: Option<usize>, cwd: &Path) -> Self {
        Self {
            collector: MarkdownStreamCollector::new(width, cwd),
            pending_snapshot: None,
            current_lines: Vec::new(),
            has_seen_delta: false,
        }
    }

    fn clear(&mut self) {
        self.collector.clear();
        self.pending_snapshot = None;
        self.current_lines.clear();
        self.has_seen_delta = false;
    }

    fn set_snapshot(&mut self, lines: Vec<Line<'static>>) -> bool {
        if lines.is_empty() || lines == self.current_lines {
            return false;
        }
        let enqueued_at = self
            .pending_snapshot
            .as_ref()
            .map(|snapshot| snapshot.enqueued_at)
            .unwrap_or_else(Instant::now);
        self.current_lines = lines.clone();
        self.pending_snapshot = Some(PendingSnapshot { lines, enqueued_at });
        true
    }

    /// Push a delta; if it contains a newline, stage the latest committed plan snapshot.
    pub(crate) fn push(&mut self, delta: &str) -> bool {
        if !delta.is_empty() {
            self.has_seen_delta = true;
        }
        self.collector.push_delta(delta);
        if delta.contains('\n') {
            let rendered = self.collector.commit_complete_lines();
            return self.set_snapshot(rendered);
        }
        false
    }

    /// Finalize the active stream and emit the final full plan snapshot.
    pub(crate) fn finalize(&mut self) -> Option<Box<dyn HistoryCell>> {
        let rendered = self.collector.finalize_and_drain();
        if !rendered.is_empty() {
            self.current_lines = rendered;
        }
        let out = if self.current_lines.is_empty() && !self.has_seen_delta {
            None
        } else {
            self.emit(self.current_lines.clone(), true)
        };
        self.clear();
        out
    }

    /// Step animation: emit the latest pending full plan snapshot.
    pub(crate) fn on_commit_tick(&mut self) -> (Option<Box<dyn HistoryCell>>, bool) {
        let step = self.pending_snapshot.take().map(|snapshot| snapshot.lines);
        let is_idle = self.pending_snapshot.is_none();
        (self.emit(step.unwrap_or_default(), false), is_idle)
    }

    /// Batch drains collapse to a single snapshot because the controller is snapshot-based.
    pub(crate) fn on_commit_tick_batch(
        &mut self,
        _max_lines: usize,
    ) -> (Option<Box<dyn HistoryCell>>, bool) {
        self.on_commit_tick()
    }

    /// Returns the current number of staged plan snapshots waiting to be displayed.
    pub(crate) fn queued_lines(&self) -> usize {
        usize::from(self.pending_snapshot.is_some())
    }

    /// Returns the age of the current staged plan snapshot.
    pub(crate) fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.pending_snapshot
            .as_ref()
            .map(|snapshot| now.saturating_duration_since(snapshot.enqueued_at))
    }

    fn emit(
        &mut self,
        lines: Vec<Line<'static>>,
        include_bottom_padding: bool,
    ) -> Option<Box<dyn HistoryCell>> {
        if lines.is_empty() && !include_bottom_padding {
            return None;
        }

        let mut out_lines: Vec<Line<'static>> = vec![
            vec!["• ".dim(), "Proposed Plan".bold()].into(),
            Line::from(" "),
        ];

        let mut plan_lines: Vec<Line<'static>> = vec![Line::from(" ")];
        plan_lines.extend(lines);
        if include_bottom_padding {
            plan_lines.push(Line::from(" "));
        }

        let plan_style = proposed_plan_style();
        let plan_lines = prefix_lines(plan_lines, "  ".into(), "  ".into())
            .into_iter()
            .map(|line| line.style(plan_style))
            .collect::<Vec<_>>();
        out_lines.extend(plan_lines);

        Some(Box::new(history_cell::new_proposed_plan_stream(
            out_lines, false,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use std::path::PathBuf;

    fn test_cwd() -> PathBuf {
        // These tests only need a stable absolute cwd; using temp_dir() avoids baking Unix- or
        // Windows-specific root semantics into the fixtures.
        std::env::temp_dir()
    }

    fn lines_to_plain_strings(lines: &[ratatui::text::Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .collect()
    }

    #[derive(Debug)]
    struct ControllerTrace {
        display_width: usize,
        deltas: Vec<String>,
        transcript: Vec<String>,
        visible_rows: Vec<String>,
        full_render: Vec<String>,
    }

    impl fmt::Display for ControllerTrace {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            writeln!(f, "display_width: {}", self.display_width)?;
            writeln!(f, "deltas:")?;
            for (idx, delta) in self.deltas.iter().enumerate() {
                writeln!(f, "  [{idx}] {delta:?}")?;
            }
            writeln!(f, "transcript: {:?}", self.transcript)?;
            writeln!(f, "visible_rows: {:?}", self.visible_rows)?;
            writeln!(f, "full_render: {:?}", self.full_render)
        }
    }

    fn render_markdown_to_plain_strings(source: &str, width: Option<usize>) -> Vec<String> {
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        let test_cwd = test_cwd();
        crate::markdown::append_markdown(source, width, Some(test_cwd.as_path()), &mut rendered);
        lines_to_plain_strings(&rendered)
    }

    fn strip_agent_prefix(line: String) -> String {
        line.chars().skip(2).collect()
    }

    fn strip_agent_prefixes(lines: Vec<String>) -> Vec<String> {
        lines.into_iter().map(strip_agent_prefix).collect()
    }

    fn collect_controller_trace(deltas: &[&str], display_width: usize) -> ControllerTrace {
        let collector_width = display_width.saturating_sub(2);
        let mut ctrl = StreamController::new(Some(collector_width), &test_cwd());
        let mut transcript = Vec::new();
        let mut visible_rows = Vec::new();

        for delta in deltas {
            ctrl.push(delta);
            while let (Some(cell), idle) = ctrl.on_commit_tick() {
                transcript =
                    strip_agent_prefixes(lines_to_plain_strings(&cell.transcript_lines(u16::MAX)));
                visible_rows = strip_agent_prefixes(lines_to_plain_strings(
                    &cell.display_lines(display_width as u16),
                ));
                if idle {
                    break;
                }
            }
        }

        if let Some(cell) = ctrl.finalize() {
            transcript =
                strip_agent_prefixes(lines_to_plain_strings(&cell.transcript_lines(u16::MAX)));
            visible_rows = strip_agent_prefixes(lines_to_plain_strings(
                &cell.display_lines(display_width as u16),
            ));
        }

        let full_source: String = deltas.iter().copied().collect();
        let full_render = render_markdown_to_plain_strings(&full_source, Some(collector_width));

        ControllerTrace {
            display_width,
            deltas: deltas.iter().map(|delta| (*delta).to_string()).collect(),
            transcript,
            visible_rows,
            full_render,
        }
    }

    fn assert_controller_matches_full(label: &str, deltas: &[&str], display_width: usize) {
        let trace = collect_controller_trace(deltas, display_width);
        assert_eq!(
            trace.transcript, trace.full_render,
            "{label} diverged at transcript layer\n{trace}"
        );
        assert_eq!(
            trace.visible_rows, trace.full_render,
            "{label} diverged at visible row layer\n{trace}"
        );
    }

    #[tokio::test]
    async fn controller_loose_vs_tight_with_commit_ticks_matches_full() {
        let mut ctrl = StreamController::new(None, &test_cwd());
        let mut lines = Vec::new();

        // Exact deltas from the session log (section: Loose vs. tight list items)
        let deltas = vec![
            "\n\n",
            "Loose",
            " vs",
            ".",
            " tight",
            " list",
            " items",
            ":\n",
            "1",
            ".",
            " Tight",
            " item",
            "\n",
            "2",
            ".",
            " Another",
            " tight",
            " item",
            "\n\n",
            "1",
            ".",
            " Loose",
            " item",
            " with",
            " its",
            " own",
            " paragraph",
            ".\n\n",
            "  ",
            " This",
            " paragraph",
            " belongs",
            " to",
            " the",
            " same",
            " list",
            " item",
            ".\n\n",
            "2",
            ".",
            " Second",
            " loose",
            " item",
            " with",
            " a",
            " nested",
            " list",
            " after",
            " a",
            " blank",
            " line",
            ".\n\n",
            "  ",
            " -",
            " Nested",
            " bullet",
            " under",
            " a",
            " loose",
            " item",
            "\n",
            "  ",
            " -",
            " Another",
            " nested",
            " bullet",
            "\n\n",
        ];

        // Simulate streaming with a commit tick attempt after each delta.
        for d in deltas.iter() {
            ctrl.push(d);
            while let (Some(cell), idle) = ctrl.on_commit_tick() {
                lines = cell.transcript_lines(u16::MAX);
                if idle {
                    break;
                }
            }
        }
        // Finalize and flush remaining lines now.
        if let Some(cell) = ctrl.finalize() {
            lines = cell.transcript_lines(u16::MAX);
        }

        let streamed = strip_agent_prefixes(lines_to_plain_strings(&lines));

        // Full render of the same source
        let source: String = deltas.iter().copied().collect();
        let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
        let test_cwd = test_cwd();
        crate::markdown::append_markdown(&source, None, Some(test_cwd.as_path()), &mut rendered);
        let rendered_strs = lines_to_plain_strings(&rendered);

        assert_eq!(streamed, rendered_strs);

        // Also assert exact expected plain strings for clarity.
        let expected = vec![
            "Loose vs. tight list items:".to_string(),
            "".to_string(),
            "1. Tight item".to_string(),
            "2. Another tight item".to_string(),
            "3. Loose item with its own paragraph.".to_string(),
            "".to_string(),
            "   This paragraph belongs to the same list item.".to_string(),
            "4. Second loose item with a nested list after a blank line.".to_string(),
            "    - Nested bullet under a loose item".to_string(),
            "    - Another nested bullet".to_string(),
        ];
        assert_eq!(
            streamed, expected,
            "expected exact rendered lines for loose/tight section"
        );
    }

    #[tokio::test]
    async fn controller_inline_code_completion_rewrites_prior_line_matches_full() {
        let deltas = ["Проверяю `S2` vs `\n", "N/A`\n"];

        for display_width in [26usize, 42, 82] {
            assert_controller_matches_full(
                "stream controller should not preserve stale pre-closure inline-code output",
                &deltas,
                display_width,
            );
        }
    }

    #[tokio::test]
    async fn controller_issue_15001_repro_b_matches_full() {
        let deltas = [
            "Evidence собран; перехожу к reviewer artifact. Открою один-два свежих backend review-файла, чтобы сохранить repo-local формат и правильно зафиксировать `S2` vs `\n",
            "N/A` для visual review.\n",
        ];

        for display_width in [42usize, 50, 62, 74, 82] {
            assert_controller_matches_full(
                "stream controller should match one-shot render for issue-15001 repro B",
                &deltas,
                display_width,
            );
        }
    }

    #[tokio::test]
    async fn controller_rewraps_prior_rows_without_swallowing_shifted_words() {
        let deltas = ["This is a very long sentence that\n", "causes wrapping\n"];

        for display_width in [22usize, 26, 30] {
            assert_controller_matches_full(
                "stream controller should replace earlier rows when later text rewraps them",
                &deltas,
                display_width,
            );
        }
    }
}
