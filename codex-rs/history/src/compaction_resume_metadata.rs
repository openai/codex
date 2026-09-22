//! Defines resume metadata stored directly on a compaction.

use crate::RolloutItem;
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

/// Returns the runtime version stored in a turn context or compaction resume metadata.
pub fn resume_multi_agent_version(item: &RolloutItem) -> Option<MultiAgentVersion> {
    match item {
        RolloutItem::TurnContext(context) => context.multi_agent_version,
        RolloutItem::Compacted(compacted) => compacted
            .resume_metadata
            .as_ref()
            .and_then(|metadata| metadata.multi_agent_version),
        RolloutItem::SessionMeta(_)
        | RolloutItem::ResponseItem(_)
        | RolloutItem::InterAgentCommunication(_)
        | RolloutItem::InterAgentCommunicationMetadata { .. }
        | RolloutItem::TokenUsageRecord(_)
        | RolloutItem::WorldState(_)
        | RolloutItem::RetainedContext(_)
        | RolloutItem::SecurityRiskScore(_)
        | RolloutItem::RealtimeItem(_)
        | RolloutItem::EventMsg(_) => None,
    }
}
