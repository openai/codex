pub use codex_protocol::protocol::RealtimeAudioFrame;
pub use codex_protocol::protocol::RealtimeEvent;
pub use codex_protocol::protocol::RealtimeHandoffRequested;
pub use codex_protocol::protocol::RealtimeTranscriptDelta;
pub use codex_protocol::protocol::RealtimeTranscriptEntry;
use serde::Serialize;
use serde_json::Value;
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeSessionConfig {
    pub instructions: String,
    pub model: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub(super) enum RealtimeOutboundMessage {
    #[serde(rename = "input_audio_buffer.append")]
    InputAudioBufferAppend { audio: String },
    #[serde(rename = "conversation.handoff.append")]
    ConversationHandoffAppend {
        handoff_id: String,
        output_text: String,
    },
    #[serde(rename = "session.update")]
    SessionUpdate { session: SessionUpdateSession },
    #[serde(rename = "conversation.item.create")]
    ConversationItemCreate { item: ConversationItem },
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionUpdateSession {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) instructions: String,
    pub(super) audio: SessionAudio,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudio {
    pub(super) input: SessionAudioInput,
    pub(super) output: SessionAudioOutput,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioInput {
    pub(super) format: SessionAudioFormat,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioFormat {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) rate: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioOutput {
    pub(super) voice: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ConversationItem {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) role: String,
    pub(super) content: Vec<ConversationItemContent>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ConversationItemContent {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) text: String,
}

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
        "conversation.output_audio.delta" => {
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
        "conversation.input_transcript.delta" => parsed
            .get("delta")
            .and_then(Value::as_str)
            .map(str::to_string)
            .map(|delta| RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta { delta })),
        "conversation.output_transcript.delta" => parsed
            .get("delta")
            .and_then(Value::as_str)
            .map(str::to_string)
            .map(|delta| RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta { delta })),
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
            .or_else(|| parsed.get("error").map(std::string::ToString::to_string))
            .map(RealtimeEvent::Error),
        _ => {
            debug!("received unsupported realtime event type: {message_type}, data: {payload}");
            None
        }
    }
}
