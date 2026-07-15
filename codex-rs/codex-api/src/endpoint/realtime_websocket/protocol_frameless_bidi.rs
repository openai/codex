use crate::endpoint::realtime_websocket::protocol_common::parse_error_event;
use crate::endpoint::realtime_websocket::protocol_common::parse_realtime_payload;
use crate::endpoint::realtime_websocket::protocol_common::parse_session_updated_event;
use codex_protocol::protocol::RealtimeAudioFrame;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeHandoffRequested;
use codex_protocol::protocol::RealtimeTranscriptDelta;
use codex_protocol::protocol::RealtimeTranscriptDone;
use serde_json::Value;
use tracing::debug;

const DEFAULT_AUDIO_SAMPLE_RATE: u32 = 24_000;
const DEFAULT_AUDIO_CHANNELS: u16 = 1;

pub(super) fn parse_frameless_bidi_event(payload: &str) -> Option<RealtimeEvent> {
    let (parsed, message_type) = parse_realtime_payload(payload, "frameless bidi")?;
    match message_type.as_str() {
        "session.started" | "session.updated" => parse_session_updated_event(&parsed),
        "output_audio.delta" => parse_output_audio_delta(&parsed),
        "input_transcript.added" => {
            parse_transcript_item(&parsed).map(RealtimeEvent::InputTranscriptDelta)
        }
        "output_transcript.added" => {
            parse_transcript_item(&parsed).map(RealtimeEvent::OutputTranscriptDelta)
        }
        "turn.done" => parse_turn_done(&parsed),
        "delegation.created" => parse_delegation_created(&parsed),
        "error" => parse_error_event(&parsed),
        _ => {
            debug!(
                "received unsupported frameless bidi event type: {message_type}, data: {payload}"
            );
            None
        }
    }
}

fn parse_output_audio_delta(parsed: &Value) -> Option<RealtimeEvent> {
    Some(RealtimeEvent::AudioOut(RealtimeAudioFrame {
        data: parsed.get("audio").and_then(Value::as_str)?.to_string(),
        sample_rate: DEFAULT_AUDIO_SAMPLE_RATE,
        num_channels: DEFAULT_AUDIO_CHANNELS,
        samples_per_channel: None,
        item_id: None,
    }))
}

fn parse_transcript_item(parsed: &Value) -> Option<RealtimeTranscriptDelta> {
    parsed
        .get("item")
        .and_then(Value::as_object)
        .and_then(|item| item.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .map(|delta| RealtimeTranscriptDelta { delta })
}

fn parse_turn_done(parsed: &Value) -> Option<RealtimeEvent> {
    let turn = parsed.get("turn")?.as_object()?;
    let role = turn.get("role").and_then(Value::as_str)?;
    let text = turn
        .get("transcript")
        .and_then(Value::as_str)
        .map(str::to_string)?;
    let done = RealtimeTranscriptDone { text };
    match role {
        "user" => Some(RealtimeEvent::InputTranscriptDone(done)),
        "assistant" => Some(RealtimeEvent::OutputTranscriptDone(done)),
        _ => None,
    }
}

fn parse_delegation_created(parsed: &Value) -> Option<RealtimeEvent> {
    let item = parsed.get("item")?.as_object()?;
    if item.get("type").and_then(Value::as_str) != Some("delegation")
        || item.get("target").and_then(Value::as_str) != Some("client")
    {
        return None;
    }
    let item_id = item.get("id").and_then(Value::as_str)?.to_string();
    let input_transcript = item
        .get("content")
        .and_then(Value::as_array)?
        .iter()
        .filter(|content| content.get("type").and_then(Value::as_str) == Some("input_text"))
        .filter_map(|content| content.get("text").and_then(Value::as_str))
        .collect::<String>();

    Some(RealtimeEvent::HandoffRequested(RealtimeHandoffRequested {
        handoff_id: item_id.clone(),
        item_id,
        input_transcript,
        active_transcript: Vec::new(),
    }))
}

#[cfg(test)]
#[path = "protocol_frameless_bidi_tests.rs"]
mod tests;
