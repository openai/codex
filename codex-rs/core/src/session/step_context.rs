//! Request-scoped settings and capabilities, including the durable context snapshot.

use std::sync::Arc;

use crate::agents_md::LoadedAgentsMd;
use crate::config::TokenBudgetConfig;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::step_settings::ResolvedStepSettings;
use crate::session::turn_context::TurnContext;
use crate::tools::router::ToolRouter;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::McpBinding;
use codex_otel::SessionTelemetry;
use codex_protocol::items::ModelInvocationContext;
use codex_protocol::protocol::TurnContextItem;

/// Request-scoped state that may change between model sampling requests.
pub(crate) struct StepContext {
    pub(crate) turn: Arc<TurnContext>,
    /// One immutable settings version captured before request preparation.
    pub(crate) settings: Arc<ResolvedStepSettings>,
    /// Frozen turn preferences resolved against this step's captured model.
    pub(crate) token_budget: Option<TokenBudgetConfig>,
    /// Telemetry context tagged with this sampling request's model.
    pub(crate) session_telemetry: SessionTelemetry,
    pub(crate) environments: TurnEnvironmentSnapshot,
    /// Capability roots bound to ready environments in this exact step.
    pub(crate) selected_capability_roots: Vec<ResolvedSelectedCapabilityRoot>,
    /// Executor-materialized capability files shared by MCP and skills in this exact step.
    pub(crate) executor_capability_discovery: Option<Arc<ExecutorCapabilityDiscoverySnapshot>>,
    /// The exact MCP connections, configuration, and catalog captured for this step.
    pub(crate) mcp: Arc<McpBinding>,
    /// The finalized tool plan advertised and executed for this exact sampling request.
    pub(crate) tool_router: Arc<ToolRouter>,
    /// The canonical AGENTS.md value observed with this environment snapshot.
    pub(crate) loaded_agents_md: Option<Arc<LoadedAgentsMd>>,
}

impl StepContext {
    /// Persist the summary captured for this request, even after a live settings update.
    pub(crate) fn to_turn_context_item(&self) -> TurnContextItem {
        let mut item = self.turn.to_turn_context_item();
        item.summary = self.settings.reasoning_summary;
        item
    }

    pub(crate) fn model_context(&self) -> ModelInvocationContext {
        ModelInvocationContext {
            model_slug: self.settings.model_info.slug.clone(),
            reasoning_effort: self
                .settings
                .effective_reasoning_effort()
                .map(|effort| effort.to_string()),
        }
    }
}
