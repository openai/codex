//! Memory subsystem for startup extraction and consolidation.
//!
//! The startup memory pipeline is split into two phases:
//! - Phase 1: select rollouts, extract stage-1 raw memories, persist stage-1 outputs, and enqueue consolidation.
//! - Phase 2: claim scopes, materialize consolidation inputs, and dispatch consolidation agents.

mod layout;
mod prompts;
mod rollout;
mod scope;
mod stage_one;
mod startup;
mod storage;
mod text;
mod types;

#[cfg(test)]
mod tests;

/// Subagent source label used to identify consolidation tasks.
const MEMORY_CONSOLIDATION_SUBAGENT_LABEL: &str = "memory_consolidation";
/// Maximum number of rollout candidates processed per startup pass.
const MAX_ROLLOUTS_PER_STARTUP: usize = 64;
/// Concurrency cap for startup memory extraction and consolidation scheduling.
const PHASE_ONE_CONCURRENCY_LIMIT: usize = MAX_ROLLOUTS_PER_STARTUP;
/// Concurrency cap for phase-2 consolidation dispatch.
const PHASE_TWO_CONCURRENCY_LIMIT: usize = MAX_ROLLOUTS_PER_STARTUP;
/// Maximum number of recent raw memories retained per scope.
const MAX_RAW_MEMORIES_PER_SCOPE: usize = 64;
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

/// Starts the memory startup pipeline for eligible root sessions.
///
/// This is the single entrypoint that `codex` uses to trigger memory startup.
pub(crate) use startup::start_memories_startup_task;
