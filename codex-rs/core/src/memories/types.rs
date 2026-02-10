use serde::Deserialize;

/// Parsed stage-1 model output payload.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct StageOneOutput {
    /// Detailed markdown raw memory for a single rollout.
    #[serde(rename = "rawMemory", alias = "traceMemory")]
    pub(super) raw_memory: String,
    /// Compact summary line used for routing and indexing.
    pub(super) summary: String,
}
