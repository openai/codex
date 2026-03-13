use crate::endpoint::realtime_websocket::protocol_common::parse_error_event;
use crate::endpoint::realtime_websocket::protocol_common::parse_realtime_payload;
use crate::endpoint::realtime_websocket::protocol_common::parse_session_updated_event;
use crate::endpoint::realtime_websocket::protocol_common::parse_transcript_delta_event;
use codex_protocol::protocol::RealtimeAudioFrame;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeHandoffRequested;
use serde_json::Value;
use tracing::debug;

pub(super) fn parse_realtime_event_v1(payload: &str) -> Option<RealtimeEvent> {
    let (parsed, message_type) = parse_realtime_payload(payload, "realtime v1")?;
    match message_type.as_str() {
        "session.updated" => parse_session_updated_event(&parsed),
        "conversation.output_audio.delta" => {
            let data = parsed
                .get("delta")
                .and_then(Value::as_str)
                .or_else(|| parsed.get("data").and_then(Value::as_str))
                .map(str::to_string)?;
            let sample_rate = parsed
                .get("sample_rate")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())?;
            let num_channels = parsed
                .get("channels")
                .or_else(|| parsed.get("num_channels"))
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())?;
            Some(RealtimeEvent::AudioOut(RealtimeAudioFrame {
                data,
                sample_rate,
                num_channels,
                samples_per_channel: parsed
                    .get("samples_per_channel")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok()),
            }))
        }
        "conversation.input_transcript.delta" => {
            parse_transcript_delta_event(&parsed, "delta").map(RealtimeEvent::InputTranscriptDelta)
        }
        "conversation.output_transcript.delta" => {
            parse_transcript_delta_event(&parsed, "delta").map(RealtimeEvent::OutputTranscriptDelta)
        }
        "conversation.item.added" => parsed
            .get("item")
            .cloned()
            .map(RealtimeEvent::ConversationItemAdded),
        "conversation.item.done" => parsed
            .get("item")
            .and_then(Value::as_object)
            .and_then(|item| item.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .map(|item_id| RealtimeEvent::ConversationItemDone { item_id }),
        "conversation.handoff.requested" => {
            let handoff_id = parsed
                .get("handoff_id")
                .and_then(Value::as_str)
                .map(str::to_string)?;
            let item_id = parsed
                .get("item_id")
                .and_then(Value::as_str)
                .map(str::to_string)?;
            let input_transcript = parsed
                .get("input_transcript")
                .and_then(Value::as_str)
                .map(str::to_string)?;
            Some(RealtimeEvent::HandoffRequested(RealtimeHandoffRequested {
                handoff_id,
                item_id,
                input_transcript,
                active_transcript: Vec::new(),
            }))
        }
        "error" => parse_error_event(&parsed),
        _ => {
            debug!("received unsupported realtime v1 event type: {message_type}, data: {payload}");
            None
        }
    }
}
