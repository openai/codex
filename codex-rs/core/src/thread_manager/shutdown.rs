//! Unloads locally controlled idle MAv2 children and exposes tracked agent-tree teardown.

use super::ThreadManager;
use super::ThreadManagerState;
use crate::CodexThread;
use crate::agent::control::AgentTreeShutdownState;
use crate::state::TaskKind;
use codex_history::ResponseItemEnvelope;
use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_thread_store::LiveThread;
use codex_thread_store::ResumeThreadParams;
use codex_thread_store::ThreadStoreError;
use std::collections::HashMap;
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
        ThreadStoreError::Retryable { .. } => "thread_store_retryable",
        ThreadStoreError::OutcomeUnknown { .. } => "thread_store_outcome_unknown",
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

/// Work to resume or start after a successful tree suspension.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SuspendedAgentTree {
    /// Original ID of the root turn that was running when suspended.
    pub root_turn_id: Option<String>,
    /// Last real root turn when a saved result should wake an idle root. The
    /// replacement starts a fresh turn only if this predecessor is still current.
    pub wake_root_after: Option<String>,
    pub child_turns: HashMap<ThreadId, String>,
}

/// Holds the restored child resident until the caller admits its turn.
pub struct LoadedSuspendedAgent {
    pub thread: Arc<CodexThread>,
    pub(crate) _residency: Option<tokio::sync::OwnedRwLockReadGuard<()>>,
}

impl ThreadManager {
    /// Stops this locally controlled tree and reports the original IDs of running regular
    /// turns on the root and durable named V2 children, including turns without a recorded
    /// model step. Returns None before installing the fence if the root lacks a durable
    /// writer or has an unsupported active task.
    /// Idle members and unsupported descendant tasks shut down normally. On success,
    /// finished children have delivered their results to their parents' saved histories,
    /// and every source writer has closed. Repeated calls on the same loaded root
    /// return the same unfinished turns, even after caller cancellation.
    pub async fn suspend_agent_tree(
        &self,
        root: &Arc<CodexThread>,
    ) -> CodexResult<Option<SuspendedAgentTree>> {
        self.check_local_agent_root(root).await?;
        if root.session.live_thread().is_none() {
            return Ok(None);
        }
        let id = root.startup_metadata().thread_id;
        let shutdown = {
            let active = root.session.active_turn.lock().await;
            if active
                .as_ref()
                .and_then(|turn| turn.task.as_ref())
                .is_some_and(|task| task.kind != TaskKind::Regular)
            {
                return Ok(None);
            }
            root.session
                .services
                .local_agent_runtime
                .request_suspension(id)?
        };
        shutdown.suspended_tree().await.map(Some)
    }

    /// Loads the named V2 child and its recorded ancestors under the replacement root.
    /// This restores the original local identities without starting any model turns.
    pub async fn load_suspended_agent(
        &self,
        root: &Arc<CodexThread>,
        child_id: ThreadId,
    ) -> CodexResult<LoadedSuspendedAgent> {
        self.check_local_agent_root(root).await?;
        let runtime = &root.session.services.local_agent_runtime;
        let _membership = runtime.admit_start()?;
        runtime
            .control(root.session.session_id())
            .load_suspended_child(root, child_id)
            .await
    }

    async fn check_local_agent_root(&self, root: &Arc<CodexThread>) -> CodexResult<()> {
        if self.state.agent_control_factory.is_some()
            || root.session_source().is_non_root_agent()
            || !Arc::ptr_eq(
                root,
                &self.get_thread(root.startup_metadata().thread_id).await?,
            )
        {
            return Err(CodexErr::InvalidRequest(
                "agent tree recovery requires its loaded locally controlled root".into(),
            ));
        }
        Ok(())
    }
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

impl ThreadManagerState {
    /// An evicted parent has no session to consume queue-only mail. After the live
    /// writers stop, reopen its writer long enough to save its unread results.
    pub(crate) async fn save_evicted_parent_results(
        &self,
        parent: ThreadId,
        config: &crate::config::Config,
        history_mode: codex_protocol::protocol::ThreadHistoryMode,
        results: &[crate::PendingAgentMessage],
    ) -> codex_thread_store::ThreadStoreResult<()> {
        let params = ResumeThreadParams {
            thread_id: parent,
            rollout_path: None,
            history: None,
            history_revision: None,
            include_archived: false,
            metadata: config.thread_persistence_metadata(),
        };
        let (writer, _) = LiveThread::resume(
            Arc::clone(&self.thread_store),
            history_mode,
            params,
            /*resume_preparation*/ None,
        )
        .await?;
        let items = results
            .iter()
            .flat_map(|message| {
                [
                    RolloutItem::InterAgentCommunicationMetadata {
                        trigger_turn: message.trigger_turn,
                    },
                    RolloutItem::ResponseItem(ResponseItemEnvelope::new(
                        crate::session::session::Session::inter_agent_history_item(message, config),
                    )),
                ]
            })
            .collect::<Vec<_>>();
        let saved = writer.append_items(&items).await;
        // Shutdown flushes pending items and always releases the acquired writer.
        saved.and(writer.shutdown().await)?;
        for result in results {
            if let Some(write) = result.consumed(parent) {
                write.confirm();
            }
        }
        Ok(())
    }
}
