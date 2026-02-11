//! Memory subsystem for startup extraction and consolidation.
//!
//! The startup memory pipeline is split into two phases:
//! - Phase 1: select rollouts, extract stage-1 raw memories, persist stage-1 outputs, and enqueue consolidation.
//! - Phase 2: claim a global consolidation lock, materialize consolidation inputs, and dispatch one consolidation agent.

mod prompts;
mod stage_one;
mod startup;
mod storage;
mod text;

#[cfg(test)]
mod tests;

use serde::Deserialize;
use std::path::Path;
use std::path::PathBuf;

/// Subagent source label used to identify consolidation tasks.
const MEMORY_CONSOLIDATION_SUBAGENT_LABEL: &str = "memory_consolidation";
const ROLLOUT_SUMMARIES_SUBDIR: &str = "rollout_summaries";
const RAW_MEMORIES_FILENAME: &str = "raw_memories.md";
/// Maximum number of rollout candidates processed per startup pass.
const MAX_ROLLOUTS_PER_STARTUP: usize = 64;
/// Concurrency cap for startup memory extraction and consolidation scheduling.
const PHASE_ONE_CONCURRENCY_LIMIT: usize = MAX_ROLLOUTS_PER_STARTUP;
/// Maximum number of recent raw memories retained for global consolidation.
const MAX_RAW_MEMORIES_FOR_GLOBAL: usize = 1_024;
/// Maximum rollout age considered for phase-1 extraction.
const PHASE_ONE_MAX_ROLLOUT_AGE_DAYS: i64 = 30;
/// Minimum rollout idle time required before phase-1 extraction.
const PHASE_ONE_MIN_ROLLOUT_IDLE_HOURS: i64 = 12;
/// Lease duration (seconds) for phase-1 job ownership.
const PHASE_ONE_JOB_LEASE_SECONDS: i64 = 3_600;
/// Backoff delay (seconds) before retrying a failed stage-1 extraction job.
const PHASE_ONE_JOB_RETRY_DELAY_SECONDS: i64 = 3_600;
/// Lease duration (seconds) for phase-2 consolidation job ownership.
const PHASE_TWO_JOB_LEASE_SECONDS: i64 = 3_600;
/// Backoff delay (seconds) before retrying a failed phase-2 consolidation job.
const PHASE_TWO_JOB_RETRY_DELAY_SECONDS: i64 = 3_600;
/// Heartbeat interval (seconds) for phase-2 running jobs.
const PHASE_TWO_JOB_HEARTBEAT_SECONDS: u64 = 30;

/// Parsed stage-1 model output payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageOneOutput {
    /// Detailed markdown raw memory for a single rollout.
    #[serde(rename = "raw_memory")]
    raw_memory: String,
    /// Compact summary line used for routing and indexing.
    #[serde(rename = "rollout_summary")]
    rollout_summary: String,
    /// Optional slug accepted from stage-1 output for forward compatibility.
    ///
    /// This is currently ignored by downstream storage and naming, which remain
    /// thread-id based.
    #[serde(default, rename = "rollout_slug")]
    _rollout_slug: Option<String>,
}

pub fn memory_root(codex_home: &Path) -> PathBuf {
    codex_home.join("memories")
}

fn rollout_summaries_dir(root: &Path) -> PathBuf {
    root.join(ROLLOUT_SUMMARIES_SUBDIR)
}

fn raw_memories_file(root: &Path) -> PathBuf {
    root.join(RAW_MEMORIES_FILENAME)
}

async fn ensure_layout(root: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(rollout_summaries_dir(root)).await
}

pub(crate) use prompts::build_memory_tool_developer_instructions;
/// Starts the memory startup pipeline for eligible root sessions.
///
/// This is the single entrypoint that `codex` uses to trigger memory startup.
pub(crate) use startup::start_memories_startup_task;
