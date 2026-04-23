//! Compaction observation event definitions.

use crate::Observation;
use serde::Serialize;

/// What initiated the compaction work.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionTrigger {
    Manual,
    Auto,
}

/// Why the runtime chose to compact context.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionReason {
    UserRequested,
    ContextLimit,
    ModelDownshift,
}

/// Runtime implementation used for the compaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionImplementation {
    Responses,
    ResponsesCompact,
}

/// Point in the turn lifecycle where compaction ran.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionPhase {
    StandaloneTurn,
    PreTurn,
    MidTurn,
}

/// Strategy used to build the compacted context.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionStrategy {
    Memento,
    PrefixCompaction,
}

/// Terminal status of a compaction run.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionStatus<'a> {
    Completed,
    Failed {
        /// Bounded failure detail intended for remote export.
        error: Option<&'a str>,
    },
    Interrupted,
}

/// Observation emitted when a compaction run reaches a terminal state.
#[derive(Observation)]
#[observation(name = "compaction.ended", crate = "crate", uses = ["analytics"])]
pub struct CompactionEnded<'a> {
    #[obs(level = "basic", class = "identifier")]
    pub thread_id: &'a str,

    #[obs(level = "basic", class = "identifier")]
    pub turn_id: &'a str,

    #[obs(level = "basic", class = "operational")]
    pub trigger: CompactionTrigger,

    #[obs(level = "basic", class = "operational")]
    pub reason: CompactionReason,

    #[obs(level = "basic", class = "operational")]
    pub implementation: CompactionImplementation,

    #[obs(level = "basic", class = "operational")]
    pub phase: CompactionPhase,

    #[obs(level = "basic", class = "operational")]
    pub strategy: CompactionStrategy,

    #[obs(level = "basic", class = "operational")]
    pub status: CompactionStatus<'a>,

    #[obs(level = "basic", class = "operational")]
    pub active_context_tokens_before: i64,

    #[obs(level = "basic", class = "operational")]
    pub active_context_tokens_after: i64,

    /// Unix timestamp in seconds when compaction work began.
    #[obs(level = "basic", class = "operational")]
    pub started_at: i64,

    /// Unix timestamp in seconds when compaction reached its terminal state.
    #[obs(level = "basic", class = "operational")]
    pub ended_at: i64,

    /// Absent when the callsite cannot measure elapsed wall time.
    #[obs(level = "basic", class = "operational")]
    pub duration_ms: Option<i64>,
}
