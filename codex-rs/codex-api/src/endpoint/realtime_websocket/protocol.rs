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
        Ok(value) => value,
        Err(err) => {
            debug!("failed to parse realtime event: {err}, data: {payload}");
            return None;
        }
    };

    let event_type = parsed.get("type")?.as_str()?;
    match event_type {
        "session.created" => {
            let session_id = parsed
                .pointer("/session/id")
                .and_then(Value::as_str)
                .or_else(|| parsed.get("session_id").and_then(Value::as_str))?;
            Some(RealtimeEvent::SessionCreated {
                session_id: session_id.to_string(),
            })
        }
        "session.updated" => Some(RealtimeEvent::SessionUpdated {
            backend_prompt: parsed
                .pointer("/session/backend_prompt")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }),
        "response.output_audio.delta" => {
            let data = parsed
                .get("delta")
                .and_then(Value::as_str)
                .or_else(|| parsed.get("data").and_then(Value::as_str))?;
            let sample_rate = parsed
                .get("sample_rate")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())?;
            let num_channels = parsed
                .get("num_channels")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())?;
            let samples_per_channel = parsed
                .get("samples_per_channel")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok());
            Some(RealtimeEvent::AudioOut(RealtimeAudioFrame {
                data: data.to_string(),
                sample_rate,
                num_channels,
                samples_per_channel,
            }))
        }
        "conversation.item.added" => parsed
            .get("item")
            .cloned()
            .map(RealtimeEvent::ConversationItemAdded),
        "error" => {
            let message = parsed
                .get("message")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .or_else(|| parsed.get("error").map(ToString::to_string))?;
            Some(RealtimeEvent::Error(message))
        }
        _ => None,
    }
}
