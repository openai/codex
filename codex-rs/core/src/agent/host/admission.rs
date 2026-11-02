//! Connects host-owned turn capacity to Core task lifetimes.

use crate::agent::types::AgentExecutionGuard;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use futures::future::BoxFuture;

/// Identifies the execution for which Core requests host capacity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTurnAdmissionRequest {
    pub thread_id: ThreadId,
    pub turn_id: String,
}

/// Acquires host-owned capacity before each configured runtime task executes, including roots.
///
/// Select host ownership and install this provider before thread startup finishes. Return
/// `AgentLimitReached` when capacity is exhausted rather than waiting for a slot.
/// Cancellation drops the acquisition future. Errors fail the attempted turn
/// without falling back to Local admission. The returned guard releases or arranges
/// release of capacity when Core ends the task.
/// A submitted turn can be denied after its input was accepted. Core records that
/// input and reports the error through the ordinary turn lifecycle.
/// Hosts own remote retries, abandoned acquisitions and worker-loss recovery.
pub trait AgentTurnAdmission: Send + Sync {
    fn admit_turn(
        &self,
        request: AgentTurnAdmissionRequest,
    ) -> BoxFuture<'_, Result<AgentExecutionGuard>>;
}
