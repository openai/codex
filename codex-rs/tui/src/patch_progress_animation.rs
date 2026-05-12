//! Animation state for live apply_patch line-count counters.
//!
//! The TUI receives patch previews as complete snapshots of the current file
//! changes, not as line-count deltas. This module converts monotonically
//! increasing added/removed totals into a small presentation state machine:
//! newly observed lines appear as a floating counter, drain into the committed
//! header total, and stop once no floating value remains.
//!
//! The module deliberately does not own patch parsing, file labels, or
//! transcript rendering. Callers compute the current and next totals from the
//! active patch cell, pass those totals to update_counter_anim, and schedule
//! redraws while PatchCounterAnim::is_active remains true.
//!
//! A first snapshot establishes the baseline and is not animated; otherwise a
//! newly-created patch would show the entire file as a fresh delta. Decreases
//! also do not animate, because they usually mean the app-server sent a revised
//! snapshot rather than a user-visible removal of already-rendered progress.

use std::time::Duration;
use std::time::Instant;

use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;

/// Millisecond cadence used while a live patch counter animation is active.
///
/// The same cadence is used both to schedule TUI redraws and to produce the
/// transcript animation tick, so a running counter invalidates the active cell
/// even when no new app-server notification has arrived.
pub(crate) const PATCH_PROGRESS_FRAME_MS: u64 = 50;

const PULSE_IN_MS: u64 = 180;
const STEP_MS: u64 = 60;
const DRAIN_CAP_MS: u64 = 800;

/// Identifies which line-count side a patch progress counter represents.
///
/// The kind owns only display concerns that are shared by committed and
/// floating counters: the sigil and the stable foreground color. It does not
/// encode any parsing behavior, so callers must compute added and removed line
/// totals before constructing animations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PatchCounterKind {
    /// Added-line counter rendered with a green plus sigil.
    Added,
    /// Removed-line counter rendered with a red minus sigil.
    Removed,
}

impl PatchCounterKind {
    /// Returns the sign rendered before the numeric counter value.
    pub(crate) fn sigil(self) -> char {
        match self {
            PatchCounterKind::Added => '+',
            PatchCounterKind::Removed => '-',
        }
    }

    /// Returns the stable style for the committed counter for this side.
    ///
    /// Floating counters may temporarily add pulse styling, but the committed
    /// header value always uses this style so the target column stays visually
    /// anchored.
    pub(crate) fn normal_style(self) -> Style {
        match self {
            PatchCounterKind::Added => Style::default().green(),
            PatchCounterKind::Removed => Style::default().red(),
        }
    }
}

/// Tracks one floating-to-committed patch line-count animation.
///
/// An instance is owned by a single StreamingPatchHistoryCell and is mutated
/// only on the UI thread when newer patch snapshots arrive. base is the
/// committed value at the start of the current drain, and target is the total
/// value that will be committed once the drain finishes. Calling
/// PatchCounterAnim::add_delta while the animation is active preserves the
/// current committed value before extending the target, which avoids visible
/// jumps when app-server notifications arrive close together.
#[derive(Clone, Debug)]
pub(crate) struct PatchCounterAnim {
    kind: PatchCounterKind,
    base: usize,
    target: usize,
    pulse_in_end: Instant,
    drain_start: Instant,
    drain_end: Instant,
}

impl PatchCounterAnim {
    /// Starts a new counter animation from an already-rendered total.
    ///
    /// base must be the committed total currently visible in the header, and
    /// delta must be the newly observed increase for the same counter kind.
    /// Passing the full next total as delta would double-count the baseline
    /// and make the floating counter report too many lines.
    pub(crate) fn start(kind: PatchCounterKind, base: usize, delta: usize, now: Instant) -> Self {
        let pulse_in_end = now + Duration::from_millis(PULSE_IN_MS);
        let drain_start = pulse_in_end;
        let drain_end = drain_start + drain_duration_for(delta);
        Self {
            kind,
            base,
            target: base.saturating_add(delta),
            pulse_in_end,
            drain_start,
            drain_end,
        }
    }

    /// Coalesces an additional increase into a running animation.
    ///
    /// The committed value at now becomes the new base before the target is
    /// extended, so the rendered total remains continuous across snapshots.
    /// Callers should only use this for increases on an active animation; use
    /// update_counter_anim for normal snapshot reconciliation.
    pub(crate) fn add_delta(&mut self, delta: usize, now: Instant) {
        let current = self.committed(now);
        self.base = current;
        self.target = self.target.saturating_add(delta);
        self.drain_start = now.max(self.pulse_in_end);
        self.drain_end = self.drain_start + drain_duration_for(self.target.saturating_sub(current));
    }

    /// Returns the portion of the target value that should render in the header.
    ///
    /// The value is clamped between base and target and is safe to query with
    /// any Instant. Querying before the drain starts intentionally keeps the
    /// committed header unchanged while the floating counter pulses in.
    pub(crate) fn committed(&self, now: Instant) -> usize {
        if now <= self.drain_start || self.target == self.base {
            return self.base;
        }
        if now >= self.drain_end {
            return self.target;
        }

        let total = self
            .drain_end
            .saturating_duration_since(self.drain_start)
            .as_secs_f64()
            .max(f64::EPSILON);
        let elapsed = now
            .saturating_duration_since(self.drain_start)
            .as_secs_f64();
        let span = self.target.saturating_sub(self.base) as f64;
        let value = self.base as f64 + span * (elapsed / total);
        value.round().clamp(self.base as f64, self.target as f64) as usize
    }

    /// Returns the portion of the target value that should render as floating.
    ///
    /// This is always target - committed(now), so callers can render the
    /// floating value above or below the header without separately tracking how
    /// much of the delta has already drained.
    pub(crate) fn floating(&self, now: Instant) -> usize {
        self.target.saturating_sub(self.committed(now))
    }

    #[cfg(test)]
    fn target(&self) -> usize {
        self.target
    }

    /// Returns whether the animation still needs redraws.
    ///
    /// An animation remains active while there is a floating value to render.
    /// Once this returns false the caller may keep the struct around, but
    /// scheduling more animation frames for it would waste redraws with no
    /// visual change.
    pub(crate) fn is_active(&self, now: Instant) -> bool {
        self.floating(now) > 0
    }

    /// Returns the style for the current floating counter phase.
    ///
    /// Pulse-in is bold and steady drain uses the normal counter style. Callers
    /// should not use this for committed header values because doing so would
    /// make the target column itself pulse.
    pub(crate) fn floating_style(&self, now: Instant) -> Style {
        if now < self.pulse_in_end {
            return self.kind.normal_style().add_modifier(Modifier::BOLD);
        }
        self.kind.normal_style()
    }
}

/// Reconciles a complete patch snapshot into optional counter animation state.
///
/// The first non-empty snapshot establishes the baseline, disabled animations
/// clear any existing state, and decreases clear the animation because they
/// represent snapshot replacement rather than forward progress. This function
/// is the preferred entry point for callers that receive complete snapshots; a
/// caller that directly starts an animation for the baseline would make the
/// first preview look like newly-arrived progress.
pub(crate) fn update_counter_anim(
    animation: &mut Option<PatchCounterAnim>,
    kind: PatchCounterKind,
    current_value: usize,
    next_value: usize,
    had_existing_changes: bool,
    animations_enabled: bool,
    now: Instant,
) {
    if current_value == next_value {
        return;
    }

    if !animations_enabled || !had_existing_changes || next_value <= current_value {
        *animation = None;
        return;
    }

    let delta = next_value - current_value;
    match animation {
        Some(anim) if anim.is_active(now) => anim.add_delta(delta, now),
        _ => {
            *animation = Some(PatchCounterAnim::start(kind, current_value, delta, now));
        }
    }
}

fn drain_duration_for(delta: usize) -> Duration {
    let ms = STEP_MS
        .saturating_mul(delta as u64)
        .clamp(STEP_MS, DRAIN_CAP_MS);
    Duration::from_millis(ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn committed_starts_at_base() {
        let now = t0();
        let anim = PatchCounterAnim::start(
            PatchCounterKind::Added,
            /*base*/ 20,
            /*delta*/ 5,
            now,
        );

        assert_eq!(anim.committed(now), 20);
        assert_eq!(anim.floating(now), 5);
    }

    #[test]
    fn committed_reaches_target_after_drain() {
        let now = t0();
        let anim = PatchCounterAnim::start(
            PatchCounterKind::Added,
            /*base*/ 20,
            /*delta*/ 5,
            now,
        );
        let after = now + Duration::from_millis(/*millis*/ PULSE_IN_MS + DRAIN_CAP_MS + 10);

        assert_eq!(anim.committed(after), 25);
        assert_eq!(anim.floating(after), 0);
    }

    #[test]
    fn coalesce_keeps_committed_continuous_and_extends_target() {
        let now = t0();
        let mut anim = PatchCounterAnim::start(
            PatchCounterKind::Added,
            /*base*/ 20,
            /*delta*/ 5,
            now,
        );
        let mid = now + Duration::from_millis(/*millis*/ PULSE_IN_MS + 150);
        let before = anim.committed(mid);

        anim.add_delta(/*delta*/ 3, mid);

        assert_eq!(anim.committed(mid), before);
        assert_eq!(anim.target(), 28);
        let later = mid + Duration::from_millis(/*millis*/ DRAIN_CAP_MS + 10);
        assert_eq!(anim.committed(later), 28);
    }

    #[test]
    fn inactive_once_floating_counter_drains() {
        let now = t0();
        let anim = PatchCounterAnim::start(
            PatchCounterKind::Added,
            /*base*/ 0,
            /*delta*/ 2,
            now,
        );
        let before_drain = now + Duration::from_millis(/*millis*/ PULSE_IN_MS + STEP_MS - 10);
        let after_drain =
            now + Duration::from_millis(/*millis*/ PULSE_IN_MS + STEP_MS * 2 + 10);

        assert!(anim.is_active(before_drain));
        assert!(!anim.is_active(after_drain));
    }

    #[test]
    fn update_counter_anim_skips_baseline_and_decreases() {
        let now = t0();
        let mut anim = None;

        update_counter_anim(
            &mut anim,
            PatchCounterKind::Added,
            /*current_value*/ 0,
            /*next_value*/ 5,
            /*had_existing_changes*/ false,
            /*animations_enabled*/ true,
            now,
        );
        assert!(anim.is_none());

        update_counter_anim(
            &mut anim,
            PatchCounterKind::Added,
            /*current_value*/ 5,
            /*next_value*/ 4,
            /*had_existing_changes*/ true,
            /*animations_enabled*/ true,
            now,
        );
        assert!(anim.is_none());
    }

    #[test]
    fn update_counter_anim_coalesces_active_animation() {
        let now = t0();
        let mut anim = None;

        update_counter_anim(
            &mut anim,
            PatchCounterKind::Added,
            /*current_value*/ 5,
            /*next_value*/ 8,
            /*had_existing_changes*/ true,
            /*animations_enabled*/ true,
            now,
        );
        let mid = now + Duration::from_millis(/*millis*/ PULSE_IN_MS + 100);
        update_counter_anim(
            &mut anim,
            PatchCounterKind::Added,
            /*current_value*/ 8,
            /*next_value*/ 10,
            /*had_existing_changes*/ true,
            /*animations_enabled*/ true,
            mid,
        );

        let anim = anim.expect("active animation");
        assert_eq!(anim.target(), 10);
        assert!(anim.committed(mid) >= 5);
    }
}
