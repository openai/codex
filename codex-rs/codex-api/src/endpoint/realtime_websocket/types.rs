pub use codex_protocol::protocol::RealtimeAudioFrame;
pub use codex_protocol::protocol::RealtimeEvent;
pub use codex_protocol::protocol::RealtimeHandoffMessage;
pub use codex_protocol::protocol::RealtimeHandoffRequested;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeApiMode {
    V1,
    V2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeSessionConfig {
    pub instructions: String,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub mode: RealtimeApiMode,
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
    #[serde(rename = "response.create")]
    ResponseCreate,
    #[serde(rename = "session.update")]
    SessionUpdate { session: SessionUpdateSession },
    #[serde(rename = "conversation.item.create")]
    ConversationItemCreate { item: ConversationItem },
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(super) enum SessionUpdateSession {
    V1(SessionUpdateSessionV1),
    V2(SessionUpdateSessionV2),
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionUpdateSessionV1 {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) instructions: String,
    pub(super) audio: SessionAudioV1,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionUpdateSessionV2 {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) instructions: String,
    pub(super) output_modalities: Vec<String>,
    pub(super) audio: SessionAudioV2,
    pub(super) tools: Vec<SessionTool>,
    pub(super) tool_choice: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioV1 {
    pub(super) input: SessionAudioInputV1,
    pub(super) output: SessionAudioOutputV1,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioV2 {
    pub(super) input: SessionAudioInputV2,
    pub(super) output: SessionAudioOutputV2,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioInputV1 {
    pub(super) format: SessionAudioFormat,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioInputV2 {
    pub(super) format: SessionAudioFormat,
    pub(super) turn_detection: SessionTurnDetection,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioFormat {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) rate: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionTurnDetection {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) interrupt_response: bool,
    pub(super) create_response: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioOutputV1 {
    pub(super) voice: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioOutputV2 {
    pub(super) format: SessionAudioOutputFormat,
    pub(super) voice: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioOutputFormat {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) rate: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionTool {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) parameters: SessionToolParameters,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionToolParameters {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) properties: SessionToolProperties,
    pub(super) required: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionToolProperties {
    pub(super) prompt: SessionToolProperty,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionToolProperty {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub(super) enum ConversationItem {
    #[serde(rename = "message")]
    Message {
        role: String,
        content: Vec<ConversationItemContent>,
    },
    #[serde(rename = "function_call_output")]
    FunctionCallOutput { call_id: String, output: String },
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ConversationItemContent {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) text: String,
}
