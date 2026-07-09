use std::sync::Arc;

use crate::agents_md::LoadedAgentsMd;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::McpRuntimeSnapshot;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_features::Feature;
use codex_mcp::ToolInfo;
use tokio::sync::OnceCell;

/// Request-scoped state that may change between model sampling requests.
#[derive(Debug)]
pub(crate) struct StepContext {
    pub(crate) turn: Arc<TurnContext>,
    pub(crate) environments: TurnEnvironmentSnapshot,
    /// Capability roots bound to ready environments in this exact step.
    pub(crate) selected_capability_roots: Vec<ResolvedSelectedCapabilityRoot>,
    /// The exact MCP config and manager used to advertise and execute tools for this step.
    pub(crate) mcp: Arc<McpRuntimeSnapshot>,
    /// The fixed MCP tool list used for this exact sampling request.
    mcp_tool_snapshot: OnceCell<Vec<ToolInfo>>,
    /// The canonical AGENTS.md value observed with this environment snapshot.
    pub(crate) loaded_agents_md: Option<Arc<LoadedAgentsMd>>,
}

impl StepContext {
    pub(crate) fn new(
        turn: Arc<TurnContext>,
        environments: TurnEnvironmentSnapshot,
        selected_capability_roots: Vec<ResolvedSelectedCapabilityRoot>,
        mcp: Arc<McpRuntimeSnapshot>,
        loaded_agents_md: Option<Arc<LoadedAgentsMd>>,
    ) -> Self {
        Self {
            turn,
            environments,
            selected_capability_roots,
            mcp,
            mcp_tool_snapshot: OnceCell::new(),
            loaded_agents_md,
        }
    }

    pub(crate) async fn mcp_tools(&self) -> &[ToolInfo] {
        self.mcp_tool_snapshot
            .get_or_init(|| self.mcp.manager().list_all_tools())
            .await
    }

    #[tracing::instrument(name = "step_context.capture", level = "info", skip_all)]
    pub(crate) async fn refresh_env(self: Arc<Self>, session: &Arc<Session>) -> Arc<Self> {
        let deferred_executor_enabled =
            self.turn.config.features.enabled(Feature::DeferredExecutor);
        // Keep the old turn-frozen environment view unless deferred executors are enabled.
        let environments = if deferred_executor_enabled {
            session.services.turn_environments.snapshot().await
        } else {
            self.turn.environments.clone()
        };
        if deferred_executor_enabled {
            session
                .services
                .agents_md_manager
                .refresh(&self.turn.config, &environments)
                .await;
        }
        let loaded_agents_md = session.services.agents_md_manager.get_loaded().await;
        let selected_capability_roots = session
            .resolve_selected_capability_roots_for_step(&environments)
            .await;
        let mcp = session
            .mcp_runtime_for_step(
                self.turn.as_ref(),
                &environments,
                &selected_capability_roots,
            )
            .await;
        let step_context = Arc::new(Self::new(
            Arc::clone(&self.turn),
            environments,
            selected_capability_roots,
            mcp,
            loaded_agents_md,
        ));

        let mut active = session.active_turn.lock().await;
        if let Some(task) = active.as_mut().and_then(|turn| turn.task.as_mut())
            && task.step_context.turn.sub_id == step_context.turn.sub_id
        {
            task.step_context = Arc::clone(&step_context);
        }

        step_context
    }
}
