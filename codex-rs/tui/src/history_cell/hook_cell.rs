//! History cell for hook execution.
//!
//! Hooks are intentionally quieter than normal tool calls. A hook that starts and finishes
//! successfully without output should not leave a transcript artifact, and very fast hooks should
//! not flash in the viewport. This cell keeps that policy local by treating each hook run as a
//! small rendering state machine:
//!
//! 1. New runs begin hidden in `PendingReveal`.
//! 2. Runs that outlive the reveal delay become visible and may be coalesced with adjacent runs.
//! 3. Visible quiet successes linger briefly so they do not disappear in the same frame they were
//!    first drawn.
//! 4. Completed runs only persist when they have output or a non-success status.
use super::HistoryCell;
use crate::motion::MotionMode;
use crate::motion::ReducedMotionIndicator;
use crate::motion::activity_indicator;
use crate::motion::shimmer_text;
use crate::render::renderable::Renderable;
use codex_app_server_protocol::HookEventName;
use codex_app_server_protocol::HookOutputEntry;
use codex_app_server_protocol::HookOutputEntryKind;
use codex_app_server_protocol::HookRunStatus;
use codex_app_server_protocol::HookRunSummary;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug)]
pub(crate) struct HookCell {
    /// Hook runs that are active, lingering, or have persistent output to render.
    runs: Vec<HookRunCell>,
    /// Mirrors the global animation setting so transcript rendering and viewport rendering agree.
    animations_enabled: bool,
}

/// Minimum runtime before a hook is allowed to draw.
///
/// Helps avoids a flash of text forwork that was effectively instant.
const HOOK_RUN_REVEAL_DELAY: Duration = Duration::from_millis(300);

/// Minimum time a quiet success remains on screen after becoming visible.
///
/// This pairs with `HOOK_RUN_REVEAL_DELAY`: once the user has seen a hook row, keep it stable long
/// enough to read instead of removing it immediately when the success event arrives.
const QUIET_HOOK_MIN_VISIBLE: Duration = Duration::from_millis(600);

#[derive(Debug)]
struct HookRunCell {
    /// Stable protocol id used to match begin/end updates for the same hook invocation.
    id: String,
    /// Hook event kind, kept outside `state` so a begin update can refresh metadata in place.
    event_name: HookEventName,
    /// Optional hook-supplied detail shown next to the running header.
    status_message: Option<String>,
    /// Rendering lifecycle for this run.
    state: HookRunState,
}

#[derive(Debug)]
enum HookRunState {
    /// A newly-started run that is active but deliberately hidden until `reveal_deadline`.
    PendingReveal {
        /// The original start time, used for spinner phase and grouping once revealed.
        start_time: Instant,
        /// First instant at which the run may become visible.
        reveal_deadline: Instant,
    },
    /// A run that survived the reveal delay and is currently shown as running.
    VisibleRunning {
        /// The original start time, used to keep animation timing stable across transitions.
        start_time: Instant,
        /// First instant the run was actually rendered, used by quiet-success linger.
        visible_since: Instant,
    },
    /// A visible run that completed successfully without output but is still lingering briefly.
    QuietLinger {
        /// The original start time, retained so the spinner does not jump during the linger frame.
        start_time: Instant,
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

#[derive(Debug, PartialEq, Eq)]
struct RunningHookGroupKey {
    event_name: HookEventName,
    status_message: Option<String>,
}

/// Accumulator for adjacent running hooks that can share one status line.
///
/// Grouping happens only while building display lines, the underlying runs stay separate so their
/// protocol ids and completion transitions remain independent.
struct RunningHookGroup {
    /// Shared event/status pair for every run in this display group.
    key: RunningHookGroupKey,
    /// Earliest start time in the group, so the combined spinner reflects the oldest work.
    start_time: Option<Instant>,
    /// Number of adjacent runs represented by the group line.
    count: usize,
}

impl HookCell {
    /// Creates a cell around a hook that has just started.
    fn new_active(run: HookRunSummary, animations_enabled: bool) -> Self {
        let mut cell = Self {
            runs: Vec::new(),
            animations_enabled,
        };
        cell.start_run(run);
        cell
    }

    /// Creates a cell around an already-completed hook from transcript/history data.
    fn new_completed(run: HookRunSummary, animations_enabled: bool) -> Self {
        let mut cell = Self {
            runs: Vec::new(),
            animations_enabled,
        };
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

    /// Returns whether this cell has at least one line worth drawing right now.
    pub(crate) fn should_render(&self) -> bool {
        self.runs.iter().any(|run| run.state.should_render())
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
        (!completed.is_empty()).then_some(Self {
            runs: completed,
            animations_enabled: self.animations_enabled,
        })
    }

    /// Used by callers that need to know whether the active cell currently occupies viewport space.
    pub(crate) fn has_visible_running_run(&self) -> bool {
        self.runs.iter().any(|run| run.state.is_running_visible())
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
            existing.event_name = run.event_name;
            existing.status_message = run.status_message;
            existing.state = HookRunState::pending(now);
            return;
        }
        self.runs.push(HookRunCell {
            id: run.id,
            event_name: run.event_name,
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
            event_name,
            status_message,
            status,
            entries,
            ..
        } = run;
        let existing = &mut self.runs[index];
        existing.event_name = event_name;
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
            event_name,
            status_message,
            status,
            entries,
            ..
        } = run;
        self.runs.push(HookRunCell {
            id,
            event_name,
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
}

impl HistoryCell for HookCell {
    /// Builds viewport lines while coalescing adjacent visible-running hooks.
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let mut running_group: Option<RunningHookGroup> = None;
        for run in &self.runs {
            if !run.state.should_render() {
                continue;
            }

            let Some(key) = run.running_group_key() else {
                // Completed runs keep their own output lines, so any pending running group must be
                // emitted before drawing the completed run.
                if let Some(group) = running_group.take() {
                    push_running_hook_group(&mut lines, &group, self.animations_enabled);
                }
                push_hook_line_separator(&mut lines);
                run.push_display_lines(&mut lines, self.animations_enabled);
                continue;
            };

            if let Some(group) = running_group.as_mut()
                && group.key == key
            {
                group.count += 1;
                // Preserve the earliest start time so grouped spinners do not reset when a later
                // adjacent hook is folded into the same line.
                group.start_time = earliest_instant(group.start_time, run.state.start_time());
                continue;
            }

            if let Some(group) =
                running_group.replace(RunningHookGroup::new(key, run.state.start_time()))
            {
                push_running_hook_group(&mut lines, &group, self.animations_enabled);
            }
        }
        if let Some(group) = running_group {
            push_running_hook_group(&mut lines, &group, self.animations_enabled);
        }
        lines
    }

    /// Hook transcript output matches viewport output.
    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.display_lines(width)
    }

    /// Produces a coarse cache key for transcript overlays while hook animations are active.
    fn transcript_animation_tick(&self) -> Option<u64> {
        if !self.animations_enabled {
            return None;
        }
        let elapsed = self
            .runs
            .iter()
            .filter(|run| run.state.is_running_visible())
            .find_map(|run| run.state.start_time())?
            .elapsed();
        Some(elapsed.as_millis() as u64 / 600)
    }
}

impl Renderable for HookCell {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let lines = self.display_lines(area.width);
        let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
        paragraph.render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        HistoryCell::desired_height(self, width)
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

    /// Returns the grouping key only for states that render as running.
    fn running_group_key(&self) -> Option<RunningHookGroupKey> {
        self.state
            .is_running_visible()
            .then(|| RunningHookGroupKey {
                event_name: self.event_name,
                status_message: self.status_message.clone(),
            })
    }

    /// Appends the lines for a single, ungrouped hook run.
    fn push_display_lines(&self, lines: &mut Vec<Line<'static>>, animations_enabled: bool) {
        let label = hook_event_label(self.event_name);
        match &self.state {
            HookRunState::VisibleRunning { start_time, .. }
            | HookRunState::QuietLinger { start_time, .. } => {
                let hook_text = format!("Running {label} hook");
                push_running_hook_header(
                    lines,
                    &hook_text,
                    Some(*start_time),
                    self.status_message.as_deref(),
                    animations_enabled,
                );
            }
            HookRunState::Completed { status, entries } => {
                let status_text = format!("{status:?}").to_lowercase();
                let bullet = hook_completed_bullet(*status, entries);
                lines.push(
                    vec![
                        bullet,
                        " ".into(),
                        format!("{label} hook ({status_text})").into(),
                    ]
                    .into(),
                );
                for entry in entries {
                    // Output entries are already short hook-authored strings; keep their prefixes
                    // explicit so warnings/stops/errors remain easy to scan in history.
                    lines
                        .push(format!("  {}{}", hook_output_prefix(entry.kind), entry.text).into());
                }
            }
            HookRunState::PendingReveal { .. } => {}
        }
    }
}

impl HookRunState {
    /// Creates the hidden initial state for a live hook run.
    fn pending(start_time: Instant) -> Self {
        Self::PendingReveal {
            start_time,
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

    /// Returns true when this run contributes at least one line to the current render.
    fn should_render(&self) -> bool {
        match self {
            HookRunState::VisibleRunning { .. }
            | HookRunState::QuietLinger { .. }
            | HookRunState::Completed { .. } => true,
            HookRunState::PendingReveal { .. } => false,
        }
    }

    /// Returns true for completed runs that should survive outside the active cell.
    fn has_persistent_output(&self) -> bool {
        match self {
            HookRunState::Completed { status, entries } => {
                *status != HookRunStatus::Completed || !entries.is_empty()
            }
            HookRunState::PendingReveal { .. }
            | HookRunState::VisibleRunning { .. }
            | HookRunState::QuietLinger { .. } => false,
        }
    }

    /// Returns the original start time for active states.
    ///
    /// Completed runs no longer animate, so they intentionally have no start time.
    fn start_time(&self) -> Option<Instant> {
        match self {
            HookRunState::PendingReveal { start_time, .. }
            | HookRunState::VisibleRunning { start_time, .. }
            | HookRunState::QuietLinger { start_time, .. } => Some(*start_time),
            HookRunState::Completed { .. } => None,
        }
    }

    /// Returns true when the run should be treated as an in-progress row.
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
        let HookRunState::PendingReveal {
            start_time,
            reveal_deadline,
        } = self
        else {
            return false;
        };
        if now < *reveal_deadline {
            return false;
        }
        *self = HookRunState::VisibleRunning {
            start_time: *start_time,
            visible_since: now,
        };
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
        let HookRunState::VisibleRunning {
            start_time,
            visible_since,
            ..
        } = self
        else {
            return false;
        };
        let start_time = *start_time;
        let minimum_deadline = *visible_since + QUIET_HOOK_MIN_VISIBLE;
        if now >= minimum_deadline {
            return false;
        }
        *self = HookRunState::QuietLinger {
            start_time,
            removal_deadline: minimum_deadline,
        };
        true
    }
}

impl RunningHookGroup {
    fn new(key: RunningHookGroupKey, start_time: Option<Instant>) -> Self {
        Self {
            key,
            start_time,
            count: 1,
        }
    }
}

/// Emits one grouped running-hook status row.
fn push_running_hook_group(
    lines: &mut Vec<Line<'static>>,
    group: &RunningHookGroup,
    animations_enabled: bool,
) {
    push_hook_line_separator(lines);
    let label = hook_event_label(group.key.event_name);
    let hook_text = if group.count == 1 {
        format!("Running {label} hook")
    } else {
        format!("Running {} {label} hooks", group.count)
    };
    push_running_hook_header(
        lines,
        &hook_text,
        group.start_time,
        group.key.status_message.as_deref(),
        animations_enabled,
    );
}

/// Emits the animated or static header used by all running hook rows.
fn push_running_hook_header(
    lines: &mut Vec<Line<'static>>,
    hook_text: &str,
    start_time: Option<Instant>,
    status_message: Option<&str>,
    animations_enabled: bool,
) {
    let mut header = Vec::new();
    let motion_mode = MotionMode::from_animations_enabled(animations_enabled);
    if let Some(indicator) =
        activity_indicator(start_time, motion_mode, ReducedMotionIndicator::Hidden)
    {
        header.push(indicator);
        header.push(" ".into());
    }
    header.extend(shimmer_text(hook_text, motion_mode));
    if !animations_enabled && let Some(span) = header.last_mut() {
        span.style = span.style.patch(Style::default().bold());
    }
    if let Some(status_message) = status_message
        && !status_message.is_empty()
    {
        header.push(": ".into());
        header.push(status_message.to_string().dim());
    }
    lines.push(header.into());
}

/// Adds a blank separator between hook blocks without leaving a leading blank line.
fn push_hook_line_separator(lines: &mut Vec<Line<'static>>) {
    if !lines.is_empty() {
        lines.push("".into());
    }
}

/// Combines optional instants while preserving the earliest known start time.
fn earliest_instant(left: Option<Instant>, right: Option<Instant>) -> Option<Instant> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

pub(crate) fn new_active_hook_cell(run: HookRunSummary, animations_enabled: bool) -> HookCell {
    HookCell::new_active(run, animations_enabled)
}

pub(crate) fn new_completed_hook_cell(run: HookRunSummary, animations_enabled: bool) -> HookCell {
    HookCell::new_completed(run, animations_enabled)
}

/// Returns true for hook completions that should be invisible in history.
fn hook_run_is_quiet_success(run: &HookRunSummary) -> bool {
    run.status == HookRunStatus::Completed && run.entries.is_empty()
}

fn hook_completed_bullet(status: HookRunStatus, entries: &[HookOutputEntry]) -> Span<'static> {
    match status {
        HookRunStatus::Completed => {
            if entries
                .iter()
                .any(|entry| entry.kind == HookOutputEntryKind::Warning)
            {
                "•".bold()
            } else {
                "•".green().bold()
            }
        }
        HookRunStatus::Blocked | HookRunStatus::Failed | HookRunStatus::Stopped => "•".red().bold(),
        HookRunStatus::Running => "•".into(),
    }
}

fn hook_output_prefix(kind: HookOutputEntryKind) -> &'static str {
    match kind {
        HookOutputEntryKind::Warning => "warning: ",
        HookOutputEntryKind::Stop => "stop: ",
        HookOutputEntryKind::Feedback => "feedback: ",
        HookOutputEntryKind::Context => "hook context: ",
        HookOutputEntryKind::Error => "error: ",
    }
}

fn hook_event_label(event_name: HookEventName) -> &'static str {
    match event_name {
        HookEventName::PreToolUse => "PreToolUse",
        HookEventName::PermissionRequest => "PermissionRequest",
        HookEventName::PostToolUse => "PostToolUse",
        HookEventName::SessionStart => "SessionStart",
        HookEventName::UserPromptSubmit => "UserPromptSubmit",
        HookEventName::Stop => "Stop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::PathBufExt;
    use crate::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use ratatui::style::Modifier;

    #[test]
    fn completed_hook_with_warning_uses_default_bold_bullet() {
        let entries = vec![HookOutputEntry {
            kind: HookOutputEntryKind::Warning,
            text: "Heads up from the hook".to_string(),
        }];

        let bullet = hook_completed_bullet(HookRunStatus::Completed, &entries);

        assert_eq!(bullet.content.as_ref(), "•");
        assert_eq!(bullet.style.fg, None);
        assert!(bullet.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn pending_hook_does_not_animate_transcript() {
        let cell =
            HookCell::new_active(hook_run_summary("hook-1"), /*animations_enabled*/ true);

        assert_eq!(cell.transcript_animation_tick(), None);
    }

    #[test]
    fn visible_hook_animates_transcript_when_animations_enabled() {
        let mut cell =
            HookCell::new_active(hook_run_summary("hook-1"), /*animations_enabled*/ true);
        cell.reveal_running_runs_now_for_test();
        cell.advance_time(Instant::now());

        assert_eq!(cell.transcript_animation_tick(), Some(0));
    }

    #[test]
    fn visible_hook_does_not_animate_transcript_when_animations_disabled() {
        let mut cell = HookCell::new_active(
            hook_run_summary("hook-1"),
            /*animations_enabled*/ false,
        );
        cell.reveal_running_runs_now_for_test();
        cell.advance_time(Instant::now());

        assert_eq!(cell.transcript_animation_tick(), None);
    }

    #[test]
    fn visible_hook_without_animations_omits_spinner() {
        let mut cell = HookCell::new_active(
            hook_run_summary("hook-1"),
            /*animations_enabled*/ false,
        );
        cell.reveal_running_runs_now_for_test();
        cell.advance_time(Instant::now());

        let rendered: Vec<String> = cell
            .display_lines(/*width*/ 80)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect();

        assert_eq!(
            rendered,
            vec!["Running PostToolUse hook: checking output policy".to_string()]
        );
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
