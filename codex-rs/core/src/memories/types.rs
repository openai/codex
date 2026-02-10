use serde::Deserialize;

/// Parsed stage-1 model output payload.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct StageOneOutput {
    /// Detailed markdown raw memory for a single rollout.
    #[serde(rename = "raw_memory", alias = "rawMemory", alias = "traceMemory")]
    pub(super) raw_memory: String,
    /// Optional rollout slug from the model output. Accepted but ignored.
    #[serde(default)]
    pub(super) rollout_slug: Option<String>,
    /// Compact summary line used for routing and indexing.
    #[serde(rename = "rollout_summary", alias = "summary")]
    pub(super) rollout_summary: String,
}
