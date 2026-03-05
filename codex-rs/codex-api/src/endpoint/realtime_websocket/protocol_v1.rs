use crate::endpoint::realtime_websocket::types::RealtimeAudioFrame;
use crate::endpoint::realtime_websocket::types::RealtimeEvent;
use crate::endpoint::realtime_websocket::types::RealtimeHandoffMessage;
use crate::endpoint::realtime_websocket::types::RealtimeHandoffRequested;
use serde_json::Value;
use std::string::ToString;
use tracing::debug;

pub(super) fn parse_realtime_event(payload: &str) -> Option<RealtimeEvent> {
    let parsed: Value = match serde_json::from_str(payload) {
        Ok(msg) => msg,
        Err(err) => {
            debug!("failed to parse realtime event: {err}, data: {payload}");
            return None;
        }
    };

    let message_type = match parsed.get("type").and_then(Value::as_str) {
        Some(message_type) => message_type,
        None => {
            debug!("received realtime event without type field: {payload}");
            return None;
        }
    };

    match message_type {
        "session.updated" => parse_session_updated(&parsed),
        "conversation.output_audio.delta" => parse_audio_delta(&parsed),
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
        "conversation.handoff.requested" => parse_handoff_requested(&parsed),
        "error" => parse_realtime_error(&parsed),
        _ => {
            debug!("received unsupported realtime event type: {message_type}, data: {payload}");
            None
        }
    }
}

fn parse_session_updated(parsed: &Value) -> Option<RealtimeEvent> {
    let session_id = parsed
        .get("session")
        .and_then(Value::as_object)
        .and_then(|session| session.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let instructions = parsed
        .get("session")
        .and_then(Value::as_object)
        .and_then(|session| session.get("instructions"))
        .and_then(Value::as_str)
        .map(str::to_string);
    session_id.map(|session_id| RealtimeEvent::SessionUpdated {
        session_id,
        instructions,
    })
}

fn parse_audio_delta(parsed: &Value) -> Option<RealtimeEvent> {
    let data = parsed
        .get("delta")
        .and_then(Value::as_str)
        .or_else(|| parsed.get("data").and_then(Value::as_str))
        .map(str::to_string)?;
    let sample_rate = parsed
        .get("sample_rate")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())?;
    let num_channels = parsed
        .get("channels")
        .or_else(|| parsed.get("num_channels"))
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok())?;
    Some(RealtimeEvent::AudioOut(RealtimeAudioFrame {
        data,
        sample_rate,
        num_channels,
        samples_per_channel: parsed
            .get("samples_per_channel")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok()),
    }))
}

fn parse_realtime_error(parsed: &Value) -> Option<RealtimeEvent> {
    parsed
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            parsed
                .get("error")
                .and_then(Value::as_object)
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| parsed.get("error").map(ToString::to_string))
        .map(RealtimeEvent::Error)
}

fn parse_handoff_requested(parsed: &Value) -> Option<RealtimeEvent> {
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
    let messages = parsed
        .get("messages")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|message| {
            let role = message.get("role").and_then(Value::as_str)?.to_string();
            let text = message.get("text").and_then(Value::as_str)?.to_string();
            Some(RealtimeHandoffMessage { role, text })
        })
        .collect();
    Some(RealtimeEvent::HandoffRequested(RealtimeHandoffRequested {
        handoff_id,
        item_id,
        input_transcript,
        messages,
    }))
}
