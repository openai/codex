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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeConnectionState {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealtimeEvent {
    State(RealtimeConnectionState),
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

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RealtimeInboundMessage {
    #[serde(rename = "session.created")]
    SessionCreated {
        session_id: Option<String>,
        session: Option<RealtimeInboundSession>,
    },
    #[serde(rename = "session.updated")]
    SessionUpdated {
        session: Option<RealtimeInboundSession>,
    },
    #[serde(rename = "response.output_audio.delta")]
    OutputAudioDelta {
        delta: Option<String>,
        data: Option<String>,
        sample_rate: Option<u32>,
        num_channels: Option<u16>,
        samples_per_channel: Option<u32>,
    },
    #[serde(rename = "conversation.item.added")]
    ConversationItemAdded { item: Option<Value> },
    #[serde(rename = "error")]
    Error {
        error: Option<Value>,
        message: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct RealtimeInboundSession {
    id: Option<String>,
    backend_prompt: Option<String>,
}

pub(super) fn parse_realtime_event(payload: &str) -> Option<RealtimeEvent> {
    let parsed: RealtimeInboundMessage = match serde_json::from_str(payload) {
        Ok(msg) => msg,
        Err(err) => {
            debug!("failed to parse realtime event: {err}, data: {payload}");
            return None;
        }
    };

    match parsed {
        RealtimeInboundMessage::SessionCreated {
            session_id,
            session,
        } => {
            let session_id = session.and_then(|s| s.id).or(session_id);
            session_id.map(|id| RealtimeEvent::SessionCreated { session_id: id })
        }
        RealtimeInboundMessage::SessionUpdated { session } => Some(RealtimeEvent::SessionUpdated {
            backend_prompt: session.and_then(|s| s.backend_prompt),
        }),
        RealtimeInboundMessage::OutputAudioDelta {
            delta,
            data,
            sample_rate,
            num_channels,
            samples_per_channel,
        } => {
            let data = delta.or(data)?;
            let sample_rate = sample_rate?;
            let num_channels = num_channels?;
            Some(RealtimeEvent::AudioOut(RealtimeAudioFrame {
                data,
                sample_rate,
                num_channels,
                samples_per_channel,
            }))
        }
        RealtimeInboundMessage::ConversationItemAdded { item } => {
            item.map(RealtimeEvent::ConversationItemAdded)
        }
        RealtimeInboundMessage::Error { error, message } => {
            let message = message.or_else(|| error.map(|e| e.to_string()))?;
            Some(RealtimeEvent::Error(message))
        }
    }
}
