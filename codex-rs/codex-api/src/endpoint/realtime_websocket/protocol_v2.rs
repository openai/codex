use crate::endpoint::realtime_websocket::types::RealtimeAudioFrame;
use crate::endpoint::realtime_websocket::types::RealtimeEvent;
use crate::endpoint::realtime_websocket::types::RealtimeHandoffMessage;
use crate::endpoint::realtime_websocket::types::RealtimeHandoffRequested;
use serde::Deserialize;
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
        "session.created" | "session.updated" => parse_session_updated(&parsed),
        "response.output_audio.delta" => parse_audio_delta(&parsed),
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
        "response.done" => {
            if let Some(handoff) = parse_handoff_requested(&parsed) {
                return Some(RealtimeEvent::HandoffRequested(handoff));
            }
            Some(RealtimeEvent::ConversationItemAdded(parsed))
        }
        "error" => parse_realtime_error(&parsed),
        _ => Some(RealtimeEvent::ConversationItemAdded(parsed)),
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
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(24_000);
    let num_channels = parsed
        .get("channels")
        .or_else(|| parsed.get("num_channels"))
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok())
        .unwrap_or(1);
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

fn parse_handoff_requested(parsed: &Value) -> Option<RealtimeHandoffRequested> {
    let outputs = parsed
        .get("response")
        .and_then(Value::as_object)
        .and_then(|response| response.get("output"))
        .and_then(Value::as_array)?;
    let function_call = outputs.iter().find(|item| {
        item.get("type").and_then(Value::as_str) == Some("function_call")
            && item.get("name").and_then(Value::as_str) == Some("codex")
    })?;
    let handoff_id = function_call
        .get("call_id")
        .and_then(Value::as_str)
        .map(str::to_string)?;
    let item_id = function_call
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| handoff_id.clone());
    let arguments = function_call
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (input_transcript, messages) = parse_handoff_arguments(arguments);
    Some(RealtimeHandoffRequested {
        handoff_id,
        item_id,
        input_transcript,
        messages,
    })
}

fn parse_handoff_arguments(arguments: &str) -> (String, Vec<RealtimeHandoffMessage>) {
    #[derive(Debug, Deserialize)]
    struct HandoffArguments {
        #[serde(default)]
        prompt: Option<String>,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        input: Option<String>,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        input_transcript: Option<String>,
        #[serde(default)]
        messages: Vec<RealtimeHandoffMessage>,
    }

    let Some(parsed) = serde_json::from_str::<HandoffArguments>(arguments).ok() else {
        return (
            arguments.to_string(),
            vec![RealtimeHandoffMessage {
                role: "user".to_string(),
                text: arguments.to_string(),
            }],
        );
    };
    let messages = parsed
        .messages
        .into_iter()
        .filter(|message| !message.text.is_empty())
        .collect::<Vec<_>>();
    for value in [
        parsed.prompt,
        parsed.text,
        parsed.input,
        parsed.message,
        parsed.input_transcript,
    ]
    .into_iter()
    .flatten()
    {
        if !value.is_empty() {
            if messages.is_empty() {
                return (
                    value.clone(),
                    vec![RealtimeHandoffMessage {
                        role: "user".to_string(),
                        text: value,
                    }],
                );
            }
            return (value, messages);
        }
    }
    if let Some(first_message) = messages.first() {
        return (first_message.text.clone(), messages);
    }
    (String::new(), messages)
}
