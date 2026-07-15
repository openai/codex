use super::conversation_function_call_output_message;
use super::conversation_handoff_append_message;
use super::standalone_handoff_message;
use crate::endpoint::realtime_websocket::protocol::RealtimeWireAdapter;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use serde_json::to_value;

#[test]
fn identical_handoff_output_encodes_for_each_bidi_wire_protocol() {
    let legacy = conversation_handoff_append_message(
        RealtimeWireAdapter::V1,
        "handoff-123".to_string(),
        "The result".to_string(),
    );
    let frameless = conversation_handoff_append_message(
        RealtimeWireAdapter::FramelessBidi,
        "handoff-123".to_string(),
        "The result".to_string(),
    );

    assert_eq!(
        to_value(legacy).expect("legacy handoff should serialize"),
        json!({
            "type": "conversation.handoff.append",
            "handoff_id": "handoff-123",
            "output_text": "The result",
        })
    );
    assert_eq!(
        to_value(frameless).expect("frameless handoff should serialize"),
        json!({
            "type": "delegation.context.append",
            "delegation_item_id": "handoff-123",
            "content": [{"type": "input_text", "text": "The result"}],
        })
    );
}

#[test]
fn standalone_handoff_uses_session_context_for_frameless() {
    let legacy = standalone_handoff_message(
        RealtimeWireAdapter::V1,
        "codex".to_string(),
        "Speak this".to_string(),
    );
    let frameless = standalone_handoff_message(
        RealtimeWireAdapter::FramelessBidi,
        "codex".to_string(),
        "Speak this".to_string(),
    );

    assert_eq!(
        to_value(legacy).expect("legacy standalone handoff should serialize"),
        json!({
            "type": "conversation.handoff.append",
            "handoff_id": "codex",
            "output_text": "Speak this",
        })
    );
    assert_eq!(
        to_value(frameless).expect("frameless standalone handoff should serialize"),
        json!({
            "type": "session.context.append",
            "content": [{"type": "input_text", "text": "Speak this"}],
        })
    );
}

#[test]
fn completed_handoff_preserves_legacy_payload_text_in_frameless() {
    let expected_text = Value::String("\"Agent Final Message\":\n\nDone".to_string());
    for wire_adapter in [RealtimeWireAdapter::V1, RealtimeWireAdapter::FramelessBidi] {
        let encoded = to_value(conversation_function_call_output_message(
            wire_adapter,
            "handoff-123".to_string(),
            "Done".to_string(),
        ))
        .expect("handoff output should serialize");
        let text = match wire_adapter {
            RealtimeWireAdapter::V1 => &encoded["output_text"],
            RealtimeWireAdapter::FramelessBidi => &encoded["content"][0]["text"],
            RealtimeWireAdapter::RealtimeV2 => unreachable!(),
        };
        assert_eq!(text, &expected_text);
    }
}
