use std::sync::Arc;

use crate::agents_md::LoadedAgentsMd;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::turn_context::TurnContext;
use crate::tools::router::ToolRouter;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::McpBinding;
use codex_otel::SessionTelemetry;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;

/// Request-scoped state that may change between model sampling requests.
pub(crate) struct StepContext {
    pub(crate) turn: Arc<TurnContext>,
    /// Concrete model and capabilities used by this sampling request.
    pub(crate) model_info: Arc<ModelInfo>,
    /// Reasoning effort selected for this sampling request.
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
    /// Reasoning summary resolved for this sampling request.
    pub(crate) reasoning_summary: ReasoningSummary,
    /// Effective service tier supported by this sampling request's model.
    pub(crate) service_tier: Option<String>,
    /// Effective approval policy used by this sampling request's tool actions.
    /// Model-specific Guardian requirements can change this during a model switch.
    pub(crate) approval_policy: AskForApproval,
    /// Effective reviewer selected for this sampling request's approvals.
    /// Model-specific Guardian requirements can change this during a model switch.
    pub(crate) approvals_reviewer: ApprovalsReviewer,
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
