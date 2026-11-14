#![cfg(test)]

//! Checks clock rollback and restoration from durable history checkpoints.

use super::ObservationClock;
use codex_history::RolloutItem;
use codex_protocol::realtime::RealtimeItem;
use codex_protocol::realtime::RealtimeItemContent;
use pretty_assertions::assert_eq;

#[test]
fn observation_time_survives_clock_rollback_and_owner_restart() {
    let mut clock = ObservationClock::default();
    assert_eq!(
        [
            clock.observe(/*wall_clock_ms*/ 100),
            clock.observe(/*wall_clock_ms*/ 90),
            clock.observe(/*wall_clock_ms*/ 110)
        ],
        [100, 100, 110]
    );

    let mut clock = ObservationClock::default();
    clock.restore(&[
        RolloutItem::RealtimeItem(RealtimeItem {
            id: "sealed".into(),
            realtime_session_id: "voice".into(),
            observed_at_ms: Some(110),
            content: RealtimeItemContent::RealtimeSessionStarted,
        }),
        RolloutItem::RealtimeItem(RealtimeItem {
            id: "legacy".into(),
            realtime_session_id: "legacy-voice".into(),
            observed_at_ms: None,
            content: RealtimeItemContent::RealtimeSessionStarted,
        }),
    ]);
    assert_eq!(
        [
            clock.observe(/*wall_clock_ms*/ 80),
            clock.observe(/*wall_clock_ms*/ 120)
        ],
        [110, 120]
    );
}

#[test]
fn compaction_retains_the_floor_when_older_events_are_omitted() {
    let compacted = serde_json::from_value::<RolloutItem>(serde_json::json!({
        "type": "compacted",
        "payload": {
            "message": "checkpoint",
            "replacement_history": [],
            "window_number": 1,
            "resume_metadata": {
                "observation_clock_floor_ms": 150,
                "multi_agent_version": null,
                "last_started_turn_id": null,
                "previous_turn_settings": null
            }
        }
    }))
    .expect("read persisted checkpoint");
    let mut clock = ObservationClock::default();
    clock.restore(&[compacted]);
    assert_eq!(
        [
            clock.observe(/*wall_clock_ms*/ 100),
            clock.observe(/*wall_clock_ms*/ 200)
        ],
        [150, 200]
    );
}
