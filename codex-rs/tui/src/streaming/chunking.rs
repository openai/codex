//! Adaptive stream chunking policy for commit animation ticks.
//!
//! This policy preserves the baseline user experience while adapting to bursty
//! stream input. In [`ChunkingMode::Smooth`], one queued line is drained per
//! baseline commit tick. When queue pressure rises, it switches to
//! [`ChunkingMode::CatchUp`] and drains queued backlog immediately so display
//! lag converges as quickly as possible.
//!
//! The policy is source-agnostic: it depends only on queue depth and queue
//! age from [`QueueSnapshot`]. It does not branch on source identity or explicit
//! throughput targets.
//!
//! # Mental model
//!
//! Think of this as a two-gear system:
//!
//! - [`ChunkingMode::Smooth`]: steady baseline display pacing.
//! - [`ChunkingMode::CatchUp`]: full queue draining while backlog exists.
//!
//! The transition logic intentionally uses hysteresis:
//!
//! - enter catch-up on higher-pressure thresholds
//! - exit catch-up on lower-pressure thresholds, held for [`EXIT_HOLD`]
//! - after exit, suppress immediate re-entry for [`REENTER_CATCH_UP_HOLD`]
//!   unless backlog is severe
//!
//! This avoids rapid gear-flapping near threshold boundaries.
//!
//! # Policy flow
//!
//! On each decision tick, [`AdaptiveChunkingPolicy::decide`] does:
//!
//! 1. If queue is empty, reset to [`ChunkingMode::Smooth`].
//! 2. If currently smooth, call [`AdaptiveChunkingPolicy::maybe_enter_catch_up`].
//! 3. If currently catch-up, call [`AdaptiveChunkingPolicy::maybe_exit_catch_up`].
//! 4. Build [`DrainPlan`] (`Single` for smooth, `Batch(queued_lines)` for catch-up).
//!
//! # Concrete examples
//!
//! With current defaults:
//!
//! - `Smooth` drains one line per commit tick.
//! - `CatchUp` drains all currently queued lines in a tick.
//!
//! # Tuning guide (in code terms)
//!
//! Prefer tuning in this order so causes remain clear:
//!
//! 1. enter/exit thresholds: [`ENTER_QUEUE_DEPTH_LINES`], [`ENTER_OLDEST_AGE`],
//!    [`EXIT_QUEUE_DEPTH_LINES`], [`EXIT_OLDEST_AGE`]
//! 2. hysteresis windows: [`EXIT_HOLD`], [`REENTER_CATCH_UP_HOLD`]
//! 3. severe gates: [`SEVERE_QUEUE_DEPTH_LINES`], [`SEVERE_OLDEST_AGE`]
//!
//! Symptom-oriented adjustments:
//!
//! - lag starts too late: lower enter thresholds
//! - frequent smooth/catch-up chatter: increase hold windows, or tighten exit
//!   thresholds
//! - catch-up re-enters too eagerly after exit: increase re-entry hold
//!
//! # Responsibilities
//!
//! - track mode and hysteresis state
//! - produce deterministic [`ChunkingDecision`] values from queue snapshots
//! - preserve queue order by draining from queue head only
//!
//! # Non-responsibilities
//!
//! - scheduling commit ticks
//! - reordering stream lines
//! - transport/source-specific semantics
//!
//! Markdown docs remain supplemental:
//!
//! - `docs/tui-stream-chunking-review.md`
//! - `docs/tui-stream-chunking-tuning.md`
//! - `docs/tui-stream-chunking-validation.md`

use std::time::Duration;
use std::time::Instant;

/// Queue-depth threshold that allows entering catch-up mode.
///
/// Crossing this threshold alone is sufficient to leave smooth mode.
const ENTER_QUEUE_DEPTH_LINES: usize = 8;

/// Oldest-line age threshold that allows entering catch-up mode.
///
/// Crossing this threshold alone is sufficient to leave smooth mode.
const ENTER_OLDEST_AGE: Duration = Duration::from_millis(120);

/// Queue-depth threshold used when evaluating catch-up exit hysteresis.
///
/// Depth must be at or below this value before exit hold timing can begin.
const EXIT_QUEUE_DEPTH_LINES: usize = 2;

/// Oldest-line age threshold used when evaluating catch-up exit hysteresis.
///
/// Age must be at or below this value before exit hold timing can begin.
const EXIT_OLDEST_AGE: Duration = Duration::from_millis(40);

/// Minimum duration queue pressure must stay below exit thresholds to leave catch-up mode.
const EXIT_HOLD: Duration = Duration::from_millis(250);

/// Cooldown window after a catch-up exit that suppresses immediate re-entry.
///
/// Severe backlog still bypasses this hold to avoid unbounded queue-age growth.
const REENTER_CATCH_UP_HOLD: Duration = Duration::from_millis(250);

/// Queue-depth cutoff that marks backlog as severe for faster convergence.
///
/// This threshold is used to bypass re-entry hold after a recent catch-up exit.
const SEVERE_QUEUE_DEPTH_LINES: usize = 64;

/// Oldest-line age cutoff that marks backlog as severe for faster convergence.
const SEVERE_OLDEST_AGE: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ChunkingMode {
    /// Drain one line per baseline commit tick.
    #[default]
    Smooth,
    /// Drain multiple lines per tick according to queue pressure.
    CatchUp,
}

/// Captures queue pressure inputs used by adaptive chunking decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct QueueSnapshot {
    /// Number of queued stream lines waiting to be displayed.
    pub(crate) queued_lines: usize,
    /// Age of the oldest queued line at decision time.
    pub(crate) oldest_age: Option<Duration>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DrainPlan {
    /// Emit exactly one queued line.
    Single,
    /// Emit up to `usize` queued lines.
    Batch(usize),
}

/// Represents one policy decision for a specific queue snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChunkingDecision {
    /// Mode after applying hysteresis transitions for this decision.
    pub(crate) mode: ChunkingMode,
    /// Whether this decision transitioned from `Smooth` into `CatchUp`.
    pub(crate) entered_catch_up: bool,
    /// Drain plan to execute for the current commit tick.
    pub(crate) drain_plan: DrainPlan,
}

/// Maintains adaptive chunking mode and hysteresis state across ticks.
#[derive(Debug, Default)]
pub(crate) struct AdaptiveChunkingPolicy {
    mode: ChunkingMode,
    below_exit_threshold_since: Option<Instant>,
    last_catch_up_exit_at: Option<Instant>,
}

impl AdaptiveChunkingPolicy {
    /// Returns the policy mode used by the most recent decision.
    pub(crate) fn mode(&self) -> ChunkingMode {
        self.mode
    }

    /// Resets state to baseline smooth mode.
    pub(crate) fn reset(&mut self) {
        self.mode = ChunkingMode::Smooth;
        self.below_exit_threshold_since = None;
        self.last_catch_up_exit_at = None;
    }

    /// Computes a drain decision from the current queue snapshot.
    ///
    /// The decision is deterministic for a given `(mode, snapshot, now)` triple. Callers should
    /// avoid inventing synthetic snapshots; stale queue age data can cause premature catch-up exits.
    pub(crate) fn decide(&mut self, snapshot: QueueSnapshot, now: Instant) -> ChunkingDecision {
        if snapshot.queued_lines == 0 {
            self.note_catch_up_exit(now);
            self.mode = ChunkingMode::Smooth;
            self.below_exit_threshold_since = None;
            return ChunkingDecision {
                mode: self.mode,
                entered_catch_up: false,
                drain_plan: DrainPlan::Single,
            };
        }

        let entered_catch_up = match self.mode {
            ChunkingMode::Smooth => self.maybe_enter_catch_up(snapshot, now),
            ChunkingMode::CatchUp => {
                self.maybe_exit_catch_up(snapshot, now);
                false
            }
        };

        let drain_plan = match self.mode {
            ChunkingMode::Smooth => DrainPlan::Single,
            ChunkingMode::CatchUp => DrainPlan::Batch(snapshot.queued_lines.max(1)),
        };

        ChunkingDecision {
            mode: self.mode,
            entered_catch_up,
            drain_plan,
        }
    }

    /// Switches from `Smooth` to `CatchUp` when enter thresholds are crossed.
    ///
    /// Returns `true` only on the transition tick so callers can emit one-shot
    /// transition observability.
    fn maybe_enter_catch_up(&mut self, snapshot: QueueSnapshot, now: Instant) -> bool {
        if !should_enter_catch_up(snapshot) {
            return false;
        }
        if self.reentry_hold_active(now) && !is_severe_backlog(snapshot) {
            return false;
        }
        self.mode = ChunkingMode::CatchUp;
        self.below_exit_threshold_since = None;
        self.last_catch_up_exit_at = None;
        true
    }

    /// Applies exit hysteresis while in `CatchUp` mode.
    ///
    /// The policy requires queue pressure to stay below exit thresholds for the
    /// full `EXIT_HOLD` window before returning to `Smooth`.
    fn maybe_exit_catch_up(&mut self, snapshot: QueueSnapshot, now: Instant) {
        if !should_exit_catch_up(snapshot) {
            self.below_exit_threshold_since = None;
            return;
        }

        match self.below_exit_threshold_since {
            Some(since) if now.saturating_duration_since(since) >= EXIT_HOLD => {
                self.mode = ChunkingMode::Smooth;
                self.below_exit_threshold_since = None;
                self.last_catch_up_exit_at = Some(now);
            }
            Some(_) => {}
            None => {
                self.below_exit_threshold_since = Some(now);
            }
        }
    }

    fn note_catch_up_exit(&mut self, now: Instant) {
        if self.mode == ChunkingMode::CatchUp {
            self.last_catch_up_exit_at = Some(now);
        }
    }

    fn reentry_hold_active(&self, now: Instant) -> bool {
        self.last_catch_up_exit_at
            .is_some_and(|exit| now.saturating_duration_since(exit) < REENTER_CATCH_UP_HOLD)
    }
}

/// Returns whether current queue pressure warrants entering catch-up mode.
///
/// Either depth or age pressure is sufficient to trigger catch-up.
fn should_enter_catch_up(snapshot: QueueSnapshot) -> bool {
    snapshot.queued_lines >= ENTER_QUEUE_DEPTH_LINES
        || snapshot
            .oldest_age
            .is_some_and(|oldest| oldest >= ENTER_OLDEST_AGE)
}

/// Returns whether queue pressure is low enough to begin exit hysteresis.
///
/// Both depth and age must be below thresholds; this prevents oscillation when
/// one signal is still under load.
fn should_exit_catch_up(snapshot: QueueSnapshot) -> bool {
    snapshot.queued_lines <= EXIT_QUEUE_DEPTH_LINES
        && snapshot
            .oldest_age
            .is_some_and(|oldest| oldest <= EXIT_OLDEST_AGE)
}

/// Returns whether backlog is severe enough to use a faster catch-up target.
///
/// Severe pressure bypasses re-entry hold to avoid queue-age growth after a
/// recent catch-up exit.
fn is_severe_backlog(snapshot: QueueSnapshot) -> bool {
    snapshot.queued_lines >= SEVERE_QUEUE_DEPTH_LINES
        || snapshot
            .oldest_age
            .is_some_and(|oldest| oldest >= SEVERE_OLDEST_AGE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn snapshot(queued_lines: usize, oldest_age_ms: u64) -> QueueSnapshot {
        QueueSnapshot {
            queued_lines,
            oldest_age: Some(Duration::from_millis(oldest_age_ms)),
        }
    }

    #[test]
    fn smooth_mode_is_default() {
        let mut policy = AdaptiveChunkingPolicy::default();
        let now = Instant::now();

        let decision = policy.decide(snapshot(1, 10), now);
        assert_eq!(decision.mode, ChunkingMode::Smooth);
        assert_eq!(decision.entered_catch_up, false);
        assert_eq!(decision.drain_plan, DrainPlan::Single);
    }

    #[test]
    fn enters_catch_up_on_depth_threshold() {
        let mut policy = AdaptiveChunkingPolicy::default();
        let now = Instant::now();

        let decision = policy.decide(snapshot(8, 10), now);
        assert_eq!(decision.mode, ChunkingMode::CatchUp);
        assert_eq!(decision.entered_catch_up, true);
        assert_eq!(decision.drain_plan, DrainPlan::Batch(8));
    }

    #[test]
    fn enters_catch_up_on_age_threshold() {
        let mut policy = AdaptiveChunkingPolicy::default();
        let now = Instant::now();

        let decision = policy.decide(snapshot(2, 120), now);
        assert_eq!(decision.mode, ChunkingMode::CatchUp);
        assert_eq!(decision.entered_catch_up, true);
        assert_eq!(decision.drain_plan, DrainPlan::Batch(2));
    }

    #[test]
    fn severe_backlog_uses_faster_paced_batches() {
        let mut policy = AdaptiveChunkingPolicy::default();
        let now = Instant::now();
        let _ = policy.decide(snapshot(9, 10), now);

        let decision = policy.decide(snapshot(64, 10), now + Duration::from_millis(5));
        assert_eq!(decision.mode, ChunkingMode::CatchUp);
        assert_eq!(decision.drain_plan, DrainPlan::Batch(64));
    }

    #[test]
    fn catch_up_batch_drains_current_backlog() {
        let mut policy = AdaptiveChunkingPolicy::default();
        let now = Instant::now();
        let decision = policy.decide(snapshot(512, 400), now);
        assert_eq!(decision.mode, ChunkingMode::CatchUp);
        assert_eq!(decision.drain_plan, DrainPlan::Batch(512));
    }

    #[test]
    fn exits_catch_up_after_hysteresis_hold() {
        let mut policy = AdaptiveChunkingPolicy::default();
        let t0 = Instant::now();

        let _ = policy.decide(snapshot(9, 10), t0);
        assert_eq!(policy.mode(), ChunkingMode::CatchUp);

        let pre_hold = policy.decide(snapshot(2, 40), t0 + Duration::from_millis(200));
        assert_eq!(pre_hold.mode, ChunkingMode::CatchUp);

        let post_hold = policy.decide(snapshot(2, 40), t0 + Duration::from_millis(460));
        assert_eq!(post_hold.mode, ChunkingMode::Smooth);
        assert_eq!(post_hold.drain_plan, DrainPlan::Single);
    }

    #[test]
    fn drops_back_to_smooth_when_idle() {
        let mut policy = AdaptiveChunkingPolicy::default();
        let now = Instant::now();
        let _ = policy.decide(snapshot(9, 10), now);
        assert_eq!(policy.mode(), ChunkingMode::CatchUp);

        let decision = policy.decide(
            QueueSnapshot {
                queued_lines: 0,
                oldest_age: None,
            },
            now + Duration::from_millis(20),
        );
        assert_eq!(decision.mode, ChunkingMode::Smooth);
        assert_eq!(decision.drain_plan, DrainPlan::Single);
    }

    #[test]
    fn holds_reentry_after_catch_up_exit() {
        let mut policy = AdaptiveChunkingPolicy::default();
        let t0 = Instant::now();

        let entered = policy.decide(snapshot(8, 20), t0);
        assert_eq!(entered.mode, ChunkingMode::CatchUp);

        let drained = policy.decide(
            QueueSnapshot {
                queued_lines: 0,
                oldest_age: None,
            },
            t0 + Duration::from_millis(20),
        );
        assert_eq!(drained.mode, ChunkingMode::Smooth);

        let held = policy.decide(snapshot(8, 20), t0 + Duration::from_millis(120));
        assert_eq!(held.mode, ChunkingMode::Smooth);
        assert_eq!(held.drain_plan, DrainPlan::Single);

        let reentered = policy.decide(snapshot(8, 20), t0 + Duration::from_millis(320));
        assert_eq!(reentered.mode, ChunkingMode::CatchUp);
        assert_eq!(reentered.drain_plan, DrainPlan::Batch(8));
    }

    #[test]
    fn severe_backlog_can_reenter_during_hold() {
        let mut policy = AdaptiveChunkingPolicy::default();
        let t0 = Instant::now();

        let _ = policy.decide(snapshot(8, 20), t0);
        let _ = policy.decide(
            QueueSnapshot {
                queued_lines: 0,
                oldest_age: None,
            },
            t0 + Duration::from_millis(20),
        );

        let severe = policy.decide(snapshot(64, 20), t0 + Duration::from_millis(120));
        assert_eq!(severe.mode, ChunkingMode::CatchUp);
        assert_eq!(severe.drain_plan, DrainPlan::Batch(64));
    }
}
