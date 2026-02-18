use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeSessionConfig {
    pub api_url: String,
    pub prompt: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealtimeAudioFrame {
    pub data: String,
    pub sample_rate: u32,
    pub num_channels: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub samples_per_channel: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealtimeEvent {
    SessionCreated { session_id: String },
    SessionUpdated { backend_prompt: Option<String> },
    AudioOut(RealtimeAudioFrame),
    ConversationItemAdded(Value),
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub(super) enum RealtimeOutboundMessage {
    #[serde(rename = "response.input_audio.delta")]
    InputAudioDelta {
        delta: String,
        sample_rate: u32,
        num_channels: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        samples_per_channel: Option<u32>,
    },
    #[serde(rename = "session.create")]
    SessionCreate { session: SessionCreateSession },
    #[serde(rename = "session.update")]
    SessionUpdate {
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<SessionUpdateSession>,
    },
    #[serde(rename = "conversation.item.create")]
    ConversationItemCreate { item: ConversationItem },
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionUpdateSession {
    pub(super) backend_prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionCreateSession {
    pub(super) backend_prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) conversation_id: Option<String>,
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
        "session.created" => {
            let session = parsed.get("session").and_then(Value::as_object);
            let session_id = session
                .and_then(|session| session.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    parsed
                        .get("session_id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            session_id.map(|id| RealtimeEvent::SessionCreated { session_id: id })
        }
        "session.updated" => {
            let backend_prompt = parsed
                .get("session")
                .and_then(Value::as_object)
                .and_then(|session| session.get("backend_prompt"))
                .and_then(Value::as_str)
                .map(str::to_string);
            Some(RealtimeEvent::SessionUpdated { backend_prompt })
        }
        "response.output_audio.delta" => {
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
                .get("num_channels")
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
        "conversation.item.added" => parsed
            .get("item")
            .cloned()
            .map(RealtimeEvent::ConversationItemAdded),
        "error" => parsed
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| parsed.get("error").map(std::string::ToString::to_string))
            .map(RealtimeEvent::Error),
        _ => {
            debug!("received unsupported realtime event type: {message_type}, data: {payload}");
            None
        }
    }
}
