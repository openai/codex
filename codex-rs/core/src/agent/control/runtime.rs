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
use crate::thread_manager::ThreadIdGenerator;
use crate::thread_manager::ThreadManagerState;
use arc_swap::ArcSwapOption;
use codex_extension_api::ThreadInstructionsProvider;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::AgentErrorContext;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;
use std::sync::Weak;
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
}

impl Default for AgentTreeShutdownState {
    fn default() -> Self {
        Self {
            members: TaskTracker::new(),
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
        // Reporters hold a membership until after recording failures. Once the tracker drains,
        // no admitted operation can add a failure after the snapshot.
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
        self.shutdown_state.members.close();
        self.shutdown.cancel();
        self.mailboxes.close();
        Arc::clone(&self.shutdown_state)
    }

    /// The caller must hold tree membership until this record has been written. This keeps
    /// the completed shutdown report stable for every waiter.
    pub(crate) fn record_shutdown_failure(&self, failure: AgentTreeShutdownFailure) {
        self.shutdown_state.record_failure(failure);
    }

    pub(crate) fn generate_thread_id(&self) -> ThreadId {
        (self.thread_id_generator)()
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
