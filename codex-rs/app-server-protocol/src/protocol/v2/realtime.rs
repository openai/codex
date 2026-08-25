use crate::JsonSchema;
use crate::TS;
use codex_protocol::protocol::CodexResponseHandoffMode;
use codex_protocol::protocol::ConversationTextRole;
use codex_protocol::protocol::RealtimeAudioFrame as CoreRealtimeAudioFrame;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RealtimeVoice;
use codex_protocol::protocol::RealtimeVoicesList;
use codex_protocol::realtime::BemItemPresentation as CoreBemItemPresentation;
use codex_protocol::realtime::RealtimeItem as CoreRealtimeItem;
use codex_protocol::realtime::RealtimeItemContent as CoreRealtimeItemContent;
use codex_protocol::realtime::RealtimeSessionOutcome as CoreRealtimeSessionOutcome;
use codex_protocol::realtime::RealtimeTranscriptRole as CoreRealtimeTranscriptRole;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

/// EXPERIMENTAL - a thread-scoped realtime item in the canonical timeline.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeItem {
    pub id: String,
    pub realtime_session_id: String,
    #[serde(flatten)]
    pub content: ThreadRealtimeItemContent,
}

/// EXPERIMENTAL - durable facts describing realtime speech and promoted work.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "type", rename_all = "camelCase", export_to = "v2/")]
pub enum ThreadRealtimeItemContent {
    RealtimeSessionStarted,
    TranscriptSegment {
        role: ThreadRealtimeTranscriptRole,
        text: String,
    },
    BemItemPromoted {
        turn_id: String,
        item_id: String,
        presentation: ThreadRealtimeBemItemPresentation,
    },
    RealtimeSessionClosed {
        outcome: ThreadRealtimeSessionOutcome,
    },
}

/// EXPERIMENTAL - how an existing agent item appears in a realtime conversation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "type", rename_all = "camelCase", export_to = "v2/")]
pub enum ThreadRealtimeBemItemPresentation {
    WholeItem,
    InlineMarkdown,
    InlineVisualization { index: u32 },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum ThreadRealtimeTranscriptRole {
    User,
    Assistant,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum ThreadRealtimeSessionOutcome {
    Ended,
    Failed,
}

impl From<CoreRealtimeItem> for ThreadRealtimeItem {
    fn from(item: CoreRealtimeItem) -> Self {
        let CoreRealtimeItem {
            id,
            realtime_session_id,
            content,
        } = item;
        let content = match content {
            CoreRealtimeItemContent::RealtimeSessionStarted => {
                ThreadRealtimeItemContent::RealtimeSessionStarted
            }
            CoreRealtimeItemContent::TranscriptSegment { role, text } => {
                ThreadRealtimeItemContent::TranscriptSegment {
                    role: match role {
                        CoreRealtimeTranscriptRole::User => ThreadRealtimeTranscriptRole::User,
                        CoreRealtimeTranscriptRole::Assistant => {
                            ThreadRealtimeTranscriptRole::Assistant
                        }
                    },
                    text,
                }
            }
            CoreRealtimeItemContent::BemItemPromoted {
                turn_id,
                item_id,
                presentation,
            } => ThreadRealtimeItemContent::BemItemPromoted {
                turn_id,
                item_id,
                presentation: match presentation {
                    CoreBemItemPresentation::WholeItem => {
                        ThreadRealtimeBemItemPresentation::WholeItem
                    }
                    CoreBemItemPresentation::InlineMarkdown => {
                        ThreadRealtimeBemItemPresentation::InlineMarkdown
                    }
                    CoreBemItemPresentation::InlineVisualization { index } => {
                        ThreadRealtimeBemItemPresentation::InlineVisualization { index }
                    }
                },
            },
            CoreRealtimeItemContent::RealtimeSessionClosed { outcome } => {
                ThreadRealtimeItemContent::RealtimeSessionClosed {
                    outcome: match outcome {
                        CoreRealtimeSessionOutcome::Ended => ThreadRealtimeSessionOutcome::Ended,
                        CoreRealtimeSessionOutcome::Failed => ThreadRealtimeSessionOutcome::Failed,
                    },
                }
            }
        };
        Self {
            id,
            realtime_session_id,
            content,
        }
    }
}

/// EXPERIMENTAL - thread realtime audio chunk.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAudioChunk {
    pub data: String,
    pub sample_rate: u32,
    pub num_channels: u16,
    pub samples_per_channel: Option<u32>,
    pub item_id: Option<String>,
}

impl From<CoreRealtimeAudioFrame> for ThreadRealtimeAudioChunk {
    fn from(value: CoreRealtimeAudioFrame) -> Self {
        let CoreRealtimeAudioFrame {
            data,
            sample_rate,
            num_channels,
            samples_per_channel,
            item_id,
        } = value;
        Self {
            data,
            sample_rate,
            num_channels,
            samples_per_channel,
            item_id,
        }
    }
}

impl From<ThreadRealtimeAudioChunk> for CoreRealtimeAudioFrame {
    fn from(value: ThreadRealtimeAudioChunk) -> Self {
        let ThreadRealtimeAudioChunk {
            data,
            sample_rate,
            num_channels,
            samples_per_channel,
            item_id,
        } = value;
        Self {
            data,
            sample_rate,
            num_channels,
            samples_per_channel,
            item_id,
        }
    }
}

/// EXPERIMENTAL - start a thread-scoped realtime session.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeStartParams {
    pub thread_id: String,
    /// Leaves Codex response handoffs to the client's explicit append calls instead of forwarding
    /// them automatically. Defaults to false.
    #[ts(optional = nullable)]
    pub client_managed_handoffs: Option<bool>,
    /// Controls whether a realtime V3 delegation produces an acknowledgement filler.
    /// Omitted values preserve the Realtime API's default behavior.
    #[ts(optional = nullable)]
    pub delegation_ack_filler: Option<bool>,
    /// Routes any transcript tail remaining at session end through Codex. Defaults to false.
    /// TODO: Remove this rollout knob once transcript-tail flushing is always enabled.
    #[ts(optional = nullable)]
    pub flush_transcript_tail_on_session_end: Option<bool>,
    // TODO: Remove this experiment-only delivery path after response-item testing is complete.
    /// Sends automatic Codex responses as realtime conversation items instead of handoff appends.
    #[ts(optional = nullable)]
    pub codex_responses_as_items: Option<bool>,
    // TODO: Remove this experiment-only prefix with `codex_responses_as_items`.
    /// Optional prefix added to automatic Codex response items when `codexResponsesAsItems` is true.
    #[ts(optional = nullable)]
    pub codex_response_item_prefix: Option<String>,
    /// Selects how automatic Codex responses are routed in Frameless Bidi sessions. Omitted values
    /// default to `thinking`. Realtime V1 and V2 ignore this setting.
    #[ts(optional = nullable)]
    pub codex_response_handoff_mode: Option<CodexResponseHandoffMode>,
    /// Overrides BEM channel prefixes by `analysis`, `commentary`, or `final`.
    /// Omitted channels retain their default uppercase bracketed prefixes.
    #[ts(optional = nullable)]
    pub codex_response_handoff_channel_prefixes: Option<BTreeMap<String, Vec<String>>>,
    /// Overrides the configured realtime model for this session only.
    #[ts(optional = nullable)]
    pub model: Option<String>,
    /// Selects text or audio output for the realtime session. Transport and voice stay
    /// independent so clients can choose how they connect separately from what the model emits.
    pub output_modality: RealtimeOutputModality,
    /// Set to false to start without Codex's startup context. Omitted or null includes it.
    #[ts(optional = nullable)]
    pub include_startup_context: Option<bool>,
    /// Adds complete role-bearing text items to the initial Frameless Bidi session history.
    /// This is only supported by realtime V3 and is sent during session startup. Requests are
    /// limited to 128 items and 8,192 estimated text tokens in total.
    #[ts(optional = nullable)]
    pub initial_items: Option<Vec<ThreadRealtimeInitialItem>>,
    /// Developer instructions given to the backing Codex model when this realtime session starts.
    #[ts(optional = nullable)]
    pub realtime_start_instructions: Option<String>,
    /// Developer instructions given to the backing Codex model when this realtime session ends.
    #[ts(optional = nullable)]
    pub realtime_end_instructions: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(optional = nullable)]
    pub prompt: Option<Option<String>>,
    #[ts(optional = nullable)]
    pub realtime_session_id: Option<String>,
    #[ts(optional = nullable)]
    pub transport: Option<ThreadRealtimeStartTransport>,
    /// Overrides the configured realtime protocol version for this session only.
    #[ts(optional = nullable)]
    pub version: Option<RealtimeConversationVersion>,
    #[ts(optional = nullable)]
    pub voice: Option<RealtimeVoice>,
}

/// EXPERIMENTAL - role-bearing text item included when a realtime V3 session starts.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeInitialItem {
    pub role: ConversationTextRole,
    pub text: String,
}

/// EXPERIMENTAL - transport used by thread realtime.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export_to = "v2/", tag = "type")]
pub enum ThreadRealtimeStartTransport {
    Websocket,
    Webrtc {
        /// SDP offer generated by a WebRTC RTCPeerConnection after configuring audio and the
        /// realtime events data channel.
        sdp: String,
    },
    ExistingCall {
        /// Identifier of a realtime call already created and negotiated by the client.
        #[serde(rename = "callId")]
        #[ts(rename = "callId")]
        call_id: String,
    },
}

/// EXPERIMENTAL - response for starting thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeStartResponse {}

/// EXPERIMENTAL - append audio input to thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAppendAudioParams {
    pub thread_id: String,
    pub audio: ThreadRealtimeAudioChunk,
}

/// EXPERIMENTAL - response for appending realtime audio input.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAppendAudioResponse {}

/// EXPERIMENTAL - append text input to thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAppendTextParams {
    pub thread_id: String,
    pub text: String,
    #[serde(default)]
    pub role: ConversationTextRole,
}

/// EXPERIMENTAL - response for appending realtime text input.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAppendTextResponse {}

/// EXPERIMENTAL - append speakable text to thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAppendSpeechParams {
    pub thread_id: String,
    pub text: String,
}

/// EXPERIMENTAL - response for appending realtime speech.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeAppendSpeechResponse {}

/// EXPERIMENTAL - stop thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeStopParams {
    pub thread_id: String,
}

/// EXPERIMENTAL - response for stopping thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeStopResponse {}

/// EXPERIMENTAL - list voices supported by thread realtime.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeListVoicesParams {}

/// EXPERIMENTAL - response for listing supported realtime voices.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeListVoicesResponse {
    pub voices: RealtimeVoicesList,
}

/// EXPERIMENTAL - emitted when thread realtime startup is accepted.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeStartedNotification {
    pub thread_id: String,
    pub realtime_session_id: Option<String>,
    pub version: RealtimeConversationVersion,
}

/// EXPERIMENTAL - raw non-audio thread realtime item emitted by the backend.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeItemAddedNotification {
    pub thread_id: String,
    pub item: JsonValue,
}

/// EXPERIMENTAL - a realtime timeline item started before its content streams.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeItemStartedNotification {
    pub thread_id: String,
    pub item: ThreadRealtimeItem,
}

/// EXPERIMENTAL - text appended to an active realtime transcript item.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeItemTranscriptDeltaNotification {
    pub thread_id: String,
    pub item_id: String,
    pub delta: String,
}

/// EXPERIMENTAL - a realtime timeline item published after canonical commit.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeItemCompletedNotification {
    pub thread_id: String,
    pub item: ThreadRealtimeItem,
}

/// EXPERIMENTAL - flat transcript delta emitted whenever realtime
/// transcript text changes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeTranscriptDeltaNotification {
    pub thread_id: String,
    pub role: String,
    /// Live transcript delta from the realtime event.
    pub delta: String,
}

/// EXPERIMENTAL - final transcript text emitted when realtime completes
/// a transcript part.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeTranscriptDoneNotification {
    pub thread_id: String,
    pub role: String,
    /// Final complete text for the transcript part.
    pub text: String,
}

/// EXPERIMENTAL - streamed output audio emitted by thread realtime.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeOutputAudioDeltaNotification {
    pub thread_id: String,
    pub audio: ThreadRealtimeAudioChunk,
}

/// EXPERIMENTAL - emitted with the remote SDP for a WebRTC realtime session.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeSdpNotification {
    pub thread_id: String,
    pub sdp: String,
}

/// EXPERIMENTAL - emitted when thread realtime encounters an error.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeErrorNotification {
    pub thread_id: String,
    pub message: String,
}

/// EXPERIMENTAL - emitted when thread realtime transport closes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeClosedNotification {
    pub thread_id: String,
    pub reason: Option<String>,
}
