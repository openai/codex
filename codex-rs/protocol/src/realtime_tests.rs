#![cfg(test)]

//! Validates the sparse channel envelope written by new voice calls.

use super::RealtimeItem;
use pretty_assertions::assert_eq;

#[test]
fn frame_roundtrips_and_requires_a_voice_payload() {
    let frame = serde_json::json!({
        "id": "speech",
        "realtime_session_id": "voice",
        "observed_at_ms": 1050,
        "type": "frame",
        "channels": { "voice_transcript": { "role": "assistant", "text": "hello" } },
    });
    let item = serde_json::from_value::<RealtimeItem>(frame.clone()).unwrap();
    assert_eq!(serde_json::to_value(item).unwrap(), frame);
    let mut empty = frame;
    empty["channels"] = serde_json::json!({});
    assert!(serde_json::from_value::<RealtimeItem>(empty).is_err());
}
