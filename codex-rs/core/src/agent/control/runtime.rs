//! Shared state and startup bindings for one local agent tree.
//! Registry identity is allocation identity; cloning this handle preserves ownership checks.

use super::LocalAgentControl;
use super::execution::AgentExecutionLimiter;
use super::residency::V2Residency;
use crate::agent::api::AgentControl;
use crate::agent::registry::AgentRegistry;
use crate::config::RolloutBudgetConfig;
use crate::rollout_budget::RolloutBudget;
use crate::thread_manager::ThreadIdGenerator;
use crate::thread_manager::ThreadManagerState;
use arc_swap::ArcSwapOption;
use codex_extension_api::ThreadInstructionsProvider;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::Weak;

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
