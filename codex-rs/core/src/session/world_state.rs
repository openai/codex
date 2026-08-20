use std::sync::Arc;

use super::session::Session;
use super::step_context::StepContext;
use crate::connectors;
use crate::context::ApprovalPromptContext;
use crate::context::TokenBudgetContext;
use crate::context::world_state::AgentsMdState;
use crate::context::world_state::AppsInstructionsState;
use crate::context::world_state::CollaborationModeState;
use crate::context::world_state::CompactPermissionsState;
use crate::context::world_state::ContextWindowGuidanceState;
use crate::context::world_state::EnvironmentsInstructionsState;
use crate::context::world_state::EnvironmentsState;
use crate::context::world_state::ManagedDeveloperInstructionsState;
use crate::context::world_state::ModelInstructionsState;
use crate::context::world_state::MultiAgentModeState;
use crate::context::world_state::MultiAgentUsageHintState;
use crate::context::world_state::PermissionsState;
use crate::context::world_state::PersonalityState;
use crate::context::world_state::PluginsInstructionsState;
use crate::context::world_state::RealtimeState;
use crate::context::world_state::ToolsState;
use crate::context::world_state::WorldState;
use codex_connectors::AppToolPolicyEvaluator;
use codex_extension_api::WorldStateContributionInput;
use codex_features::Feature;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::BaseInstructionsProvenance;

impl Session {
    #[tracing::instrument(name = "world_state.build", level = "info", skip_all)]
    pub(crate) async fn build_world_state_for_step(
        &self,
        step_context: &StepContext,
    ) -> CodexResult<WorldState> {
        let turn_context = step_context.turn.as_ref();
        tracing::trace!(
            selected_capability_root_count = step_context.selected_capability_roots.len(),
            "building step world state"
        );
        let model_instructions = turn_context
            .model_info
            .get_model_instructions(turn_context.personality);
        let (previous_model, previous_context, base_instructions) = {
            let state = self.state.lock().await;
            let base_instructions = state.session_configuration.base_instructions.clone();
            (
                state
                    .previous_turn_settings()
                    .map(|previous| previous.model)
                    .or_else(|| {
                        state
                            .base_instructions_provenance
                            .as_ref()
                            .and_then(|provenance| match provenance {
                                BaseInstructionsProvenance::Model { model } => Some(model),
                                BaseInstructionsProvenance::Custom => None,
                            })
                            .filter(|_| base_instructions != model_instructions)
                            .cloned()
                    }),
                state.reference_context_item(),
                base_instructions,
            )
        };
        let personality_is_baked = turn_context.model_info.supports_personality()
            && base_instructions == model_instructions;
        let environment_subagents = if turn_context.config.include_environment_context {
            self.services
                .agent_control
                .format_environment_context_subagents(self.thread_id)
                .await
        } else {
            String::new()
        };
        let mut world_state = WorldState::default();
        world_state.add_section(ModelInstructionsState::new(
            &turn_context.model_info.slug,
            previous_model.as_deref(),
            model_instructions,
        ));
        if self.features.enabled(Feature::Personality) {
            let personality_instructions = turn_context.personality.and_then(|personality| {
                turn_context
                    .model_info
                    .model_messages
                    .as_ref()
                    .and_then(|messages| messages.get_personality_message(Some(personality)))
                    .filter(|message| !message.is_empty())
            });
            world_state.add_section(PersonalityState::new(
                &turn_context.model_info.slug,
                turn_context.personality,
                previous_context
                    .as_ref()
                    .map(|previous| previous.model.as_str())
                    .or(previous_model.as_deref()),
                previous_context
                    .as_ref()
                    .and_then(|previous| previous.personality),
                personality_instructions,
                personality_is_baked,
            ));
        }
        if turn_context.config.features.enabled(Feature::TokenBudget)
            && turn_context.model_context_window().is_some()
        {
            let window_ids = self.state.lock().await.auto_compact_window_ids();
            world_state.add_section(TokenBudgetContext::new(
                turn_context
                    .session_source
                    .get_agent_path()
                    .unwrap_or_else(codex_protocol::AgentPath::root),
                window_ids.first_window_id,
                window_ids.previous_window_id,
                window_ids.window_id,
                /*mcp_result*/ None,
            ));
            if let Some(guidance) = turn_context
                .config
                .token_budget
                .as_ref()
                .and_then(|config| config.guidance_message.as_deref())
                .filter(|message| !message.trim().is_empty())
            {
                world_state.add_section(ContextWindowGuidanceState::new(guidance));
            }
        }
        let realtime_mode_instructions = self.conversation.mode_instructions().await;
        world_state.add_section(RealtimeState::new(
            turn_context.realtime_active,
            realtime_mode_instructions
                .as_ref()
                .and_then(|instructions| instructions.start.as_deref())
                .or(turn_context
                    .config
                    .experimental_realtime_start_instructions
                    .as_deref()),
            realtime_mode_instructions
                .as_ref()
                .and_then(|instructions| instructions.end.as_deref()),
        ));
        world_state.add_section(AgentsMdState::new(step_context.loaded_agents_md.as_deref()));
        let exec_policy = self
            .services
            .exec_policy
            .current_for_prefix_rules(turn_context.allow_prefix_rules());
        if turn_context.config.include_permissions_instructions {
            let environment = step_context.environments.primary();
            let permission_profile = step_context
                .environments
                .permission_profile_or_else(|| turn_context.permission_profile());
            #[allow(deprecated)]
            let cwd = environment
                .and_then(|environment| environment.cwd().to_abs_path().ok())
                .unwrap_or_else(|| turn_context.cwd.clone());
            let model_messages = turn_context.model_info.model_messages.as_ref();
            world_state.add_section(PermissionsState::new(
                &permission_profile,
                step_context.approval_policy,
                ApprovalPromptContext::new(
                    step_context.approvals_reviewer,
                    model_messages.and_then(|messages| messages.approvals.as_ref()),
                    model_messages.and_then(|messages| messages.permissions.as_ref()),
                ),
                exec_policy.as_ref(),
                &cwd,
                turn_context
                    .config
                    .features
                    .enabled(Feature::ExecPermissionApprovals),
                turn_context
                    .config
                    .features
                    .enabled(Feature::RequestPermissionsTool),
            ));
        } else {
            world_state.add_section(CompactPermissionsState::new(exec_policy.as_ref()));
        }
        if turn_context.config.include_collaboration_mode_instructions {
            world_state.add_section(CollaborationModeState::from_collaboration_mode(
                &turn_context.collaboration_mode(),
                turn_context
                    .model_info
                    .model_messages
                    .as_ref()
                    .and_then(|messages| messages.collaboration_modes.as_ref()),
            ));
        }
        if turn_context.config.include_environment_context {
            let current_date = self
                .services
                .time_provider
                .current_time(self.thread_id())
                .await
                .map_err(|err| CodexErr::Fatal(format!("failed to read current time: {err:#}")))?
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d")
                .to_string();
            world_state.add_section(
                EnvironmentsState::from_turn_context_with_environments(
                    turn_context,
                    &step_context.environments,
                    Some(current_date),
                )
                .with_subagents(environment_subagents),
            );
        }
        world_state.add_section(EnvironmentsInstructionsState::new(
            turn_context.config.include_environment_context
                && turn_context
                    .config
                    .features
                    .enabled(Feature::DeferredExecutor),
        ));
        let apps_available =
            if turn_context.config.include_apps_instructions && turn_context.apps_enabled() {
                AppToolPolicyEvaluator::new(&turn_context.config.config_layer_stack)
                    .apply_app_enabled_state(connectors::accessible_connectors_from_mcp_tools(
                        step_context.mcp.tools(),
                    ))
                    .into_iter()
                    .any(|connector| connector.is_accessible && connector.is_enabled)
            } else {
                false
            };
        let apps_usage_instructions_available =
            apps_available && turn_context.model_info.include_apps_usage_instructions;
        world_state.add_section(AppsInstructionsState::new(
            apps_usage_instructions_available,
        ));
        let plugins_usage_instructions_available = step_context.mcp.plugins_available()
            && turn_context.model_info.include_plugin_usage_instructions;
        world_state.add_section(PluginsInstructionsState::new(
            plugins_usage_instructions_available,
        ));
        if turn_context
            .config
            .features
            .enabled(Feature::DeferredToolWorldState)
        {
            world_state.add_section(ToolsState::new(
                step_context.tool_router.deferred_tool_namespaces(),
            ));
        }
        let environments = step_context.environments.to_selections();
        let ready_selected_capability_roots = step_context
            .selected_capability_roots
            .iter()
            .map(|root| root.selected_root().clone())
            .collect::<Vec<_>>();
        let extension_metrics = super::extension_metrics::from_session_telemetry(
            turn_context.session_telemetry.clone(),
        );
        for contributor in self.services.extensions.context_contributors() {
            for section in contributor
                .contribute_world_state(WorldStateContributionInput {
                    thread_id: self.thread_id(),
                    turn_id: turn_context.sub_id.as_str(),
                    environments: &environments,
                    ready_selected_capability_roots: &ready_selected_capability_roots,
                    executor_capability_discovery: step_context
                        .executor_capability_discovery
                        .as_deref(),
                    extension_metrics: Some(Arc::clone(&extension_metrics)),
                    session_store: &self.services.session_extension_data,
                    thread_store: &self.services.thread_extension_data,
                    turn_store: turn_context.extension_data.as_ref(),
                })
                .await
            {
                world_state.add_extension_section(section);
            }
        }
        let mut multi_agent_mode = MultiAgentModeState::new(
            super::multi_agents::effective_multi_agent_mode(turn_context),
        );
        if let Some(usage_hint_text) =
            super::multi_agents::usage_hint_text(turn_context, &turn_context.session_source)
        {
            let usage_hint = MultiAgentUsageHintState::new(usage_hint_text);
            multi_agent_mode = multi_agent_mode.with_usage_hint(&usage_hint);
            world_state.add_section(usage_hint);
        }
        world_state.add_section(multi_agent_mode);
        world_state.add_section(ManagedDeveloperInstructionsState::new(
            turn_context
                .config
                .config_layer_stack
                .requirements()
                .additional_developer_instructions
                .as_ref(),
        ));
        Ok(world_state)
    }
}
