//! Hands observed inference spending to one host-owned accounting authority.

use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use codex_protocol::protocol::TokenUsage;
use futures::future::BoxFuture;

/// Spending reported for one model response, including compaction.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentUsageReport {
    pub thread_id: ThreadId,
    pub turn_id: String,
    /// Per-response usage, not cumulative totals. Remote adapters must explicitly
    /// carry `codex_rollout_budget_units`, which `TokenUsage` skips when serialized.
    pub usage: Option<TokenUsage>,
    /// Synchronous post-hook contributions, independent of the provider's usage.
    pub additional_budget_units: f64,
}

/// Each decision means the host recorded the reported spending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageAccountingDecision {
    Continue,
    /// Do not start more tool handlers in this local rollout tree. Already-running work continues.
    Exhausted,
    /// Fail the reporting response with `SessionBudgetExceeded` after recording its output and usage.
    /// Background compaction fails only its job; later turns and peer threads remain eligible.
    ExhaustedForTurn,
}

/// Replaces this runtime's `AgentControl::record_usage` and native rollout-budget reminders.
///
/// Install an `Arc<dyn AgentUsageAccounting>` in thread extension data before host-owned startup;
/// supply reminder context through context contributors. Core awaits each report. The host
/// owns budget policy, timeouts, report-only retries, reporting IDs, deduplication and recovery.
/// A turn may emit multiple reports; thread/turn IDs give attribution, not unique charges.
/// `Err` may leave acceptance unknown and causes a nonretryable failure without inference
/// retry or Local fallback. Incomplete responses retain their original error; background
/// compaction errors fail that job. This callback neither reserves future spending nor
/// interrupts other agents in the tree.
pub trait AgentUsageAccounting: Send + Sync {
    fn report_usage(
        &self,
        report: AgentUsageReport,
    ) -> BoxFuture<'_, Result<UsageAccountingDecision>>;
}
