use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// A realtime thread item persisted in the canonical rollout.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
pub struct RealtimeItem {
    pub id: String,
    pub realtime_session_id: String,
    /// Owner-assigned Unix milliseconds at first observation, retained when sealed.
    /// Absent for history recorded before canonical observation times were introduced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null", optional)]
    pub observed_at_ms: Option<i64>,
    #[serde(flatten)]
    pub content: RealtimeItemContent,
}

/// A frame holds one or more payloads, at most one per channel. They share one ID,
/// observation time, and seal. V1 requires its only channel, `voice_transcript`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
pub struct RealtimeFrameChannels {
    pub voice_transcript: RealtimeVoiceTranscript,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
pub struct RealtimeVoiceTranscript {
    pub role: RealtimeTranscriptRole,
    pub text: String,
}

/// The minimum facts needed to interleave realtime speech and agent work.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum RealtimeItemContent {
    RealtimeSessionStarted,
    Frame {
        channels: RealtimeFrameChannels,
    },
    /// Original rollout spelling, retained for older histories and unenrolled writers.
    TranscriptSegment {
        role: RealtimeTranscriptRole,
        text: String,
    },
    BemItemPromoted {
        turn_id: String,
        item_id: String,
        presentation: BemItemPresentation,
    },
    RealtimeSessionClosed {
        outcome: RealtimeSessionOutcome,
    },
    /// Durable acceptance of the exact bounded input used by an agent turn.
    /// The enclosing item ID is the acceptance identity; source IDs never use
    /// provisional rollout ordinals.
    TurnInputAccepted {
        handoff_id: String,
        turn_id: String,
        mode: RealtimeHandoffMode,
        source_event_ids: Vec<String>,
        input_item_id: String,
        input: String,
    },
    /// End-of-call input recorded in model history without starting or steering a turn.
    SessionEndInputSaved {
        source_event_ids: Vec<String>,
        input_item_id: String,
        input: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum RealtimeHandoffMode {
    Start,
    Steer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum RealtimeSessionOutcome {
    Ended,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum RealtimeTranscriptRole {
    User,
    Assistant,
}

/// How an existing agent item is presented in the realtime conversation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum BemItemPresentation {
    WholeItem,
    InlineMarkdown,
    InlineVisualization { index: u32 },
}

#[cfg(test)]
#[path = "realtime_tests.rs"]
mod tests;
