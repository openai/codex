use std::sync::Arc;

use crate::agents_md::LoadedAgentsMd;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::McpRuntimeSnapshot;
use crate::session::turn_context::TurnContext;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::ToolInfo;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::TurnContextItem;
use tokio::sync::OnceCell;

/// Request-scoped state that may change between model sampling requests.
#[derive(Debug)]
pub(crate) struct StepContext {
    pub(crate) turn: Arc<TurnContext>,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
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
        reasoning_effort: Option<ReasoningEffortConfig>,
        environments: TurnEnvironmentSnapshot,
        selected_capability_roots: Vec<ResolvedSelectedCapabilityRoot>,
        mcp: Arc<McpRuntimeSnapshot>,
        loaded_agents_md: Option<Arc<LoadedAgentsMd>>,
    ) -> Self {
        Self {
            turn,
            reasoning_effort,
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

    pub(crate) fn effective_reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        if self.turn.model_info.supports_reasoning_summaries {
            self.reasoning_effort
                .clone()
                .or_else(|| self.turn.model_info.default_reasoning_level.clone())
        } else {
            None
        }
    }

    pub(crate) fn effective_reasoning_effort_for_tracing(&self) -> String {
        self.effective_reasoning_effort()
            .map(|effort| effort.to_string())
            .unwrap_or_else(|| "default".to_string())
    }

    pub(crate) fn collaboration_mode(&self) -> CollaborationMode {
        self.turn.collaboration_mode.with_updates(
            /*model*/ None,
            Some(self.reasoning_effort.clone()),
            /*developer_instructions*/ None,
        )
    }

    pub(crate) fn to_turn_context_item(&self) -> TurnContextItem {
        let workspace_roots = self.turn.config.effective_workspace_roots();
        #[allow(deprecated)]
        let cwd = self.turn.cwd.clone();
        TurnContextItem {
            turn_id: Some(self.turn.sub_id.clone()),
            cwd,
            workspace_roots: (!workspace_roots.is_empty()).then_some(workspace_roots),
            current_date: self.turn.current_date.clone(),
            timezone: self.turn.timezone.clone(),
            approval_policy: self.turn.approval_policy.value(),
            sandbox_policy: self.turn.sandbox_policy(),
            permission_profile: Some(self.turn.permission_profile()),
            network: self.turn.turn_context_network_item(),
            file_system_sandbox_policy: self.turn.non_legacy_file_system_sandbox_policy(),
            model: self.turn.model_info.slug.clone(),
            comp_hash: self.turn.model_info.comp_hash.clone(),
            personality: self.turn.personality,
            collaboration_mode: Some(self.collaboration_mode()),
            multi_agent_version: Some(self.turn.multi_agent_version),
            multi_agent_mode: super::multi_agents::effective_multi_agent_mode(self),
            realtime_active: Some(self.turn.realtime_active),
            effort: self.reasoning_effort.clone(),
            summary: ReasoningSummaryConfig::Auto,
        }
    }
}

/// Frozen step inputs needed by downstream work that can outlive a sampling request.
#[derive(Clone, Debug)]
pub(crate) struct StepContextSeed {
    pub(crate) turn: Arc<TurnContext>,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
}

impl StepContextSeed {
    pub(crate) fn from_turn(turn: Arc<TurnContext>) -> Self {
        let reasoning_effort = turn.config.model_reasoning_effort.clone();
        Self {
            turn,
            reasoning_effort,
        }
    }
}

impl From<&StepContext> for StepContextSeed {
    fn from(step_context: &StepContext) -> Self {
        Self {
            turn: Arc::clone(&step_context.turn),
            reasoning_effort: step_context.reasoning_effort.clone(),
        }
    }
}
