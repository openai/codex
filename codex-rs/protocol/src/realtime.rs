use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// A realtime thread item persisted in the canonical rollout.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
pub struct RealtimeItem {
    pub id: String,
    pub realtime_session_id: String,
    #[serde(flatten)]
    pub content: RealtimeItemContent,
}

/// The minimum facts needed to interleave realtime speech and agent work.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum RealtimeItemContent {
    RealtimeSessionStarted,
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
