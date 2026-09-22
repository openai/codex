//! Defines resume metadata stored directly on a compaction.

use codex_protocol::protocol::MultiAgentVersion;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

/// Resume metadata that is not represented by the companion records after a compaction.
///
/// Presence distinguishes compactions that explicitly persisted these values from older
/// compactions that did not. Each field is authoritative, including an intentionally absent value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CompactionResumeMetadata {
    /// Selected runtime, including when the companion turn context is absent.
    pub multi_agent_version: Option<MultiAgentVersion>,
    /// Turn identity used to admit continuations after cold resume.
    pub last_started_turn_id: Option<String>,
    pub previous_turn_settings: Option<PreviousTurnSettings>,
}

/// Previous user-turn settings used to reconstruct context changes after resume.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PreviousTurnSettings {
    pub model: String,
    pub comp_hash: Option<String>,
    pub realtime_active: Option<bool>,
}
