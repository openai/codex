//! Report the frozen critical path over the same interval as the turn profile.

use super::TurnProfileState;
use super::duration_to_u64_ms;
use codex_analytics::TurnCriticalPath;
use codex_analytics::TurnCriticalPathDurations;
use codex_otel::TURN_CRITICAL_PATH_CAPTURE_METRIC;
use codex_otel::TURN_CRITICAL_PATH_COMPONENT_METRIC;
use codex_otel::TURN_CRITICAL_PATH_DURATION_METRIC;
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
        inference_ms: duration_to_u64_ms(totals.sampling.saturating_add(totals.compaction)),
        sampling_ms: duration_to_u64_ms(totals.sampling),
        compaction_ms: duration_to_u64_ms(totals.compaction),
        auto_review_ms: duration_to_u64_ms(totals.auto_review),
        auto_review_overhead_ms: totals.auto_review_overhead.map(duration_to_u64_ms),
        user_wait_ms: duration_to_u64_ms(totals.user_wait),
        tools_ms: duration_to_u64_ms(totals.tools),
        overhead_ms: duration_to_u64_ms(totals.overhead),
        unknown_ms: duration_to_u64_ms(totals.unknown),
    })
}

pub(super) fn record_turn_profile_metrics(
    turn: &super::TurnContext,
    profile: &codex_analytics::TurnProfile,
) {
    let telemetry = &turn.session_telemetry;
    let sku = codex_otel::bounded_product_sku(turn.config.apps_mcp_product_sku.as_deref())
        .unwrap_or("unknown");
    let effort = turn.effective_reasoning_effort_for_tracing();
    telemetry.counter(
        TURN_CRITICAL_PATH_CAPTURE_METRIC,
        /*inc*/ 1,
        &[
            ("product_sku", sku),
            ("reasoning_effort", effort.as_str()),
            ("capture_status", profile.critical_path.status()),
        ],
    );
    let Some(times) = profile.critical_path.durations() else {
        return;
    };
    telemetry.record_duration(
        TURN_CRITICAL_PATH_DURATION_METRIC,
        std::time::Duration::from_millis(times.duration_ms),
        &[("product_sku", sku), ("reasoning_effort", effort.as_str())],
    );
    for (component, ms) in [
        ("inference", times.inference_ms),
        ("auto_review", times.auto_review_ms),
        ("tools", times.tools_ms),
        ("user_wait", times.user_wait_ms),
        ("overhead", times.overhead_ms),
        ("unknown", times.unknown_ms),
    ] {
        telemetry.record_duration(
            TURN_CRITICAL_PATH_COMPONENT_METRIC,
            std::time::Duration::from_millis(ms),
            &[
                ("product_sku", sku),
                ("reasoning_effort", effort.as_str()),
                ("component", component),
            ],
        );
    }
}
