//! Memory subsystem for startup extraction and consolidation.
//!
//! The startup memory pipeline is split into two phases:
//! - Phase 1: select rollouts, extract stage-1 raw memories, persist stage-1 outputs, and enqueue consolidation.
//! - Phase 2: claim a global consolidation lock, materialize consolidation inputs, and dispatch one consolidation agent.

mod phase1;
mod phase2;
pub(crate) mod prompts;
mod start;
mod storage;
#[cfg(test)]
mod tests;
pub(crate) mod usage;

/// Starts the memory startup pipeline for eligible root sessions.
/// This is the single entrypoint that `codex` uses to trigger memory startup.
///
/// This is the entry point to read and understand this module.
pub(crate) use start::start_memories_startup_task;

mod artifacts {
    pub(super) const ROLLOUT_SUMMARIES_SUBDIR: &str = "rollout_summaries";
    pub(super) const RAW_MEMORIES_FILENAME: &str = "raw_memories.md";
}

/// Phase 1 (startup extraction).
mod phase_one {
    /// Default model used for phase 1.
    pub(super) const MODEL: &str = "gpt-5.1-codex-mini";
    /// Prompt used for phase 1.
    pub(super) const PROMPT: &str = include_str!("../../templates/memories/stage_one_system.md");
    /// Concurrency cap for startup memory extraction and consolidation scheduling.
    pub(super) const CONCURRENCY_LIMIT: usize = 8;
    /// Fallback stage-1 rollout truncation limit (tokens) when model metadata
    /// does not include a valid context window.
    pub(super) const DEFAULT_STAGE_ONE_ROLLOUT_TOKEN_LIMIT: usize = 150_000;
    /// Maximum number of tokens from `memory_summary.md` injected into memory
    /// tool developer instructions.
    pub(super) const MEMORY_TOOL_DEVELOPER_INSTRUCTIONS_SUMMARY_TOKEN_LIMIT: usize = 5_000;
    /// Portion of the model effective input window reserved for the stage-1
    /// rollout input.
    ///
    /// Keeping this below 100% leaves room for system instructions, prompt
    /// framing, and model output.
    pub(super) const CONTEXT_WINDOW_PERCENT: i64 = 70;
    /// Lease duration (seconds) for phase-1 job ownership.
    pub(super) const JOB_LEASE_SECONDS: i64 = 3_600;
    /// Backoff delay (seconds) before retrying a failed stage-1 extraction job.
    pub(super) const JOB_RETRY_DELAY_SECONDS: i64 = 3_600;
    /// Maximum number of threads to scan.
    pub(super) const THREAD_SCAN_LIMIT: usize = 5_000;
}

/// Phase 2 (aka `Consolidation`).
mod phase_two {
    /// Default model used for phase 2.
    pub(super) const MODEL: &str = "gpt-5.3-codex";
    /// Lease duration (seconds) for phase-2 consolidation job ownership.
    pub(super) const JOB_LEASE_SECONDS: i64 = 3_600;
    /// Backoff delay (seconds) before retrying a failed phase-2 consolidation
    /// job.
    pub(super) const JOB_RETRY_DELAY_SECONDS: i64 = 3_600;
    /// Heartbeat interval (seconds) for phase-2 running jobs.
    pub(super) const JOB_HEARTBEAT_SECONDS: u64 = 90;
}

mod metrics {
    /// Number of phase-1 startup jobs grouped by status.
    pub(super) const MEMORY_PHASE_ONE_JOBS: &str = "codex.memory.phase1";
    /// End-to-end latency for a single phase-1 startup run.
    pub(super) const MEMORY_PHASE_ONE_E2E_MS: &str = "codex.memory.phase1.e2e_ms";
    /// Number of raw memories produced by phase-1 startup extraction.
    pub(super) const MEMORY_PHASE_ONE_OUTPUT: &str = "codex.memory.phase1.output";
    /// Histogram for aggregate token usage across one phase-1 startup run.
    pub(super) const MEMORY_PHASE_ONE_TOKEN_USAGE: &str = "codex.memory.phase1.token_usage";
    /// Number of phase-2 startup jobs grouped by status.
    pub(super) const MEMORY_PHASE_TWO_JOBS: &str = "codex.memory.phase2";
    /// End-to-end latency for a single phase-2 consolidation run.
    pub(super) const MEMORY_PHASE_TWO_E2E_MS: &str = "codex.memory.phase2.e2e_ms";
    /// Number of stage-1 memories included in each phase-2 consolidation step.
    pub(super) const MEMORY_PHASE_TWO_INPUT: &str = "codex.memory.phase2.input";
    /// Histogram for aggregate token usage across one phase-2 consolidation run.
    pub(super) const MEMORY_PHASE_TWO_TOKEN_USAGE: &str = "codex.memory.phase2.token_usage";
}

use std::path::Path;
use std::path::PathBuf;

pub fn memory_root(codex_home: &Path) -> PathBuf {
    codex_home.join("memories")
}

fn rollout_summaries_dir(root: &Path) -> PathBuf {
    root.join(artifacts::ROLLOUT_SUMMARIES_SUBDIR)
}

fn raw_memories_file(root: &Path) -> PathBuf {
    root.join(artifacts::RAW_MEMORIES_FILENAME)
}

async fn ensure_layout(root: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(rollout_summaries_dir(root)).await
}
