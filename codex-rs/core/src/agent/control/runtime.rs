//! Shared state and startup bindings for one local agent tree.
//! Registry identity is allocation identity; cloning this handle preserves ownership checks.

use super::LocalAgentControl;
use super::execution::AgentExecutionLimiter;
use super::residency::V2Residency;
use crate::agent::api::AgentControl;
use crate::agent::registry::AgentRegistry;
use crate::config::RolloutBudgetConfig;
use crate::rollout_budget::RolloutBudget;
use crate::thread_manager::AgentTreeShutdownFailure;
use crate::thread_manager::AgentTreeShutdownFailureReason;
use crate::thread_manager::AgentTreeShutdownReport;
use crate::thread_manager::SuspendedAgentTree;
use crate::thread_manager::ThreadIdGenerator;
use crate::thread_manager::ThreadManagerState;
use arc_swap::ArcSwapOption;
use codex_extension_api::ThreadInstructionsProvider;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::AgentErrorContext;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_rollout_trace::ThreadTraceContext;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;
use std::sync::Weak;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tokio_util::task::task_tracker::TaskTrackerToken;
use uuid::Uuid;

// Avoid retaining an unbounded number of failures during a long-lived tree's lifetime.
const MAX_RETAINED_SHUTDOWN_FAILURES: usize = 64;

#[derive(Debug)]
pub(crate) struct AgentTreeShutdownState {
    members: TaskTracker,
    report: Mutex<AgentTreeShutdownReport>,
    disposition: OnceLock<TreeDisposition>,
    suspended: Mutex<SuspendedAgentTree>,
    children: Mutex<HashMap<ThreadId, TaskTracker>>,
}

#[derive(Debug)]
enum TreeDisposition {
    Shutdown,
    Suspend {
        root: ThreadId,
        evicted_writers: TaskTracker,
    },
}

impl Default for AgentTreeShutdownState {
    fn default() -> Self {
        Self {
            members: TaskTracker::new(),
            disposition: OnceLock::new(),
            suspended: Mutex::default(),
            children: Mutex::default(),
            report: Mutex::new(AgentTreeShutdownReport {
                tree_id: Uuid::now_v7(),
                failures: Vec::new(),
                omitted_failures: 0,
            }),
        }
    }
}

impl AgentTreeShutdownState {
    pub(crate) async fn wait(&self) -> CodexResult<()> {
        self.wait_detailed()
            .await
            .map_err(|_| CodexErr::Fatal("agent tree shutdown did not complete cleanly".to_owned()))
    }

    pub(crate) async fn wait_detailed(&self) -> Result<(), AgentTreeShutdownReport> {
        self.members.wait().await;
        // The source may need to reopen an evicted parent's writer after all live
        // sessions stop. Both shutdown waiters and suspension callers join that write.
        if let Some(TreeDisposition::Suspend {
            evicted_writers, ..
        }) = self.disposition.get()
        {
            evicted_writers.wait().await;
        }
        let report = self
            .report
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if report.failures.is_empty() && report.omitted_failures == 0 {
            Ok(())
        } else {
            Err(report)
        }
    }

    pub(crate) async fn suspended_tree(&self) -> CodexResult<SuspendedAgentTree> {
        self.wait().await?;
        Ok(self
            .suspended
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    fn record_failure(&self, failure: AgentTreeShutdownFailure) {
        let tree_id = {
            let mut report = self.report.lock().unwrap_or_else(PoisonError::into_inner);
            if report.failures.len() < MAX_RETAINED_SHUTDOWN_FAILURES {
                report.failures.push(failure.clone());
            } else {
                report.omitted_failures = report.omitted_failures.saturating_add(/*rhs*/ 1);
            }
            report.tree_id
        };
        let (reason, phase, error_kind) = match &failure.reason {
            AgentTreeShutdownFailureReason::OperationFailed { phase, error_kind } => {
                ("operation_failed", Some(*phase), Some(*error_kind))
            }
            AgentTreeShutdownFailureReason::GuardAbandoned => ("guard_abandoned", None, None),
        };
        let thread_id = failure.thread_id.map(tracing::field::display);
        tracing::warn!(
            %tree_id,
            operation = failure.operation,
            phase,
            thread_id,
            reason,
            error_kind,
            "agent tree shutdown failure recorded"
        );
    }
}

#[derive(Clone)]
pub(crate) struct AgentTreeMembership {
    state: Arc<AgentTreeShutdownState>,
    _member: TaskTrackerToken,
    _child: Option<TaskTrackerToken>,
}

impl AgentTreeMembership {
    pub(crate) fn into_teardown_guard(
        self,
        operation: &'static str,
        thread_id: Option<ThreadId>,
    ) -> AgentTreeTeardownGuard {
        AgentTreeTeardownGuard {
            membership: self,
            operation,
            thread_id,
            completed: false,
        }
    }
}

/// Marks tree shutdown as failed if teardown work exits without completing.
pub(crate) struct AgentTreeTeardownGuard {
    membership: AgentTreeMembership,
    operation: &'static str,
    thread_id: Option<ThreadId>,
    completed: bool,
}

impl AgentTreeTeardownGuard {
    pub(crate) fn clone_for_teardown(
        &self,
        operation: &'static str,
        thread_id: Option<ThreadId>,
    ) -> Self {
        self.membership
            .clone()
            .into_teardown_guard(operation, thread_id)
    }

    pub(crate) fn set_thread_id(&mut self, thread_id: ThreadId) {
        self.thread_id = Some(thread_id);
    }

    pub(crate) fn record_shutdown_failure(&self, phase: &'static str, error_kind: &'static str) {
        self.membership
            .state
            .record_failure(AgentTreeShutdownFailure::operation_failed(
                self.operation,
                phase,
                self.thread_id,
                error_kind,
            ));
    }

    pub(crate) fn complete(mut self) {
        self.completed = true;
    }
}

impl Drop for AgentTreeTeardownGuard {
    fn drop(&mut self) {
        if !self.completed {
            // Record before membership is dropped, so a waiter cannot snapshot first.
            self.membership
                .state
                .record_failure(AgentTreeShutdownFailure {
                    operation: self.operation,
                    thread_id: self.thread_id,
                    reason: AgentTreeShutdownFailureReason::GuardAbandoned,
                });
        }
    }
}

/// Local tree state, kept separate from the shared agent operation interface.
#[derive(Clone)]
pub(crate) struct LocalAgentRuntime {
    /// Weak handle back to the global thread registry/state.
    /// This is `Weak` to avoid reference cycles and shadow persistence of the form
    /// `ThreadManagerState -> CodexThread -> Session -> SessionServices -> ThreadManagerState`.
    pub(super) manager: Weak<ThreadManagerState>,
    /// Captured at construction so delegates retain their manager's allocation policy.
    pub(super) thread_id_generator: ThreadIdGenerator,
    pub(super) agent_execution_limiter: Arc<AgentExecutionLimiter>,
    /// Session-scoped state shared by the root thread and every cloned sub-agent control handle.
    pub(super) rollout_budget: Arc<RolloutBudget>,
    /// The user-selected root routing tier, shared by the entire agent tree.
    pub(super) root_service_tier: Arc<ArcSwapOption<String>>,
    /// Retains the root's opt-in instruction provider even when the root is unloaded.
    pub(super) shared_thread_instructions_provider:
        Arc<OnceLock<Arc<dyn ThreadInstructionsProvider>>>,
    pub(super) registry: Arc<AgentRegistry>,
    pub(super) residency: Arc<V2Residency>,
    pub(super) unconfirmed_results: Arc<AtomicUsize>,
    pub(super) mailboxes: Arc<super::mailbox::Mailboxes>,
    /// Shared by every session in this tree, including private delegates.
    pub(crate) shutdown: CancellationToken,
    shutdown_state: Arc<AgentTreeShutdownState>,
}

impl LocalAgentRuntime {
    pub(super) fn new(
        manager: Weak<ThreadManagerState>,
        thread_id_generator: ThreadIdGenerator,
        rollout_budget: Option<RolloutBudgetConfig>,
    ) -> Self {
        let runtime = Self {
            manager,
            thread_id_generator,
            registry: Arc::default(),
            residency: Arc::default(),
            unconfirmed_results: Arc::default(),
            mailboxes: Arc::default(),
            shutdown: CancellationToken::new(),
            shutdown_state: Arc::default(),
            agent_execution_limiter: Arc::default(),
            rollout_budget: Arc::default(),
            root_service_tier: Arc::new(ArcSwapOption::from(None)),
            shared_thread_instructions_provider: Arc::default(),
        };
        if let Some(rollout_budget) = rollout_budget {
            runtime.rollout_budget.configure(rollout_budget);
        }
        runtime
    }

    pub(crate) async fn wait_for_children(&self, parent: ThreadId) {
        let children = self
            .shutdown_state
            .children
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&parent)
            .cloned();
        if let Some(children) = children {
            children.close();
            children.wait().await;
        }
    }

    pub(crate) fn admit_session(
        &self,
        parent: Option<ThreadId>,
    ) -> CodexResult<AgentTreeMembership> {
        let mut children = self
            .shutdown_state
            .children
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut membership = self.admit_start()?;
        children.retain(|_, tracker| !tracker.is_empty());
        membership._child = parent.map(|id| children.entry(id).or_default().token());
        Ok(membership)
    }

    pub(crate) fn tree_id(&self) -> Uuid {
        self.shutdown_state
            .report
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tree_id
    }

    /// Bind local startup to the same tree state with this session's identity.
    pub(crate) fn control(&self, session_id: SessionId) -> LocalAgentControl {
        LocalAgentControl {
            session_id,
            runtime: self.clone(),
        }
    }
}

/// Local construction binds identity after reading history. Hosts and internal children
/// provide an already-bound controller without selecting a backend again.
#[derive(Clone)]
pub(crate) enum AgentControlInit {
    Local(LocalAgentControl),
    Provided {
        control: Arc<dyn AgentControl>,
        runtime: LocalAgentRuntime,
    },
}

impl From<LocalAgentControl> for AgentControlInit {
    fn from(control: LocalAgentControl) -> Self {
        Self::Local(control)
    }
}

impl AgentControlInit {
    pub(crate) fn runtime(&self) -> &LocalAgentRuntime {
        match self {
            Self::Local(control) => &control.runtime,
            Self::Provided { runtime, .. } => runtime,
        }
    }

    pub(crate) fn control(&self) -> &dyn AgentControl {
        match self {
            Self::Local(control) => control,
            Self::Provided { control, .. } => control.as_ref(),
        }
    }
}

impl LocalAgentRuntime {
    pub(crate) fn admit_start(&self) -> CodexResult<AgentTreeMembership> {
        if self.shutdown_state.members.is_closed() {
            return Err(
                CodexErr::InvalidRequest("agent runtime is shutting down".to_owned())
                    .with_agent_context(AgentErrorContext::RuntimeShutdown),
            );
        }
        let membership = AgentTreeMembership {
            state: Arc::clone(&self.shutdown_state),
            _member: self.shutdown_state.members.token(),
            _child: None,
        };
        // Closing a TaskTracker does not reject new tokens. Recheck so a start racing with
        // shutdown is either admitted before the fence or rejected after it.
        if self.shutdown_state.members.is_closed() {
            return Err(
                CodexErr::InvalidRequest("agent runtime is shutting down".to_owned())
                    .with_agent_context(AgentErrorContext::RuntimeShutdown),
            );
        }
        Ok(membership)
    }

    pub(crate) fn request_shutdown(&self) -> Arc<AgentTreeShutdownState> {
        let disposition = self
            .shutdown_state
            .disposition
            .get_or_init(|| TreeDisposition::Shutdown);
        self.shutdown_state.members.close();
        self.shutdown.cancel();
        if matches!(disposition, TreeDisposition::Shutdown) {
            self.mailboxes.close();
        }
        Arc::clone(&self.shutdown_state)
    }

    pub(crate) fn request_suspension(
        &self,
        root: ThreadId,
    ) -> CodexResult<Arc<AgentTreeShutdownState>> {
        let disposition = self.shutdown_state.disposition.get_or_init(|| {
            let evicted_writers = TaskTracker::new();
            let guard = AgentTreeMembership {
                state: Arc::clone(&self.shutdown_state),
                _member: evicted_writers.token(),
                _child: None,
            }
            .into_teardown_guard("save_evicted_parent_results", Some(root));
            evicted_writers.close();
            let runtime = self.clone();
            tokio::spawn(async move {
                if let Err(failure) = runtime.save_results_after_eviction().await {
                    runtime.record_shutdown_failure(failure);
                }
                runtime.mailboxes.close();
                guard.complete();
            });
            TreeDisposition::Suspend {
                root,
                evicted_writers,
            }
        });
        if !matches!(disposition, TreeDisposition::Suspend { root: owner, .. } if *owner == root) {
            return Err(CodexErr::InvalidRequest(
                "agent tree shutdown already has a different disposition".into(),
            ));
        }
        Ok(self.request_shutdown())
    }

    async fn save_results_after_eviction(&self) -> Result<(), AgentTreeShutdownFailure> {
        self.shutdown_state.members.wait().await;
        if self.unconfirmed_results.load(Ordering::Acquire) == 0 {
            return Ok(());
        }
        // A failed teardown may still own a writer. Never reopen it.
        if !self
            .shutdown_state
            .report
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .failures
            .is_empty()
        {
            return Ok(());
        }
        for parent in self
            .registry
            .live_agents()
            .into_iter()
            .filter_map(|agent| agent.agent_id)
        {
            let results = self
                .mailboxes
                .take(parent, crate::PendingAgentMessage::is_terminal_result);
            if results.is_empty() {
                continue;
            }
            let failed = |phase, kind| {
                AgentTreeShutdownFailure::operation_failed(
                    "save_evicted_parent_results",
                    phase,
                    Some(parent),
                    kind,
                )
            };
            let owner = self
                .registry
                .evicted_environments(parent)
                .ok_or_else(|| failed("locate_parent", "parent_not_evicted"))?;
            self.upgrade()
                .map_err(|_| failed("open_writer", "manager_unavailable"))?
                .save_evicted_parent_results(parent, &owner.config, owner.history_mode, &results)
                .await
                .map_err(|err| {
                    failed(
                        "persist",
                        crate::thread_manager::thread_store_error_kind(&err),
                    )
                })?;
        }
        // A task that claimed a result and stopped before its append returned leaves
        // an unconfirmed write. Only its writer knows whether the append took effect.
        if self.unconfirmed_results.load(Ordering::Acquire) != 0 {
            return Err(AgentTreeShutdownFailure::operation_failed(
                "tree_suspension",
                "persist_child_result",
                /*thread_id*/ None,
                "unconfirmed_write",
            ));
        }
        Ok(())
    }

    pub(crate) fn suspension_root(&self) -> Option<ThreadId> {
        match self.shutdown_state.disposition.get() {
            Some(TreeDisposition::Suspend { root, .. }) => Some(*root),
            _ => None,
        }
    }

    pub(crate) fn record_root_wake_after(&self, previous_turn_id: String) {
        self.shutdown_state
            .suspended
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .wake_root_after = Some(previous_turn_id);
    }

    pub(crate) fn record_suspended_turn(&self, thread_id: ThreadId, turn_id: String) {
        let mut tree = self
            .shutdown_state
            .suspended
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if self.suspension_root() == Some(thread_id) {
            tree.root_turn_id = Some(turn_id);
            tree.wake_root_after = None;
        } else {
            tree.child_turns.insert(thread_id, turn_id);
        }
    }

    /// The caller must hold tree membership until this record has been written. This keeps
    /// the completed shutdown report stable for every waiter.
    pub(crate) fn record_shutdown_failure(&self, failure: AgentTreeShutdownFailure) {
        self.shutdown_state.record_failure(failure);
    }

    pub(crate) fn generate_thread_id(&self) -> ThreadId {
        (self.thread_id_generator)()
    }

    /// Shares the tree's trace with a new or reloaded child runtime.
    pub(crate) async fn parent_rollout_thread_trace(
        &self,
        session_source: &SessionSource,
    ) -> ThreadTraceContext {
        let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        }) = session_source
        else {
            return ThreadTraceContext::disabled();
        };
        let Some(manager) = self.manager.upgrade() else {
            return ThreadTraceContext::disabled();
        };
        // Reattach reloaded children through the v2 root: their immediate parent
        // may also be evicted. V1 has no registered root, so it uses the parent.
        let trace_owner = self
            .registry
            .agent_id_for_path(&AgentPath::root())
            .unwrap_or(*parent_thread_id);
        manager
            .get_thread(trace_owner)
            .await
            .ok()
            .filter(|thread| thread.is_running())
            .map(|thread| thread.session.services.rollout_thread_trace.clone())
            .unwrap_or_else(ThreadTraceContext::disabled)
    }

    pub(crate) fn root_thread_instructions_provider(
        &self,
        root_thread_id: ThreadId,
        provider: Option<Arc<dyn ThreadInstructionsProvider>>,
    ) -> Option<Arc<dyn ThreadInstructionsProvider>> {
        let provider = match self.manager.upgrade() {
            Some(manager) => manager.shared_thread_instructions_provider(root_thread_id, provider),
            None => provider,
        };
        if let Some(provider) = provider
            .as_ref()
            .filter(|provider| provider.share_with_subagents())
        {
            let _ = self
                .shared_thread_instructions_provider
                .set(Arc::clone(provider));
        }
        provider
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
