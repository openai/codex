use codex_protocol::protocol::RealtimeAudioFrame;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeHandoffRequested;
use codex_protocol::protocol::RealtimeTranscriptDelta;
use serde_json::Map as JsonMap;
use serde_json::Value;
use tracing::debug;

const CODEX_TOOL_NAME: &str = "codex";
const DEFAULT_AUDIO_SAMPLE_RATE: u32 = 24_000;
const DEFAULT_AUDIO_CHANNELS: u16 = 1;
const TOOL_ARGUMENT_KEYS: [&str; 5] = ["input_transcript", "input", "text", "prompt", "query"];

pub(super) fn parse_realtime_event_v2(payload: &str) -> Option<RealtimeEvent> {
    let parsed: Value = match serde_json::from_str(payload) {
        Ok(msg) => msg,
        Err(err) => {
            debug!("failed to parse realtime v2 event: {err}, data: {payload}");
            return None;
        }
    };

    let message_type = match parsed.get("type").and_then(Value::as_str) {
        Some(message_type) => message_type,
        None => {
            debug!("received realtime v2 event without type field: {payload}");
            return None;
        }
    };

    match message_type {
        "session.updated" => parse_session_updated_event(&parsed),
        "response.output_audio.delta" => parse_output_audio_delta_event(&parsed),
        "conversation.item.input_audio_transcription.delta" => {
            parse_transcript_delta_event(&parsed).map(RealtimeEvent::InputTranscriptDelta)
        }
        "conversation.item.input_audio_transcription.completed" => {
            parse_transcript_completed_event(&parsed).map(RealtimeEvent::InputTranscriptDelta)
        }
        "response.output_text.delta" | "response.output_audio_transcript.delta" => {
            parse_transcript_delta_event(&parsed).map(RealtimeEvent::OutputTranscriptDelta)
        }
        "conversation.item.added" => parsed
            .get("item")
            .cloned()
            .map(RealtimeEvent::ConversationItemAdded),
        "conversation.item.done" => parse_conversation_item_done_event(&parsed),
        "error" => parse_error_event(&parsed),
        _ => {
            debug!("received unsupported realtime v2 event type: {message_type}, data: {payload}");
            None
        }
    }
}

fn parse_session_updated_event(parsed: &Value) -> Option<RealtimeEvent> {
    let session_id = parsed
        .get("session")
        .and_then(Value::as_object)
        .and_then(|session| session.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)?;
    let instructions = parsed
        .get("session")
        .and_then(Value::as_object)
        .and_then(|session| session.get("instructions"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(RealtimeEvent::SessionUpdated {
        session_id,
        instructions,
    })
}

fn parse_output_audio_delta_event(parsed: &Value) -> Option<RealtimeEvent> {
    let data = parsed
        .get("delta")
        .and_then(Value::as_str)
        .map(str::to_string)?;
    let sample_rate = parsed
        .get("sample_rate")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(DEFAULT_AUDIO_SAMPLE_RATE);
    let num_channels = parsed
        .get("channels")
        .or_else(|| parsed.get("num_channels"))
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .unwrap_or(DEFAULT_AUDIO_CHANNELS);
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

fn parse_transcript_delta_event(parsed: &Value) -> Option<RealtimeTranscriptDelta> {
    parsed
        .get("delta")
        .and_then(Value::as_str)
        .map(str::to_string)
        .map(|delta| RealtimeTranscriptDelta { delta })
}

fn parse_transcript_completed_event(parsed: &Value) -> Option<RealtimeTranscriptDelta> {
    parsed
        .get("transcript")
        .and_then(Value::as_str)
        .map(str::to_string)
        .map(|delta| RealtimeTranscriptDelta { delta })
}

fn parse_conversation_item_done_event(parsed: &Value) -> Option<RealtimeEvent> {
    let item = parsed.get("item")?.as_object()?;
    if let Some(handoff) = parse_handoff_requested_event(item) {
        return Some(handoff);
    }

    item.get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .map(|item_id| RealtimeEvent::ConversationItemDone { item_id })
}

fn parse_handoff_requested_event(item: &JsonMap<String, Value>) -> Option<RealtimeEvent> {
    let item_type = item.get("type").and_then(Value::as_str);
    let item_name = item.get("name").and_then(Value::as_str);
    if item_type != Some("function_call") || item_name != Some(CODEX_TOOL_NAME) {
        return None;
    }

    let call_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))?;
    let item_id = item
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(call_id)
        .to_string();
    let arguments = item.get("arguments").and_then(Value::as_str).unwrap_or("");

    Some(RealtimeEvent::HandoffRequested(RealtimeHandoffRequested {
        handoff_id: call_id.to_string(),
        item_id,
        input_transcript: extract_input_transcript(arguments),
        active_transcript: Vec::new(),
    }))
}

fn extract_input_transcript(arguments: &str) -> String {
    if arguments.is_empty() {
        return String::new();
    }

    if let Ok(arguments_json) = serde_json::from_str::<Value>(arguments)
        && let Some(arguments_object) = arguments_json.as_object()
    {
        for key in TOOL_ARGUMENT_KEYS {
            if let Some(value) = arguments_object.get(key).and_then(Value::as_str) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }

    arguments.to_string()
}

fn parse_error_event(parsed: &Value) -> Option<RealtimeEvent> {
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
