use crate::endpoint::realtime_websocket::protocol_v1::parse_realtime_event_v1;
use crate::endpoint::realtime_websocket::protocol_v2::parse_realtime_event_v2;
pub use codex_protocol::protocol::RealtimeAudioFrame;
pub use codex_protocol::protocol::RealtimeEvent;
pub use codex_protocol::protocol::RealtimeHandoffRequested;
pub use codex_protocol::protocol::RealtimeTranscriptDelta;
pub use codex_protocol::protocol::RealtimeTranscriptEntry;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeEventParser {
    V1,
    RealtimeV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeSessionMode {
    Conversational,
    Transcription,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeSessionConfig {
    pub instructions: String,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub event_parser: RealtimeEventParser,
    pub session_mode: RealtimeSessionMode,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub(super) enum RealtimeOutboundMessage {
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
    ConversationItemCreate { item: ConversationItemPayload },
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionUpdateSession {
    #[serde(rename = "type")]
    pub(super) r#type: SessionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) output_modalities: Option<Vec<String>>,
    pub(super) audio: SessionAudio,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tools: Option<Vec<SessionFunctionTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_choice: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SessionType {
    Quicksilver,
    Realtime,
    Transcription,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudio {
    pub(super) input: SessionAudioInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) output: Option<SessionAudioOutput>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioInput {
    pub(super) format: SessionAudioFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) noise_reduction: Option<SessionNoiseReduction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) turn_detection: Option<SessionTurnDetection>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioFormat {
    #[serde(rename = "type")]
    pub(super) r#type: AudioFormatType,
    pub(super) rate: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(super) enum AudioFormatType {
    #[serde(rename = "audio/pcm")]
    AudioPcm,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) format: Option<SessionAudioOutputFormat>,
    pub(super) voice: SessionAudioVoice,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(super) enum SessionAudioVoice {
    #[serde(rename = "fathom")]
    Fathom,
    #[serde(rename = "marin")]
    Marin,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionNoiseReduction {
    #[serde(rename = "type")]
    pub(super) r#type: NoiseReductionType,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum NoiseReductionType {
    NearField,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionTurnDetection {
    #[serde(rename = "type")]
    pub(super) r#type: TurnDetectionType,
    pub(super) interrupt_response: bool,
    pub(super) create_response: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TurnDetectionType {
    ServerVad,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionAudioOutputFormat {
    #[serde(rename = "type")]
    pub(super) r#type: AudioFormatType,
    pub(super) rate: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ConversationMessageItem {
    #[serde(rename = "type")]
    pub(super) r#type: ConversationItemType,
    pub(super) role: ConversationRole,
    pub(super) content: Vec<ConversationItemContent>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ConversationItemType {
    Message,
    FunctionCallOutput,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ConversationRole {
    User,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(super) enum ConversationItemPayload {
    Message(ConversationMessageItem),
    FunctionCallOutput(ConversationFunctionCallOutputItem),
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ConversationFunctionCallOutputItem {
    #[serde(rename = "type")]
    pub(super) r#type: ConversationItemType,
    pub(super) call_id: String,
    pub(super) output: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ConversationItemContent {
    #[serde(rename = "type")]
    pub(super) r#type: ConversationContentType,
    pub(super) text: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ConversationContentType {
    Text,
    InputText,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SessionFunctionTool {
    #[serde(rename = "type")]
    pub(super) r#type: SessionToolType,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) parameters: Value,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SessionToolType {
    Function,
}

pub(super) fn parse_realtime_event(
    payload: &str,
    event_parser: RealtimeEventParser,
) -> Option<RealtimeEvent> {
    match event_parser {
        RealtimeEventParser::V1 => parse_realtime_event_v1(payload),
        RealtimeEventParser::RealtimeV2 => parse_realtime_event_v2(payload),
    }
}
