//! Report the frozen critical path over the same interval as the turn profile.

use super::TurnProfileState;
use super::duration_to_u64_ms;
use codex_analytics::TurnCriticalPath;
use codex_analytics::TurnCriticalPathDurations;
use std::time::Instant;

pub(super) fn completed_critical_path(state: &TurnProfileState, end: Instant) -> TurnCriticalPath {
    if !state.capture_enabled {
        return TurnCriticalPath::Disabled;
    }
    let (Some(path), Some(start)) = (&state.path, state.started_at) else {
        return TurnCriticalPath::Incomplete;
    };
    let totals = path.durations(start, end);
    TurnCriticalPath::Complete(TurnCriticalPathDurations {
        duration_ms: duration_to_u64_ms(end.saturating_duration_since(start)),
        inference_ms: duration_to_u64_ms(totals.inference),
        auto_review_ms: duration_to_u64_ms(totals.auto_review),
        user_wait_ms: duration_to_u64_ms(totals.user_wait),
        tools_ms: duration_to_u64_ms(totals.tools),
        overhead_ms: duration_to_u64_ms(totals.overhead),
        unknown_ms: duration_to_u64_ms(totals.unknown),
    })
}
