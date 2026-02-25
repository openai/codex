//! Orchestrates commit-tick drains across streaming controllers.
//!
//! This module bridges queue-based chunking policy (`chunking`) with the concrete stream
//! controllers (`controller`). Callers provide the current controllers and tick scope; the module
//! computes queue pressure, selects a drain plan, applies it, and returns emitted history cells.
//!
//! The module preserves ordering by draining only from controller queue heads. It does not schedule
//! ticks and it does not mutate UI state directly; callers remain responsible for animation events
//! and history insertion side effects.
//!
//! The main flow is:
//! [`run_commit_tick`] -> [`stream_queue_snapshot`] -> [`QueueSnapshot`] ->
//! [`resolve_chunking_plan`] -> [`ChunkingDecision`]/[`DrainPlan`] ->
//! [`apply_commit_tick_plan`] -> [`CommitTickOutput`].

use std::time::Duration;
use std::time::Instant;

use crate::history_cell::HistoryCell;

use super::chunking::AdaptiveChunkingPolicy;
use super::chunking::ChunkingDecision;
use super::chunking::ChunkingMode;
use super::chunking::DrainPlan;
use super::chunking::QueueSnapshot;
use super::controller::PlanStreamController;
use super::controller::StreamController;

/// Describes whether a commit tick may run in all modes or only in catch-up mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommitTickScope {
    /// Always run the tick, regardless of current chunking mode.
    AnyMode,
    /// Run model transitions and policy updates, but commit lines only in `CatchUp`.
    CatchUpOnly,
}

/// Describes what a single commit tick produced.
pub(crate) struct CommitTickOutput {
    /// Cells produced by drained stream lines during this tick.
    pub(crate) cells: Vec<Box<dyn HistoryCell>>,
    /// Whether at least one stream controller was present for this tick.
    pub(crate) has_controller: bool,
    /// Whether all present controllers were idle after this tick.
    pub(crate) all_idle: bool,
}

impl Default for CommitTickOutput {
    /// Creates an output that represents "no commit performed".
    ///
    /// This is used when a tick is intentionally suppressed, for example when
    /// the scope is [`CommitTickScope::CatchUpOnly`] and policy is not in catch-up mode.
    fn default() -> Self {
        Self {
            cells: Vec::new(),
            has_controller: false,
            all_idle: true,
        }
    }
}

/// Runs one commit tick against the provided stream controllers.
///
/// This function collects a [`QueueSnapshot`], asks [`AdaptiveChunkingPolicy`] for a
/// [`ChunkingDecision`], and then applies the resulting [`DrainPlan`] to both controllers.
/// If callers pass stale controller references (for example, references not tied to the
/// current turn), queue age can be misread and the policy may stay in catch-up longer
/// than expected.
pub(crate) fn run_commit_tick(
    policy: &mut AdaptiveChunkingPolicy,
    stream_controller: Option<&mut StreamController>,
    plan_stream_controller: Option<&mut PlanStreamController>,
    scope: CommitTickScope,
    now: Instant,
) -> CommitTickOutput {
    let snapshot = stream_queue_snapshot(
        stream_controller.as_deref(),
        plan_stream_controller.as_deref(),
        now,
    );
    let decision = resolve_chunking_plan(policy, snapshot, now);
    if scope == CommitTickScope::CatchUpOnly && decision.mode != ChunkingMode::CatchUp {
        return CommitTickOutput::default();
    }

    let output = apply_commit_tick_plan(
        decision.drain_plan,
        stream_controller,
        plan_stream_controller,
    );
    tracing::trace!(
        mode = ?decision.mode,
        queued_lines = snapshot.queued_lines,
        oldest_queued_age_ms = snapshot.oldest_age.map(|age| age.as_millis() as u64),
        drain_plan = ?decision.drain_plan,
        has_controller = output.has_controller,
        all_idle = output.all_idle,
        "stream chunking commit tick"
    );
    output
}

/// Builds the combined queue-pressure snapshot consumed by chunking policy.
///
/// The snapshot sums queue depth across controllers and keeps the maximum oldest age
/// so policy decisions reflect the most delayed queued line currently visible.
fn stream_queue_snapshot(
    stream_controller: Option<&StreamController>,
    plan_stream_controller: Option<&PlanStreamController>,
    now: Instant,
) -> QueueSnapshot {
    let mut queued_lines = 0usize;
    let mut oldest_age: Option<Duration> = None;

    if let Some(controller) = stream_controller {
        queued_lines += controller.queued_lines();
        oldest_age = max_duration(oldest_age, controller.oldest_queued_age(now));
    }
    if let Some(controller) = plan_stream_controller {
        queued_lines += controller.queued_lines();
        oldest_age = max_duration(oldest_age, controller.oldest_queued_age(now));
    }

    QueueSnapshot {
        queued_lines,
        oldest_age,
    }
}

/// Computes one policy decision and emits a trace log on mode transitions.
///
/// This keeps policy transition logging in one place so callers can rely on
/// [`run_commit_tick`] to provide consistent observability.
fn resolve_chunking_plan(
    policy: &mut AdaptiveChunkingPolicy,
    snapshot: QueueSnapshot,
    now: Instant,
) -> ChunkingDecision {
    let prior_mode = policy.mode();
    let decision = policy.decide(snapshot, now);
    if decision.mode != prior_mode {
        tracing::trace!(
            prior_mode = ?prior_mode,
            new_mode = ?decision.mode,
            queued_lines = snapshot.queued_lines,
            oldest_queued_age_ms = snapshot.oldest_age.map(|age| age.as_millis() as u64),
            entered_catch_up = decision.entered_catch_up,
            "stream chunking mode transition"
        );
    }
    decision
}

/// Applies a [`DrainPlan`] to all available stream controllers.
///
/// The returned [`CommitTickOutput`] reports emitted cells and whether all
/// present controllers are idle after draining.
fn apply_commit_tick_plan(
    drain_plan: DrainPlan,
    stream_controller: Option<&mut StreamController>,
    plan_stream_controller: Option<&mut PlanStreamController>,
) -> CommitTickOutput {
    let mut output = CommitTickOutput::default();

    if let Some(controller) = stream_controller {
        output.has_controller = true;
        let (cell, is_idle) = drain_stream_controller(controller, drain_plan);
        if let Some(cell) = cell {
            output.cells.push(cell);
        }
        output.all_idle &= is_idle;
    }
    if let Some(controller) = plan_stream_controller {
        output.has_controller = true;
        let (cell, is_idle) = drain_plan_stream_controller(controller, drain_plan);
        if let Some(cell) = cell {
            output.cells.push(cell);
        }
        output.all_idle &= is_idle;
    }

    output
}

/// Applies one drain step to the main stream controller.
///
/// [`DrainPlan::Single`] maps to one-line drain; [`DrainPlan::Batch`] maps to
/// multi-line drain (including instant catch-up when policy requests the full
/// queued backlog).
fn drain_stream_controller(
    controller: &mut StreamController,
    drain_plan: DrainPlan,
) -> (Option<Box<dyn HistoryCell>>, bool) {
    match drain_plan {
        DrainPlan::Single => controller.on_commit_tick(),
        DrainPlan::Batch(max_lines) => controller.on_commit_tick_batch(max_lines),
    }
}

/// Applies one drain step to the plan stream controller.
///
/// This mirrors [`drain_stream_controller`] so both controller types follow the
/// same chunking policy decisions.
fn drain_plan_stream_controller(
    controller: &mut PlanStreamController,
    drain_plan: DrainPlan,
) -> (Option<Box<dyn HistoryCell>>, bool) {
    match drain_plan {
        DrainPlan::Single => controller.on_commit_tick(),
        DrainPlan::Batch(max_lines) => controller.on_commit_tick_batch(max_lines),
    }
}

/// Returns the greater of two optional durations.
///
/// This helper preserves whichever side is present when only one duration exists.
fn max_duration(lhs: Option<Duration>, rhs: Option<Duration>) -> Option<Duration> {
    match (lhs, rhs) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}
