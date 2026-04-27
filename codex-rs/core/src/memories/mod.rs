//! Memory startup extraction and consolidation orchestration.
//!
//! The startup memory pipeline is split into two phases:
//! - Phase 1: select rollouts, extract stage-1 raw memories, persist stage-1 outputs, and enqueue consolidation.
//! - Phase 2: claim a global consolidation lock, materialize consolidation inputs, and dispatch one consolidation agent.

mod phase1;
mod phase2;
mod start;
#[cfg(test)]
mod tests;

use codex_protocol::openai_models::ReasoningEffort;

/// Starts the memory startup pipeline for eligible root sessions.
/// This is the single entrypoint that `codex` uses to trigger memory startup.
///
/// This is the entry point to read and understand this module.
pub(crate) use start::start_memories_startup_task;

/// Phase 1 (startup extraction).
mod phase_one {
    /// Default model used for phase 1.
    pub(super) const MODEL: &str = "gpt-5.4-mini";
    /// Default reasoning effort used for phase 1.
    pub(super) const REASONING_EFFORT: super::ReasoningEffort = super::ReasoningEffort::Low;
    /// Prompt used for phase 1.
    pub(super) const PROMPT: &str = codex_memories_write::STAGE_ONE_PROMPT;
    /// Concurrency cap for startup memory extraction and consolidation scheduling.
    pub(super) const CONCURRENCY_LIMIT: usize = 8;
    /// Lease duration (seconds) for phase-1 job ownership.
    pub(super) const JOB_LEASE_SECONDS: i64 = 3_600;
    /// Backoff delay (seconds) before retrying a failed stage-1 extraction job.
    pub(super) const JOB_RETRY_DELAY_SECONDS: i64 = 3_600;
    /// Maximum number of threads to scan.
    pub(super) const THREAD_SCAN_LIMIT: usize = 5_000;
    /// Size of the batches when pruning old thread memories.
    pub(super) const PRUNE_BATCH_SIZE: usize = 200;
}

/// Phase 2 (aka `Consolidation`).
mod phase_two {
    /// Default model used for phase 2.
    pub(super) const MODEL: &str = "gpt-5.4";
    /// Default reasoning effort used for phase 2.
    pub(super) const REASONING_EFFORT: super::ReasoningEffort = super::ReasoningEffort::Medium;
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
