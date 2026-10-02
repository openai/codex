//! History cell for hook execution.
//!
//! Hooks are intentionally quieter than normal tool calls. A hook that starts and finishes
//! successfully without user-facing output should not leave a transcript artifact, and very fast
//! hooks should not flash in the viewport. Model-facing hook context never appears in the TUI. This
//! cell keeps that policy local by treating each hook run as a small rendering state machine:
//!
//! 1. New runs begin hidden in `PendingReveal`.
//! 2. Revealed runs contribute to the compact activity summary, never transcript lines.
//! 3. Quiet completions leave the summary immediately but retain their existing cleanup timer
//!    for exec-flush and usage-output ordering.
//! 4. Completed runs only persist when they have user-facing output or a non-success status.
use super::HistoryCell;
use super::plain_lines;
use codex_ansi_escape::ansi_escape;
use codex_app_server_protocol::HookOutputEntry;
use codex_app_server_protocol::HookOutputEntryKind;
use codex_app_server_protocol::HookRunStatus;
use codex_app_server_protocol::HookRunSummary;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug)]
pub(crate) struct HookCell {
    /// Hook runs that are active, lingering, or have persistent output to render.
    runs: Vec<HookRunCell>,
}

/// Minimum runtime before a hook is allowed to draw.
///
/// Avoids flashing activity for work that was effectively instant.
const HOOK_RUN_REVEAL_DELAY: Duration = Duration::from_millis(300);

/// Minimum interval after reveal before quiet-completion bookkeeping is removed.
///
/// The activity summary clears on completion; retain the existing cleanup timing
/// used by exec flushing and deferred usage output.
const QUIET_HOOK_MIN_VISIBLE: Duration = Duration::from_millis(600);

const HOOK_OUTPUT_INDENT: &str = "  ";
const HOOK_OUTPUT_BODY_INDENT: &str = "    ";

#[derive(Debug)]
struct HookRunCell {
    /// Stable protocol id used to match begin/end updates for the same hook invocation.
    id: String,
    /// Optional hook-supplied detail shown next to the running header.
    status_message: Option<String>,
    /// Rendering lifecycle for this run.
    state: HookRunState,
}

#[derive(Debug)]
enum HookRunState {
    /// A newly-started run that is active but deliberately hidden until `reveal_deadline`.
    PendingReveal {
        /// First instant at which the run may become visible.
        reveal_deadline: Instant,
    },
    /// A run that survived the reveal delay and is currently shown as running.
    VisibleRunning {
        /// First instant the run was actually rendered, used by quiet-success linger.
        visible_since: Instant,
    },
    /// A visible run that completed successfully without output but is still lingering briefly.
    QuietLinger {
        /// Instant after which the quiet success can be removed entirely.
        removal_deadline: Instant,
    },
    /// A completed run with output or a status worth preserving in history.
    Completed {
        /// Final protocol status for the hook invocation.
        status: HookRunStatus,
        /// Hook output entries rendered below the completed header.
        entries: Vec<HookOutputEntry>,
    },
}

impl HookCell {
    /// Creates a cell around a hook that has just started.
    fn new_active(run: HookRunSummary) -> Self {
        let mut cell = Self { runs: Vec::new() };
        cell.start_run(run);
        cell
    }

    /// Creates a cell around an already-completed hook from transcript/history data.
    fn new_completed(run: HookRunSummary) -> Self {
        let mut cell = Self { runs: Vec::new() };
        cell.add_completed_run(run);
        cell
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Returns true while any run can still change due to an end event or timer.
    pub(crate) fn is_active(&self) -> bool {
        self.runs.iter().any(|run| run.state.is_active())
    }

    /// Completed hook cells are flushed out of the active slot once no timers remain.
    pub(crate) fn should_flush(&self) -> bool {
        !self.is_active() && !self.is_empty()
    }

    /// Splits durable completed runs from ephemeral active-cell bookkeeping.
    ///
    /// Quiet successes are left behind so they can disappear from the active cell, while failures,
    /// blocked/stopped hooks, and hooks with emitted output become a persistent history cell.
    pub(crate) fn take_completed_persistent_runs(&mut self) -> Option<Self> {
        let mut completed = Vec::new();
        let mut remaining = Vec::new();
        for run in self.runs.drain(..) {
            if run.state.has_persistent_output() {
                completed.push(run);
            } else {
                remaining.push(run);
            }
        }
        self.runs = remaining;
        (!completed.is_empty()).then_some(Self { runs: completed })
    }

    /// Returns true for revealed runs still participating in lifecycle bookkeeping.
    pub(crate) fn has_visible_running_run(&self) -> bool {
        self.runs.iter().any(|run| run.state.is_running_visible())
    }

    /// Describes revealed hooks without implying per-handler completion progress.
    /// Synchronous completions can arrive together after the whole batch finishes.
    pub(crate) fn running_status_summary(&self) -> Option<String> {
        let mut messages = self
            .runs
            .iter()
            .filter(|run| matches!(run.state, HookRunState::VisibleRunning { .. }))
            .map(|run| {
                run.status_message
                    .as_deref()
                    .map(str::trim)
                    .filter(|message| !message.is_empty())
            })
            .peekable();
        let message = messages.next()?;
        let multiple = messages.peek().is_some();
        Some(
            if let Some(message) = message
                && messages.all(|other| other == Some(message))
            {
                message.to_string()
            } else if multiple {
                "Running hooks".to_string()
            } else {
                "Running hook".to_string()
            },
        )
    }

    /// Advances reveal/removal timers and reports whether rendering should be refreshed.
    pub(crate) fn advance_time(&mut self, now: Instant) -> bool {
        let old_len = self.runs.len();
        let mut changed = false;
        for run in &mut self.runs {
            changed |= run.state.reveal_if_due(now);
        }
        self.runs.retain(|run| !run.state.quiet_linger_expired(now));
        changed || self.runs.len() != old_len
    }

    /// Inserts or refreshes a started hook run.
    ///
    /// A duplicate begin event resets the reveal timer rather than adding a second row, because
    /// matching by id is the invariant that keeps begin/end events paired.
    pub(crate) fn start_run(&mut self, run: HookRunSummary) {
        let now = Instant::now();
        if let Some(existing) = self.runs.iter_mut().find(|existing| existing.id == run.id) {
            existing.status_message = run.status_message;
            existing.state = HookRunState::pending(now);
            return;
        }
        self.runs.push(HookRunCell {
            id: run.id,
            status_message: run.status_message,
            state: HookRunState::pending(now),
        });
    }

    /// Completes a run and returns whether the run was already present in this cell.
    ///
    /// Quiet successes intentionally avoid persistent output. If they were never visible, they
    /// disappear immediately; if they had already drawn, they move into `QuietLinger`.
    pub(crate) fn complete_run(&mut self, run: HookRunSummary) -> bool {
        let Some(index) = self.runs.iter().position(|existing| existing.id == run.id) else {
            return false;
        };
        if hook_run_is_quiet_success(&run) {
            if !self.runs[index]
                .state
                .complete_quiet_success(Instant::now())
            {
                self.runs.remove(index);
            }
            return true;
        }
        let HookRunSummary {
            status_message,
            status,
            entries,
            ..
        } = run;
        let existing = &mut self.runs[index];
        existing.status_message = status_message;
        existing.state = HookRunState::completed(status, entries);
        true
    }

    /// Adds a completed hook that did not pass through this live cell.
    ///
    /// This is used for replay/restoration paths where the final run summary is already known.
    pub(crate) fn add_completed_run(&mut self, run: HookRunSummary) {
        if hook_run_is_quiet_success(&run) {
            return;
        }
        let HookRunSummary {
            id,
            status_message,
            status,
            entries,
            ..
        } = run;
        self.runs.push(HookRunCell {
            id,
            status_message,
            state: HookRunState::completed(status, entries),
        });
    }

    pub(crate) fn next_timer_deadline(&self) -> Option<Instant> {
        self.runs
            .iter()
            .filter_map(|run| run.state.next_timer_deadline())
            .min()
    }

    #[cfg(test)]
    pub(crate) fn expire_quiet_runs_now_for_test(&mut self) {
        for run in &mut self.runs {
            run.expire_quiet_linger_now_for_test();
        }
    }

    #[cfg(test)]
    pub(crate) fn reveal_running_runs_now_for_test(&mut self) {
        let now = Instant::now();
        for run in &mut self.runs {
            run.reveal_running_now_for_test(now);
        }
    }

    #[cfg(test)]
    pub(crate) fn reveal_running_runs_after_delayed_redraw_for_test(&mut self) {
        let now = Instant::now();
        for run in &mut self.runs {
            run.reveal_running_after_delayed_redraw_for_test(now);
        }
    }

    /// Builds only durable completion output; active hooks use the activity summary.
    fn output_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for run in &self.runs {
            let HookRunState::Completed { status, entries } = &run.state else {
                continue;
            };
            if !lines.is_empty() {
                lines.push("".into());
            }
            let system_message = entries
                .iter()
                .find(|entry| entry.kind == HookOutputEntryKind::Warning);
            let mut system_message_lines = system_message.map(|entry| {
                let mut lines = ansi_escape(&entry.text).lines;
                if entry.text.ends_with('\n') {
                    lines.push("".into());
                }
                lines.into_iter()
            });
            if *status == HookRunStatus::Completed
                && let Some(mut first_line) = system_message_lines.as_mut().and_then(Iterator::next)
            {
                first_line.spans.insert(/*index*/ 0, "↳ Hook · ".dim());
                lines.push(first_line);
            } else {
                let header_text = match status {
                    HookRunStatus::Completed => "Hook completed",
                    HookRunStatus::Failed => "Hook failed",
                    HookRunStatus::Blocked => "Blocked by hook",
                    HookRunStatus::Stopped => "Hook stopped",
                    HookRunStatus::Running => "Hook running",
                };
                lines.push(
                    vec![
                        hook_completed_bullet(*status),
                        " ".into(),
                        header_text.into(),
                    ]
                    .into(),
                );
                if let Some(mut first_line) = system_message_lines.as_mut().and_then(Iterator::next)
                {
                    first_line
                        .spans
                        .insert(/*index*/ 0, format!("{HOOK_OUTPUT_INDENT}└ ").into());
                    lines.push(first_line);
                }
            }
            if let Some(system_message_lines) = system_message_lines {
                for mut line in system_message_lines {
                    if line.spans.is_empty() {
                        lines.push("".into());
                    } else {
                        line.spans
                            .insert(/*index*/ 0, HOOK_OUTPUT_BODY_INDENT.into());
                        lines.push(line);
                    }
                }
            }
            for entry in entries {
                if matches!(
                    entry.kind,
                    HookOutputEntryKind::Warning | HookOutputEntryKind::Context
                ) {
                    continue;
                }
                push_full_hook_output_entry(&mut lines, entry);
            }
        }
        lines
    }
}

impl HistoryCell for HookCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.output_lines()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.output_lines())
    }
}

impl HookRunCell {
    #[cfg(test)]
    fn expire_quiet_linger_now_for_test(&mut self) {
        if let HookRunState::QuietLinger {
            removal_deadline, ..
        } = &mut self.state
        {
            *removal_deadline = Instant::now();
        }
    }

    #[cfg(test)]
    fn reveal_running_now_for_test(&mut self, now: Instant) {
        if let HookRunState::PendingReveal {
            reveal_deadline, ..
        } = &mut self.state
        {
            *reveal_deadline = now;
        }
    }

    #[cfg(test)]
    fn reveal_running_after_delayed_redraw_for_test(&mut self, now: Instant) {
        if let HookRunState::PendingReveal {
            reveal_deadline, ..
        } = &mut self.state
        {
            let delayed_deadline = now
                .checked_sub(QUIET_HOOK_MIN_VISIBLE + Duration::from_millis(100))
                .unwrap_or(now);
            *reveal_deadline = delayed_deadline;
        }
    }
}

fn push_full_hook_output_entry(lines: &mut Vec<Line<'static>>, entry: &HookOutputEntry) {
    let mut output_lines = entry.text.split('\n');
    if let Some(first_line) = output_lines.next() {
        lines.push(format!("{HOOK_OUTPUT_INDENT}└ {first_line}").into());
    }
    for line in output_lines {
        if line.is_empty() {
            lines.push("".into());
        } else {
            lines.push(format!("{HOOK_OUTPUT_BODY_INDENT}{line}").into());
        }
    }
}

impl HookRunState {
    /// Creates the hidden initial state for a live hook run.
    fn pending(start_time: Instant) -> Self {
        Self::PendingReveal {
            reveal_deadline: start_time + HOOK_RUN_REVEAL_DELAY,
        }
    }

    /// Creates the persistent final state for a hook with visible output or a notable status.
    fn completed(status: HookRunStatus, entries: Vec<HookOutputEntry>) -> Self {
        Self::Completed { status, entries }
    }

    /// Returns true while the run is still waiting for a completion event or timer cleanup.
    fn is_active(&self) -> bool {
        match self {
            HookRunState::PendingReveal { .. }
            | HookRunState::VisibleRunning { .. }
            | HookRunState::QuietLinger { .. } => true,
            HookRunState::Completed { .. } => false,
        }
    }

    /// Returns true for completed runs that should survive outside the active cell.
    fn has_persistent_output(&self) -> bool {
        match self {
            HookRunState::Completed { status, entries } => {
                *status != HookRunStatus::Completed
                    || entries
                        .iter()
                        .any(|entry| entry.kind != HookOutputEntryKind::Context)
            }
            HookRunState::PendingReveal { .. }
            | HookRunState::VisibleRunning { .. }
            | HookRunState::QuietLinger { .. } => false,
        }
    }

    /// Returns true for revealed runs whose lifecycle bookkeeping is still active.
    fn is_running_visible(&self) -> bool {
        matches!(
            self,
            HookRunState::VisibleRunning { .. } | HookRunState::QuietLinger { .. }
        )
    }

    /// Reveals a pending run once its deadline has passed.
    ///
    /// Returns true only when this call changes the state, allowing timer callbacks to avoid
    /// unnecessary redraws.
    fn reveal_if_due(&mut self, now: Instant) -> bool {
        let HookRunState::PendingReveal { reveal_deadline } = self else {
            return false;
        };
        if now < *reveal_deadline {
            return false;
        }
        *self = HookRunState::VisibleRunning { visible_since: now };
        true
    }

    /// Returns the next state-machine deadline owned by this run.
    fn next_timer_deadline(&self) -> Option<Instant> {
        match self {
            HookRunState::PendingReveal {
                reveal_deadline, ..
            } => Some(*reveal_deadline),
            HookRunState::QuietLinger {
                removal_deadline, ..
            } => Some(*removal_deadline),
            HookRunState::VisibleRunning { .. } | HookRunState::Completed { .. } => None,
        }
    }

    /// Returns true once a quiet success has lingered for long enough.
    fn quiet_linger_expired(&self, now: Instant) -> bool {
        match self {
            HookRunState::QuietLinger {
                removal_deadline, ..
            } => now >= *removal_deadline,
            HookRunState::PendingReveal { .. }
            | HookRunState::VisibleRunning { .. }
            | HookRunState::Completed { .. } => false,
        }
    }

    /// Converts a visible quiet success into a temporary linger state.
    ///
    /// Returns false when the success should be removed immediately: either it was never visible or
    /// it has already stayed visible for the minimum duration.
    fn complete_quiet_success(&mut self, now: Instant) -> bool {
        let HookRunState::VisibleRunning { visible_since, .. } = self else {
            return false;
        };
        let minimum_deadline = *visible_since + QUIET_HOOK_MIN_VISIBLE;
        if now >= minimum_deadline {
            return false;
        }
        *self = HookRunState::QuietLinger {
            removal_deadline: minimum_deadline,
        };
        true
    }
}

pub(crate) fn new_active_hook_cell(run: HookRunSummary) -> HookCell {
    HookCell::new_active(run)
}

pub(crate) fn new_completed_hook_cell(run: HookRunSummary) -> HookCell {
    HookCell::new_completed(run)
}

/// Returns true for hook completions that should be invisible in history.
fn hook_run_is_quiet_success(run: &HookRunSummary) -> bool {
    run.status == HookRunStatus::Completed
        && run
            .entries
            .iter()
            .all(|entry| entry.kind == HookOutputEntryKind::Context)
}

fn hook_completed_bullet(status: HookRunStatus) -> Span<'static> {
    match status {
        HookRunStatus::Completed => "•".green().bold(),
        HookRunStatus::Blocked | HookRunStatus::Failed | HookRunStatus::Stopped => "•".red().bold(),
        HookRunStatus::Running => "•".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::PathBufExt;
    use crate::test_support::test_path_buf;
    use codex_app_server_protocol::HookEventName;
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;
    use ratatui::style::Modifier;

    #[test]
    fn completed_hook_system_message_uses_dim_provenance_and_hides_context() {
        let cell = completed_hook_cell(
            HookEventName::SessionStart,
            HookRunStatus::Completed,
            vec![
                HookOutputEntry {
                    kind: HookOutputEntryKind::Warning,
                    text: "Heads up from the hook".to_string(),
                },
                HookOutputEntry {
                    kind: HookOutputEntryKind::Context,
                    text: "Private model-facing project instructions".to_string(),
                },
            ],
        );
        let lines = cell.display_lines(/*width*/ 80);
        let expected = vec!["↳ Hook · Heads up from the hook".to_string()];

        assert_eq!(line_texts(&lines), expected);
        assert_eq!(line_texts(&cell.transcript_lines(/*width*/ 80)), expected);
        assert_eq!(line_texts(&cell.raw_lines()), expected);
        assert!(lines[0].spans[0].style.add_modifier.contains(Modifier::DIM));
        assert_eq!(lines[0].spans[1].style, Style::default());
    }

    #[test]
    fn completed_hook_system_message_renders_ansi_styles() {
        let cell = completed_hook_cell(
            HookEventName::UserPromptSubmit,
            HookRunStatus::Completed,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Warning,
                text: "\x1b[1;92mStyled output\nacross lines\x1b[0m plain".to_string(),
            }],
        );

        let lines = cell.display_lines(/*width*/ 80);

        assert_eq!(
            line_texts(&cell.raw_lines()),
            vec![
                "↳ Hook · Styled output".to_string(),
                "    across lines plain".to_string(),
            ],
        );
        insta::assert_debug_snapshot!("completed_hook_system_message_ansi_styles", lines);
    }

    #[test]
    fn completed_hook_with_only_context_is_quiet_on_every_tui_surface() {
        let cell = completed_hook_cell(
            HookEventName::SessionStart,
            HookRunStatus::Completed,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Context,
                text: "## Working Memory Recall\n\nSource: Codex compaction".to_string(),
            }],
        );

        assert!(cell.is_empty());
        assert!(cell.display_lines(/*width*/ 80).is_empty());
        assert!(cell.transcript_lines(/*width*/ 80).is_empty());
        assert!(cell.raw_lines().is_empty());
    }

    #[test]
    fn running_hook_summary_uses_only_revealed_messages() {
        for (messages, expected) in [
            (vec![None], "Running hook"),
            (vec![Some("   ")], "Running hook"),
            (vec![None, None], "Running hooks"),
            (vec![Some("checking policy")], "checking policy"),
            (
                vec![Some("  checking policy  "), Some("checking policy")],
                "checking policy",
            ),
            (vec![Some("checking policy"), None], "Running hooks"),
            (vec![None, Some("checking policy")], "Running hooks"),
            (
                vec![Some("checking policy"), Some("scanning secrets")],
                "Running hooks",
            ),
        ] {
            let mut first = hook_run_summary("0");
            first.status_message = messages[0].map(str::to_string);
            let mut cell = HookCell::new_active(first);
            for (index, message) in messages.iter().enumerate().skip(1) {
                let mut run = hook_run_summary(&index.to_string());
                run.status_message = message.map(str::to_string);
                cell.start_run(run);
            }
            assert_eq!(cell.running_status_summary(), None);
            cell.reveal_running_runs_now_for_test();
            cell.advance_time(Instant::now());
            assert_eq!(cell.running_status_summary().as_deref(), Some(expected));

            // A new hook must not change the summary before its own reveal delay.
            let mut fast = hook_run_summary("fast");
            fast.status_message = Some("another message".to_string());
            cell.start_run(fast.clone());
            assert_eq!(cell.running_status_summary().as_deref(), Some(expected));
            fast.status = HookRunStatus::Completed;
            cell.complete_run(fast);
            assert_eq!(cell.running_status_summary().as_deref(), Some(expected));

            // Synchronous completion notifications arrive after the batch finishes.
            for index in 0..messages.len() {
                let mut completed = hook_run_summary(&index.to_string());
                completed.status = HookRunStatus::Completed;
                cell.complete_run(completed);
            }
            assert_eq!(cell.running_status_summary(), None);
        }
    }

    #[test]
    fn completed_hook_non_context_entries_are_not_truncated() {
        for kind in [
            HookOutputEntryKind::Warning,
            HookOutputEntryKind::Stop,
            HookOutputEntryKind::Feedback,
            HookOutputEntryKind::Error,
        ] {
            let cell = completed_hook_cell(
                HookEventName::UserPromptSubmit,
                HookRunStatus::Stopped,
                vec![HookOutputEntry {
                    kind,
                    text: "first\nsecond\nthird\nfourth\nfifth".to_string(),
                }],
            );

            assert_eq!(
                line_texts(&cell.display_lines(/*width*/ 20)).join("\n"),
                "• Hook stopped\n  └ first\n    second\n    third\n    fourth\n    fifth",
                "expected {kind:?} output to remain complete",
            );
        }
    }

    #[test]
    fn unsuccessful_hooks_use_bold_red_bullets_and_actionable_details() {
        for (status, expected_header) in [
            (HookRunStatus::Failed, "• Hook failed"),
            (HookRunStatus::Blocked, "• Blocked by hook"),
            (HookRunStatus::Stopped, "• Hook stopped"),
        ] {
            let detail = "Policy prevented this action.";
            let cell = completed_hook_cell(
                HookEventName::PreToolUse,
                status,
                vec![HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: detail.to_string(),
                }],
            );
            let lines = cell.display_lines(/*width*/ 80);

            assert_eq!(
                line_texts(&lines),
                vec![expected_header.to_string(), format!("  └ {detail}")],
            );
            assert_eq!(lines[0].spans[0].style.fg, Some(Color::Red));
            assert!(
                lines[0].spans[0]
                    .style
                    .add_modifier
                    .contains(Modifier::BOLD)
            );
        }
    }

    #[test]
    fn completed_stop_hook_multiline_system_message_prefixes_first_line_only() {
        let cell = completed_hook_cell(
            HookEventName::Stop,
            HookRunStatus::Completed,
            vec![HookOutputEntry {
                kind: HookOutputEntryKind::Warning,
                text: "Heads up\n\u{301}\nReview generated files\n".to_string(),
            }],
        );

        assert_eq!(
            line_texts(&cell.display_lines(/*width*/ 80)),
            vec![
                "↳ Hook · Heads up".to_string(),
                "    \u{301}".to_string(),
                "    Review generated files".to_string(),
                "".to_string(),
            ]
        );
    }

    fn completed_hook_cell(
        event_name: HookEventName,
        status: HookRunStatus,
        entries: Vec<HookOutputEntry>,
    ) -> HookCell {
        let mut run = hook_run_summary("hook-1");
        run.event_name = event_name;
        run.status = status;
        run.status_message = None;
        run.completed_at = Some(2);
        run.duration_ms = Some(1);
        run.entries = entries;
        HookCell::new_completed(run)
    }

    fn line_texts(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(line_text).collect()
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    fn hook_run_summary(id: &str) -> HookRunSummary {
        HookRunSummary {
            id: id.to_string(),
            event_name: HookEventName::PostToolUse,
            handler_type: codex_app_server_protocol::HookHandlerType::Command,
            execution_mode: codex_app_server_protocol::HookExecutionMode::Sync,
            scope: codex_app_server_protocol::HookScope::Turn,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: codex_app_server_protocol::HookSource::User,
            display_order: 0,
            status: HookRunStatus::Running,
            status_message: Some("checking output policy".to_string()),
            started_at: 1,
            completed_at: None,
            duration_ms: None,
            entries: Vec::new(),
        }
    }
}
