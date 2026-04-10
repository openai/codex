use super::HistoryCell;
use crate::exec_cell::spinner;
use crate::render::renderable::Renderable;
use crate::shimmer::shimmer_spans;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug)]
pub(crate) struct HookCell {
    runs: Vec<HookRunCell>,
    animations_enabled: bool,
}

const HOOK_RUN_REVEAL_DELAY: Duration = Duration::from_millis(300);
const QUIET_HOOK_MIN_VISIBLE: Duration = Duration::from_millis(600);

#[derive(Debug)]
struct HookRunCell {
    id: String,
    event_name: HookEventName,
    status_message: Option<String>,
    state: HookRunState,
}

#[derive(Debug)]
enum HookRunState {
    PendingReveal {
        start_time: Instant,
        reveal_deadline: Instant,
    },
    Running {
        start_time: Instant,
        reveal_deadline: Instant,
    },
    QuietLinger {
        start_time: Instant,
        removal_deadline: Instant,
    },
    Completed(CompletedHookRun),
}

#[derive(Debug)]
struct CompletedHookRun {
    status: HookRunStatus,
    entries: Vec<HookOutputEntry>,
}

#[derive(Debug, PartialEq, Eq)]
struct RunningHookGroupKey {
    event_name: HookEventName,
    status_message: Option<String>,
}

struct RunningHookGroup {
    key: RunningHookGroupKey,
    start_time: Option<Instant>,
    count: usize,
}

impl HookCell {
    pub(crate) fn new(run: HookRunSummary, animations_enabled: bool) -> Self {
        let mut cell = Self {
            runs: Vec::new(),
            animations_enabled,
        };
        cell.start_run(run);
        cell
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    pub(crate) fn is_active(&self) -> bool {
        self.runs.iter().any(|run| run.state.is_active())
    }

    pub(crate) fn should_flush(&self) -> bool {
        !self.is_active() && !self.is_empty()
    }

    pub(crate) fn should_render(&self) -> bool {
        self.runs.iter().any(|run| run.state.should_render())
    }

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

    pub(crate) fn has_visible_running_run(&self) -> bool {
        self.runs.iter().any(|run| run.state.is_running_visible())
    }

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
    pub(crate) fn complete_run(&mut self, run: HookRunSummary) -> bool {
        let Some(index) = self.runs.iter().position(|existing| existing.id == run.id) else {
            return false;
        };
        if hook_run_is_quiet_success(&run) {
            if self.runs[index].start_quiet_linger_after_success() {
                return true;
            } else {
                self.runs.remove(index);
            }
            return true;
        }
        let existing = &mut self.runs[index];
        existing.event_name = run.event_name;
        existing.status_message = run.status_message;
        existing.state = HookRunState::Completed(CompletedHookRun {
            status: run.status,
            entries: run.entries,
        });
        true
    }

    pub(crate) fn add_completed_run(&mut self, run: HookRunSummary) {
        if hook_run_is_quiet_success(&run) {
            return;
        }
        self.runs.push(HookRunCell {
            id: run.id,
            event_name: run.event_name,
            status_message: run.status_message,
            state: HookRunState::Completed(CompletedHookRun {
                status: run.status,
                entries: run.entries,
            }),
        });
    }

    pub(crate) fn prune_expired_quiet_runs(&mut self, now: Instant) -> bool {
        let old_len = self.runs.len();
        self.runs.retain(|run| !run.state.quiet_linger_expired(now));
        self.runs.len() != old_len
    }

    pub(crate) fn update_due_visibility(&mut self, now: Instant) -> bool {
        let mut changed = false;
        for run in &mut self.runs {
            if run.state.reveal_if_due(now) {
                changed = true;
            }
        }
        changed
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

    fn display_lines_inner(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let mut running_group: Option<RunningHookGroup> = None;
        for run in &self.runs {
            if !run.state.should_render() {
                continue;
            }
            if let Some(key) = run.running_group_key() {
                match running_group.as_mut() {
                    Some(group) if group.key == key => {
                        group.count += 1;
                        group.start_time =
                            earliest_instant(group.start_time, run.state.start_time());
                    }
                    Some(group) => {
                        push_running_hook_group(&mut lines, group, self.animations_enabled);
                        running_group = Some(RunningHookGroup::new(key, run.state.start_time()));
                    }
                    None => {
                        running_group = Some(RunningHookGroup::new(key, run.state.start_time()));
                    }
                }
                continue;
            }
            if let Some(group) = running_group.take() {
                push_running_hook_group(&mut lines, &group, self.animations_enabled);
            }
            push_hook_line_separator(&mut lines);
            run.push_display_lines(&mut lines, self.animations_enabled);
        }
        if let Some(group) = running_group {
            push_running_hook_group(&mut lines, &group, self.animations_enabled);
        }
        lines
    }
}

impl HistoryCell for HookCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.display_lines_inner()
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.display_lines(width)
    }

    fn transcript_animation_tick(&self) -> Option<u64> {
        let elapsed = self
            .runs
            .iter()
            .find_map(|run| {
                run.state
                    .is_active()
                    .then(|| run.state.start_time())
                    .flatten()
            })?
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
    fn start_quiet_linger_after_success(&mut self) -> bool {
        let Some((start_time, removal_deadline)) = self.state.quiet_linger_after_success() else {
            return false;
        };
        self.state = HookRunState::QuietLinger {
            start_time,
            removal_deadline,
        };
        true
    }

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

    fn running_group_key(&self) -> Option<RunningHookGroupKey> {
        self.state
            .is_running_visible()
            .then(|| RunningHookGroupKey {
                event_name: self.event_name,
                status_message: self.status_message.clone(),
            })
    }

    fn push_display_lines(&self, lines: &mut Vec<Line<'static>>, animations_enabled: bool) {
        let label = hook_event_label(self.event_name);
        match &self.state {
            HookRunState::Running { start_time, .. }
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
            HookRunState::Completed(completed) => {
                let status = format!("{:?}", completed.status).to_lowercase();
                let bullet = hook_completed_bullet(completed);
                lines.push(
                    vec![
                        bullet,
                        " ".into(),
                        format!("{label} hook ({status})").into(),
                    ]
                    .into(),
                );
                for entry in &completed.entries {
                    lines
                        .push(format!("  {}{}", hook_output_prefix(entry.kind), entry.text).into());
                }
            }
            HookRunState::PendingReveal { .. } => {}
        }
    }
}

impl HookRunState {
    fn pending(start_time: Instant) -> Self {
        Self::PendingReveal {
            start_time,
            reveal_deadline: start_time + HOOK_RUN_REVEAL_DELAY,
        }
    }

    fn is_active(&self) -> bool {
        match self {
            HookRunState::PendingReveal { .. }
            | HookRunState::Running { .. }
            | HookRunState::QuietLinger { .. } => true,
            HookRunState::Completed(_) => false,
        }
    }

    fn should_render(&self) -> bool {
        match self {
            HookRunState::Running { .. }
            | HookRunState::QuietLinger { .. }
            | HookRunState::Completed(_) => true,
            HookRunState::PendingReveal { .. } => false,
        }
    }

    fn has_persistent_output(&self) -> bool {
        match self {
            HookRunState::Completed(completed) => {
                completed.status != HookRunStatus::Completed || !completed.entries.is_empty()
            }
            HookRunState::PendingReveal { .. }
            | HookRunState::Running { .. }
            | HookRunState::QuietLinger { .. } => false,
        }
    }

    fn start_time(&self) -> Option<Instant> {
        match self {
            HookRunState::PendingReveal { start_time, .. }
            | HookRunState::Running { start_time, .. }
            | HookRunState::QuietLinger { start_time, .. } => Some(*start_time),
            HookRunState::Completed(_) => None,
        }
    }

    fn is_running_visible(&self) -> bool {
        matches!(
            self,
            HookRunState::Running { .. } | HookRunState::QuietLinger { .. }
        )
    }

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
        *self = HookRunState::Running {
            start_time: *start_time,
            reveal_deadline: *reveal_deadline,
        };
        true
    }

    fn next_timer_deadline(&self) -> Option<Instant> {
        match self {
            HookRunState::PendingReveal {
                reveal_deadline, ..
            } => Some(*reveal_deadline),
            HookRunState::QuietLinger {
                removal_deadline, ..
            } => Some(*removal_deadline),
            HookRunState::Running { .. } | HookRunState::Completed(_) => None,
        }
    }

    fn quiet_linger_expired(&self, now: Instant) -> bool {
        match self {
            HookRunState::QuietLinger {
                removal_deadline, ..
            } => now >= *removal_deadline,
            HookRunState::PendingReveal { .. }
            | HookRunState::Running { .. }
            | HookRunState::Completed(_) => false,
        }
    }

    fn quiet_linger_after_success(&self) -> Option<(Instant, Instant)> {
        let HookRunState::Running {
            start_time,
            reveal_deadline,
            ..
        } = self
        else {
            return None;
        };
        let minimum_deadline = *reveal_deadline + QUIET_HOOK_MIN_VISIBLE;
        (Instant::now() < minimum_deadline).then_some((*start_time, minimum_deadline))
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

fn push_running_hook_header(
    lines: &mut Vec<Line<'static>>,
    hook_text: &str,
    start_time: Option<Instant>,
    status_message: Option<&str>,
    animations_enabled: bool,
) {
    let mut header = vec![spinner(start_time, animations_enabled), " ".into()];
    if animations_enabled {
        header.extend(shimmer_spans(hook_text));
    } else {
        header.push(hook_text.to_string().bold());
    }
    if let Some(status_message) = status_message
        && !status_message.is_empty()
    {
        header.push(": ".into());
        header.push(status_message.to_string().dim());
    }
    lines.push(header.into());
}

fn push_hook_line_separator(lines: &mut Vec<Line<'static>>) {
    if !lines.is_empty() {
        lines.push("".into());
    }
}

fn earliest_instant(left: Option<Instant>, right: Option<Instant>) -> Option<Instant> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

pub(crate) fn new_active_hook_cell(run: HookRunSummary, animations_enabled: bool) -> HookCell {
    HookCell::new(run, animations_enabled)
}

pub(crate) fn new_completed_hook_cell(run: HookRunSummary, animations_enabled: bool) -> HookCell {
    let mut cell = HookCell {
        runs: Vec::new(),
        animations_enabled,
    };
    cell.add_completed_run(run);
    cell
}

fn hook_run_is_quiet_success(run: &HookRunSummary) -> bool {
    run.status == HookRunStatus::Completed && run.entries.is_empty()
}

fn hook_completed_bullet(completed: &CompletedHookRun) -> Span<'static> {
    match completed.status {
        HookRunStatus::Completed => {
            if completed
                .entries
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
        HookEventName::PostToolUse => "PostToolUse",
        HookEventName::SessionStart => "SessionStart",
        HookEventName::UserPromptSubmit => "UserPromptSubmit",
        HookEventName::Stop => "Stop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Modifier;

    #[test]
    fn completed_hook_with_warning_uses_default_bold_bullet() {
        let completed = CompletedHookRun {
            status: HookRunStatus::Completed,
            entries: vec![HookOutputEntry {
                kind: HookOutputEntryKind::Warning,
                text: "Heads up from the hook".to_string(),
            }],
        };

        let bullet = hook_completed_bullet(&completed);

        assert_eq!(bullet.content.as_ref(), "•");
        assert_eq!(bullet.style.fg, None);
        assert!(bullet.style.add_modifier.contains(Modifier::BOLD));
    }
}
