//! Unloads locally controlled idle MAv2 children and exposes tracked agent-tree teardown.

use super::ThreadManager;
use crate::agent::control::AgentTreeShutdownState;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_thread_store::ThreadStoreError;
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;

/// Return a stable, payload-free category for a thread-store error.
pub(crate) fn thread_store_error_kind(error: &ThreadStoreError) -> &'static str {
    match error {
        ThreadStoreError::ThreadNotFound { .. } => "thread_store_not_found",
        ThreadStoreError::InvalidRequest { .. } => "thread_store_invalid_request",
        ThreadStoreError::Conflict { .. } => "thread_store_conflict",
        ThreadStoreError::Unsupported { .. } => "thread_store_unsupported",
        ThreadStoreError::Internal { .. } => "thread_store_internal",
    }
}

/// A safely loggable reason that a tracked part of an agent tree failed to shut down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentTreeShutdownFailureReason {
    /// An operation returned an error at a specific step. The kind is a stable,
    /// non-sensitive classification.
    OperationFailed {
        phase: &'static str,
        error_kind: &'static str,
    },
    /// A tracked operation was dropped before it could report that its work completed.
    GuardAbandoned,
}

/// One failure observed during the lifetime or teardown of an agent tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTreeShutdownFailure {
    pub operation: &'static str,
    pub thread_id: Option<ThreadId>,
    pub reason: AgentTreeShutdownFailureReason,
}

impl AgentTreeShutdownFailure {
    /// Classifies an operation failure without retaining arbitrary error messages.
    pub fn operation_failed(
        operation: &'static str,
        phase: &'static str,
        thread_id: Option<ThreadId>,
        error_kind: &'static str,
    ) -> Self {
        Self {
            operation,
            thread_id,
            reason: AgentTreeShutdownFailureReason::OperationFailed { phase, error_kind },
        }
    }
}

/// The bounded set of failures observed for one local agent-tree instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTreeShutdownReport {
    /// Correlation ID for this runtime instance; this is not a thread's ID.
    pub tree_id: Uuid,
    /// Failures retained in recording order. Each failure is also logged when recorded.
    pub failures: Vec<AgentTreeShutdownFailure>,
    /// Number of additional failures omitted after the retention limit was reached.
    pub omitted_failures: usize,
}

impl fmt::Display for AgentTreeShutdownReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "agent tree shutdown did not complete cleanly: tree_id={}, failures=[",
            self.tree_id
        )?;
        for (index, failure) in self.failures.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{{operation={}", failure.operation)?;
            if let AgentTreeShutdownFailureReason::OperationFailed { phase, .. } = &failure.reason {
                write!(f, ", phase={phase}")?;
            }
            if let Some(thread_id) = failure.thread_id {
                write!(f, ", thread_id={thread_id}")?;
            }
            match &failure.reason {
                AgentTreeShutdownFailureReason::OperationFailed { error_kind, .. } => {
                    write!(f, ", reason=operation_failed, error_kind={error_kind}")?;
                }
                AgentTreeShutdownFailureReason::GuardAbandoned => {
                    write!(f, ", reason=guard_abandoned")?;
                }
            }
            write!(f, "}}")?;
        }
        write!(f, "]; omitted_failures={}", self.omitted_failures)
    }
}

impl std::error::Error for AgentTreeShutdownReport {}

/// Completion handle for one exact local agent tree.
#[derive(Clone, Debug)]
#[must_use = "agent-tree shutdown is not complete until wait() finishes"]
pub struct AgentTreeShutdown {
    state: Arc<AgentTreeShutdownState>,
}

impl AgentTreeShutdown {
    /// Waits for every operation and session admitted before the shutdown fence to finish.
    /// Returns an error if cleanup or persistence writer termination failed. Registry removal and
    /// caller-owned persistence handoff remain with the caller. Dropping this future does not
    /// cancel shutdown.
    pub async fn wait(&self) -> CodexResult<()> {
        self.state.wait().await
    }

    /// Waits for the tree and returns its recorded failure report if cleanup was unsuccessful.
    /// Each call observes the same completed report. Dropping the future does not cancel shutdown.
    pub async fn wait_detailed(&self) -> Result<(), AgentTreeShutdownReport> {
        self.state.wait_detailed().await
    }
}

impl ThreadManager {
    /// Unloads this exact idle MAv2 child using the manager's local controller backend,
    /// preserving mail and environments for reloading. Host controllers own their eviction
    /// and recovery; managers configured with a host controller factory are rejected.
    /// Dropping the future after shutdown starts does not cancel cleanup.
    pub async fn try_evict_v2_thread(
        &self,
        thread: Arc<crate::CodexThread>,
    ) -> CodexResult<crate::ThreadEvictionOutcome> {
        // Backend selection is fixed before the manager is shared or starts any threads.
        if self.state.agent_control_factory.is_some() {
            return Err(CodexErr::InvalidRequest(
                "idle eviction requires the local agent controller".to_owned(),
            ));
        }
        let Ok(membership) = thread.session.services.local_agent_runtime.admit_start() else {
            return Ok(crate::ThreadEvictionOutcome::Busy);
        };
        self.state.try_evict_v2_thread(thread, &membership).await
    }

    /// Requests shutdown of a loaded thread and every session sharing its local runtime.
    ///
    /// Returns after fencing new starts and signalling admitted sessions. Callers can wait on the
    /// returned handle before completing registry cleanup or a persistence handoff. This fences
    /// only starts sharing the current runtime; callers must separately serialize top-level loads
    /// or resumes of the same thread ID through that handoff.
    pub async fn request_agent_tree_shutdown(
        &self,
        thread_id: ThreadId,
    ) -> CodexResult<AgentTreeShutdown> {
        let thread = self.get_thread(thread_id).await?;
        Ok(AgentTreeShutdown {
            state: thread
                .session
                .services
                .local_agent_runtime
                .request_shutdown(),
        })
    }
}
