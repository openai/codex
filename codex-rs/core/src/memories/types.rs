use codex_protocol::ThreadId;
use serde::Deserialize;
use std::path::PathBuf;

/// A rollout selected for stage-1 memory extraction during startup.
#[derive(Debug, Clone)]
pub(super) struct RolloutCandidate {
    /// Source thread identifier for this rollout.
    pub(super) thread_id: ThreadId,
    /// Absolute path to the rollout file to summarize.
    pub(super) rollout_path: PathBuf,
    /// Thread working directory used for per-project memory bucketing.
    pub(super) cwd: PathBuf,
    /// Thread update timestamp (unix seconds) used for stage-1 staleness checks.
    pub(super) source_updated_at: i64,
}

/// Parsed stage-1 model output payload.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct StageOneOutput {
    /// Detailed markdown raw memory for a single rollout.
    #[serde(rename = "rawMemory", alias = "traceMemory")]
    pub(super) raw_memory: String,
    /// Compact summary line used for routing and indexing.
    pub(super) summary: String,
}
