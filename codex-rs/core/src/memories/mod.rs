//! Memory subsystem for startup extraction and consolidation.
//!
//! The startup memory pipeline is split into two phases:
//! - Phase 1: select rollouts, extract stage-1 raw memories, persist stage-1 outputs, and enqueue consolidation.
//! - Phase 2: claim a global consolidation lock, materialize consolidation inputs, and dispatch one consolidation agent.

mod dispatch;
mod phase1;
mod phase2;
pub(crate) mod prompts;
mod start;
mod storage;
#[cfg(test)]
mod tests;

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
    /// Prompt used for phase 1.
    pub(super) const PROMPT: &str = include_str!("../../templates/memories/stage_one_system.md");
    /// Maximum number of rollout candidates processed per startup pass.
    pub(super) const MAX_ROLLOUTS_PER_STARTUP: usize = 8;
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
    /// Maximum rollout age considered for phase-1 extraction.
    pub(super) const MAX_ROLLOUT_AGE_DAYS: i64 = 30;
    /// Minimum rollout idle time required before phase-1 extraction.
    pub(super) const MIN_ROLLOUT_IDLE_HOURS: i64 = 12;
    /// Lease duration (seconds) for phase-1 job ownership.
    pub(super) const JOB_LEASE_SECONDS: i64 = 3_600;
    /// Backoff delay (seconds) before retrying a failed stage-1 extraction job.
    pub(super) const JOB_RETRY_DELAY_SECONDS: i64 = 3_600;
    /// Maximum number of threads to scan.
    pub(super) const THREAD_SCAN_LIMIT: usize = 5_000;
}

/// Phase 2 (aka `Consolidation`).
mod phase_two {
    /// Subagent source label used to identify consolidation tasks.
    pub(super) const MEMORY_CONSOLIDATION_SUBAGENT_LABEL: &str = "memory_consolidation";
    /// Maximum number of recent raw memories retained for global consolidation.
    pub(super) const MAX_RAW_MEMORIES_FOR_GLOBAL: usize = 1_024;
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
    /// Number of raw memories produced by phase-1 startup extraction.
    pub(super) const MEMORY_PHASE_ONE_OUTPUT: &str = "codex.memory.phase1.output";
    /// Number of phase-2 startup jobs grouped by status.
    pub(super) const MEMORY_PHASE_TWO_JOBS: &str = "codex.memory.phase2";
    /// Number of stage-1 memories included in each phase-2 consolidation step.
    pub(super) const MEMORY_PHASE_TWO_INPUT: &str = "codex.memory.phase2.input";
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
