use codex_protocol::protocol::RealtimeAudioFrame;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeHandoffRequested;
use codex_protocol::protocol::RealtimeTranscriptDelta;
use serde_json::Value;
use tracing::debug;

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
        "session.updated" => {
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
        "response.output_audio.delta" => {
            let data = parsed
                .get("delta")
                .and_then(Value::as_str)
                .map(str::to_string)?;
            let sample_rate = parsed
                .get("sample_rate")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(24_000);
            let num_channels = parsed
                .get("channels")
                .or_else(|| parsed.get("num_channels"))
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(1);
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
        "conversation.item.input_audio_transcription.delta" => parsed
            .get("delta")
            .and_then(Value::as_str)
            .map(str::to_string)
            .map(|delta| RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta { delta })),
        "conversation.item.input_audio_transcription.completed" => parsed
            .get("transcript")
            .and_then(Value::as_str)
            .map(str::to_string)
            .map(|delta| RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta { delta })),
        "response.output_text.delta" | "response.output_audio_transcript.delta" => parsed
            .get("delta")
            .and_then(Value::as_str)
            .map(str::to_string)
            .map(|delta| RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta { delta })),
        "conversation.item.added" => parsed
            .get("item")
            .cloned()
            .map(RealtimeEvent::ConversationItemAdded),
        "conversation.item.done" => {
            let item = parsed.get("item")?.as_object()?;
            let item_type = item.get("type").and_then(Value::as_str);
            let item_name = item.get("name").and_then(Value::as_str);

            if item_type == Some("function_call") && item_name == Some("codex") {
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
                let mut input_transcript = String::new();
                if !arguments.is_empty() {
                    if let Ok(arguments_json) = serde_json::from_str::<Value>(arguments)
                        && let Some(arguments_object) = arguments_json.as_object()
                    {
                        for key in ["input_transcript", "input", "text", "prompt", "query"] {
                            if let Some(value) = arguments_object.get(key).and_then(Value::as_str) {
                                let trimmed = value.trim();
                                if !trimmed.is_empty() {
                                    input_transcript = trimmed.to_string();
                                    break;
                                }
                            }
                        }
                    }
                    if input_transcript.is_empty() {
                        input_transcript = arguments.to_string();
                    }
                }

                return Some(RealtimeEvent::HandoffRequested(RealtimeHandoffRequested {
                    handoff_id: call_id.to_string(),
                    item_id,
                    input_transcript,
                    active_transcript: Vec::new(),
                }));
            }

            item.get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .map(|item_id| RealtimeEvent::ConversationItemDone { item_id })
        }
        "error" => parsed
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
            .or_else(|| parsed.get("error").map(|error| error.to_string()))
            .map(RealtimeEvent::Error),
        _ => {
            debug!("received unsupported realtime v2 event type: {message_type}, data: {payload}");
            None
        }
    }
}
