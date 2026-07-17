use super::conversation_function_call_output_message;
use super::conversation_handoff_append_message;
use super::standalone_handoff_message;
use crate::endpoint::realtime_websocket::protocol::RealtimeContextAppendChannel;
use crate::endpoint::realtime_websocket::protocol::RealtimeWireAdapter;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use serde_json::to_value;

#[test]
fn context_append_channel_only_encodes_for_frameless_handoff_output() {
    let legacy = conversation_handoff_append_message(
        RealtimeWireAdapter::V1,
        "handoff-123".to_string(),
        "The result".to_string(),
        Some(RealtimeContextAppendChannel::Commentary),
    );
    let frameless = conversation_handoff_append_message(
        RealtimeWireAdapter::FramelessBidi,
        "handoff-123".to_string(),
        "The result".to_string(),
        Some(RealtimeContextAppendChannel::Commentary),
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
            "channel": "commentary",
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
        Some(RealtimeContextAppendChannel::Speakable),
    );
    let frameless = standalone_handoff_message(
        RealtimeWireAdapter::FramelessBidi,
        "codex".to_string(),
        "Speak this".to_string(),
        Some(RealtimeContextAppendChannel::Speakable),
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
            "channel": "speakable",
            "content": [{"type": "input_text", "text": "Speak this"}],
        })
    );
}

#[test]
fn completed_handoff_only_prefixes_v1_payload_text() {
    for wire_adapter in [RealtimeWireAdapter::V1, RealtimeWireAdapter::FramelessBidi] {
        let encoded = to_value(conversation_function_call_output_message(
            wire_adapter,
            "handoff-123".to_string(),
            "Done".to_string(),
            Some(RealtimeContextAppendChannel::Speakable),
        ))
        .expect("handoff output should serialize");
        let text = match wire_adapter {
            RealtimeWireAdapter::V1 => &encoded["output_text"],
            RealtimeWireAdapter::FramelessBidi => &encoded["content"][0]["text"],
            RealtimeWireAdapter::RealtimeV2 => unreachable!(),
        };
        assert_eq!(
            text,
            &Value::String(match wire_adapter {
                RealtimeWireAdapter::V1 => "\"Agent Final Message\":\n\nDone".to_string(),
                RealtimeWireAdapter::FramelessBidi => "Done".to_string(),
                RealtimeWireAdapter::RealtimeV2 => unreachable!(),
            })
        );
        assert_eq!(
            encoded.get("channel").cloned(),
            match wire_adapter {
                RealtimeWireAdapter::V1 => None,
                RealtimeWireAdapter::FramelessBidi => {
                    Some(Value::String("speakable".to_string()))
                }
                RealtimeWireAdapter::RealtimeV2 => unreachable!(),
            }
        );
    }
}
