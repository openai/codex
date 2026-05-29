use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeTranscriptDelta;
use codex_protocol::protocol::RealtimeTranscriptDone;
use serde_json::Value;
use tracing::debug;

pub(super) fn parse_realtime_payload(payload: &str, parser_name: &str) -> Option<(Value, String)> {
    let parsed: Value = match serde_json::from_str(payload) {
        Ok(message) => message,
        Err(err) => {
            debug!("failed to parse {parser_name} event: {err}, data: {payload}");
            return None;
        }
    };

    let message_type = match parsed.get("type").and_then(Value::as_str) {
        Some(message_type) => message_type.to_string(),
        None => {
            debug!("received {parser_name} event without type field: {payload}");
            return None;
        }
    };

    Some((parsed, message_type))
}

pub(super) fn parse_session_updated_event(parsed: &Value) -> Option<RealtimeEvent> {
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
        realtime_session_id: session_id,
        instructions,
    })
}

pub(super) fn parse_transcript_delta_event(
    parsed: &Value,
    field: &str,
) -> Option<RealtimeTranscriptDelta> {
    parsed
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .map(|delta| RealtimeTranscriptDelta { delta })
}

pub(super) fn parse_transcript_done_event(
    parsed: &Value,
    field: &str,
) -> Option<RealtimeTranscriptDone> {
    parsed
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .map(|text| RealtimeTranscriptDone { text })
}

pub(super) fn parse_error_event(parsed: &Value) -> Option<RealtimeEvent> {
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
