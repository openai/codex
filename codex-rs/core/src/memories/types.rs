use codex_protocol::ThreadId;
use serde::Deserialize;
use std::path::PathBuf;

/// A rollout selected for stage-1 memory extraction during startup.
#[derive(Debug, Clone)]
pub(crate) struct RolloutCandidate {
    /// Source thread identifier for this rollout.
    pub(crate) thread_id: ThreadId,
    /// Absolute path to the rollout file to summarize.
    pub(crate) rollout_path: PathBuf,
    /// Thread working directory used for per-project memory bucketing.
    pub(crate) cwd: PathBuf,
    /// Last observed thread update timestamp (RFC3339), if available.
    pub(crate) updated_at: Option<String>,
}

/// Parsed stage-1 model output payload.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StageOneOutput {
    /// Detailed markdown raw memory for a single rollout.
    #[serde(rename = "rawMemory", alias = "traceMemory")]
    pub(crate) raw_memory: String,
    /// Compact summary line used for routing and indexing.
    pub(crate) summary: String,
}
