use super::session::Session;
use super::step_context::StepContext;
use crate::connectors;
use crate::context::ApprovalPromptContext;
use crate::context::world_state::AgentsMdState;
use crate::context::world_state::AppsInstructionsState;
use crate::context::world_state::CollaborationModeState;
use crate::context::world_state::EnvironmentsInstructionsState;
use crate::context::world_state::EnvironmentsState;
use crate::context::world_state::PermissionsState;
use crate::context::world_state::PluginsInstructionsState;
use crate::context::world_state::RealtimeState;
use crate::context::world_state::WorldState;
use codex_extension_api::WorldStateContributionInput;
use codex_features::Feature;

impl Session {
    #[tracing::instrument(name = "world_state.build", level = "info", skip_all)]
    pub(crate) async fn build_world_state_for_step(
        &self,
        step_context: &StepContext,
    ) -> WorldState {
        let turn_context = step_context.turn.as_ref();
        tracing::trace!(
            selected_capability_root_count = step_context.selected_capability_roots.len(),
            "building step world state"
        );
        let environment_subagents = if turn_context.config.include_environment_context {
            self.services
                .agent_control
                .format_environment_context_subagents(self.thread_id)
                .await
        } else {
            String::new()
        };
        let mut world_state = WorldState::default();
        world_state.add_section(RealtimeState::new(
            turn_context.realtime_active,
            turn_context
                .config
                .experimental_realtime_start_instructions
                .as_deref(),
        ));
        world_state.add_section(AgentsMdState::new(step_context.loaded_agents_md.as_deref()));
        if turn_context.config.include_permissions_instructions {
            let permission_profile = turn_context.permission_profile();
            let model_messages = turn_context.model_info.model_messages.as_ref();
            let exec_policy = self.services.exec_policy.current();
            world_state.add_section(PermissionsState::new(
                &permission_profile,
                turn_context.approval_policy.value(),
                ApprovalPromptContext::new(
                    turn_context.config.approvals_reviewer,
                    model_messages.and_then(|messages| messages.approvals.as_ref()),
                    model_messages.and_then(|messages| messages.permissions.as_ref()),
                ),
                exec_policy.as_ref(),
                #[allow(deprecated)]
                &turn_context.cwd,
                turn_context
                    .config
                    .features
                    .enabled(Feature::ExecPermissionApprovals),
                turn_context
                    .config
                    .features
                    .enabled(Feature::RequestPermissionsTool),
            ));
        }
        if turn_context.config.include_collaboration_mode_instructions
            && let Some(collaboration_mode) =
                CollaborationModeState::from_collaboration_mode(&turn_context.collaboration_mode())
        {
            world_state.add_section(collaboration_mode);
        }
        if turn_context.config.include_environment_context {
            world_state.add_section(
                EnvironmentsState::from_turn_context_with_environments(
                    turn_context,
                    &step_context.environments,
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
                let tools = step_context.mcp_tools().await;
                connectors::with_app_enabled_state(
                    connectors::accessible_connectors_from_mcp_tools(tools),
                    &turn_context.config,
                )
                .into_iter()
                .any(|connector| connector.is_accessible && connector.is_enabled)
            } else {
                false
            };
        world_state.add_section(AppsInstructionsState::new(apps_available));
        world_state.add_section(PluginsInstructionsState::new(
            step_context.mcp.plugins_available(),
        ));
        let environments = step_context.environments.to_selections();
        let ready_selected_capability_roots = step_context
            .selected_capability_roots
            .iter()
            .map(|root| root.selected_root().clone())
            .collect::<Vec<_>>();
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
                    session_store: &self.services.session_extension_data,
                    thread_store: &self.services.thread_extension_data,
                    turn_store: turn_context.extension_data.as_ref(),
                })
                .await
            {
                world_state.add_extension_section(section);
            }
        }
        world_state
    }
}
