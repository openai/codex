//! Timing and easing for smooth rotation and fades to a static mark or empty space.

use std::time::Duration;

pub(super) const STATIC_FADE: Duration = Duration::from_millis(/*millis*/ 400);
pub(super) const STATIC_OPACITY: f32 = 0.18;
// The renderer performs two complete rotations per loop.
pub(super) const LOOP_SECONDS: f64 = 7.2;
pub(super) const SPIN_DURATION: Duration = Duration::from_millis(/*millis*/ 10_800);
pub(super) const COMPLETION_FADE: Duration = Duration::from_millis(/*millis*/ 600);

pub(super) fn static_opacity(elapsed: Duration, from: f32) -> f32 {
    STATIC_OPACITY + (from - STATIC_OPACITY) * (1.0 - progress(elapsed, STATIC_FADE)) as f32
}

pub(super) fn completion_opacity(elapsed: Duration) -> f32 {
    (1.0 - progress(elapsed, COMPLETION_FADE)) as f32
}

pub(super) fn progress(elapsed: Duration, duration: Duration) -> f64 {
    let t = (elapsed.as_secs_f64() / duration.as_secs_f64()).min(/*other*/ 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
