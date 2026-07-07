use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::agents_md::LoadedAgentsMd;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::McpRuntimeSnapshot;
use crate::session::step_model_context::StepModelContext;
use crate::session::turn_context::TurnContext;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::protocol::TurnContextItem;

/// Request-scoped state that may change between model sampling requests.
#[derive(Debug)]
pub(crate) struct StepContext {
    pub(crate) turn: Arc<TurnContext>,
    pub(crate) model: Arc<StepModelContext>,
    pub(crate) environments: TurnEnvironmentSnapshot,
    /// Capability roots bound to ready environments in this exact step.
    pub(crate) selected_capability_roots: Vec<ResolvedSelectedCapabilityRoot>,
    /// The exact MCP config and manager used to advertise and execute tools for this step.
    pub(crate) mcp: Arc<McpRuntimeSnapshot>,
    /// The canonical AGENTS.md value observed with this environment snapshot.
    pub(crate) loaded_agents_md: Option<Arc<LoadedAgentsMd>>,
}

impl StepContext {
    pub(crate) fn new(
        turn: Arc<TurnContext>,
        model: Arc<StepModelContext>,
        environments: TurnEnvironmentSnapshot,
        selected_capability_roots: Vec<ResolvedSelectedCapabilityRoot>,
        mcp: Arc<McpRuntimeSnapshot>,
        loaded_agents_md: Option<Arc<LoadedAgentsMd>>,
    ) -> Self {
        Self {
            turn,
            model,
            environments,
            selected_capability_roots,
            mcp,
            loaded_agents_md,
        }
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
            model: self.model.model_info.slug.clone(),
            comp_hash: self.model.model_info.comp_hash.clone(),
            personality: self.turn.personality,
            collaboration_mode: Some(self.model.collaboration_mode.clone()),
            multi_agent_version: Some(self.turn.multi_agent_version),
            multi_agent_mode: super::multi_agents::effective_multi_agent_mode(self),
            realtime_active: Some(self.turn.realtime_active),
            effort: self.model.reasoning_effort(),
            summary: ReasoningSummaryConfig::Auto,
        }
    }
}

/// Frozen turn-start inputs used to capture request-scoped [`StepContext`] values.
#[derive(Clone, Debug)]
pub(crate) struct StepContextSeed {
    pub(crate) turn: Arc<TurnContext>,
    pub(crate) model: Arc<StepModelContext>,
}

impl StepContextSeed {
    pub(crate) fn new(turn: Arc<TurnContext>, model: Arc<StepModelContext>) -> Self {
        Self { turn, model }
    }

    pub(crate) async fn with_model(
        &self,
        model: String,
        models_manager: &SharedModelsManager,
    ) -> Self {
        let mut config = (*self.turn.config).clone();
        config.model = Some(model.clone());
        let model_info = models_manager
            .get_model_info(model.as_str(), &config.to_models_manager_config())
            .await;
        let supported_reasoning_levels = model_info
            .supported_reasoning_levels
            .iter()
            .map(|preset| preset.effort.clone())
            .collect::<Vec<_>>();
        let reasoning_effort = if let Some(current_reasoning_effort) = self.model.reasoning_effort()
        {
            if supported_reasoning_levels.contains(&current_reasoning_effort) {
                Some(current_reasoning_effort)
            } else {
                supported_reasoning_levels
                    .get(supported_reasoning_levels.len().saturating_sub(1) / 2)
                    .cloned()
                    .or_else(|| model_info.default_reasoning_level.clone())
            }
        } else {
            supported_reasoning_levels
                .get(supported_reasoning_levels.len().saturating_sub(1) / 2)
                .cloned()
                .or_else(|| model_info.default_reasoning_level.clone())
        };
        let collaboration_mode = self.model.collaboration_mode.with_updates(
            Some(model.clone()),
            Some(reasoning_effort),
            /*developer_instructions*/ None,
        );
        let model_context = StepModelContext {
            session_telemetry: self
                .model
                .session_telemetry
                .clone()
                .with_model(model.as_str(), model_info.slug.as_str()),
            model_info,
            collaboration_mode,
            reasoning_summary: self.model.reasoning_summary,
            service_tier: self.model.service_tier.clone(),
            server_model_warning_emitted: AtomicBool::new(
                self.model
                    .server_model_warning_emitted
                    .load(Ordering::Relaxed),
            ),
            model_verification_emitted: AtomicBool::new(
                self.model
                    .model_verification_emitted
                    .load(Ordering::Relaxed),
            ),
        };
        Self::new(Arc::clone(&self.turn), Arc::new(model_context))
    }
}

impl From<&StepContext> for StepContextSeed {
    fn from(step_context: &StepContext) -> Self {
        Self::new(
            Arc::clone(&step_context.turn),
            Arc::clone(&step_context.model),
        )
    }
}
