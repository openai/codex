use std::sync::Arc;

use super::session::Session;
use super::step_context::StepContext;
use super::turn_context::TurnContext;
use crate::agent::host::AgentRuntimeServices;
use crate::agent::host::AgentSessionReader;
use crate::connectors;
use crate::context::TokenBudgetContext;
use crate::context::world_state::AgentsMdState;
use crate::context::world_state::AppsInstructionsState;
use crate::context::world_state::CollaborationModeState;
use crate::context::world_state::CompactPermissionsState;
use crate::context::world_state::ContextWindowGuidanceState;
use crate::context::world_state::EnvironmentsInstructionsState;
use crate::context::world_state::EnvironmentsState;
use crate::context::world_state::ManagedDeveloperInstructionsState;
use crate::context::world_state::ModelCatalogState;
use crate::context::world_state::ModelInstructionsState;
use crate::context::world_state::MultiAgentModeState;
use crate::context::world_state::MultiAgentUsageHintState;
use crate::context::world_state::PermissionsState;
use crate::context::world_state::PersistentModeState;
use crate::context::world_state::PluginsInstructionsState;
use crate::context::world_state::RealtimeState;
use crate::context::world_state::ToolsState;
use crate::context::world_state::TopLevelToolsState;
use crate::context::world_state::WorldState;
use crate::context::world_state::format_agent_paths;
use crate::tools::handlers::multi_agents_spec::MULTI_AGENT_V1_NAMESPACE;
use codex_connectors::AppToolPolicyEvaluator;
use codex_extension_api::WorldStateContributionInput;
use codex_features::Feature;
use codex_file_system::FileSystemSandboxContext;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_prompts::ApprovalPromptContext;
use codex_prompts::ResolvedModelMessages;
use codex_prompts::render_model_instructions;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::BaseInstructionsProvenance;
use codex_protocol::models::ContentItemNamespace;
use codex_protocol::protocol::MultiAgentVersion;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

impl Session {
    pub(super) fn host_roster_reader(
        &self,
        turn_context: &TurnContext,
    ) -> Option<&Arc<dyn AgentSessionReader>> {
        if turn_context.config.include_environment_context
            && turn_context.multi_agent_version == MultiAgentVersion::V2
            && let AgentRuntimeServices::Host(services) = &self.agent_runtime
        {
            services.session_reader.as_ref()
        } else {
            None
        }
    }

    pub(crate) async fn current_window_uses_incremental_tools(
        &self,
        step_context: &StepContext,
    ) -> bool {
        if step_context.incremental_tools_feature().is_none() {
            return false;
        }
        let state = self.state.lock().await;
        if state.history.annotated_items().is_empty() {
            return step_context.incremental_tools_enabled();
        }
        state.history.has_tool_declarations()
    }

    #[tracing::instrument(name = "world_state.build", level = "info", skip_all)]
    pub(crate) async fn build_world_state_for_step(
        &self,
        step_context: &StepContext,
        new_window: bool,
    ) -> CodexResult<WorldState> {
        let incremental_tools = if new_window {
            step_context.incremental_tools_enabled()
        } else {
            self.current_window_uses_incremental_tools(step_context)
                .await
        };
        let turn_context = step_context.turn.as_ref();
        let settings = &step_context.settings;
        let model_info = settings.model_info.as_ref();
        let model_messages = ResolvedModelMessages::from_model(model_info);
        tracing::trace!(
            selected_capability_root_count = step_context
                .extension_data
                .get::<Vec<codex_exec_server::ResolvedSelectedCapabilityRoot>>()
                .map_or(0, |roots| roots.len()),
            "building step world state"
        );
        let render_current_instructions = || {
            let instructions = render_model_instructions(model_info);
            if !turn_context.config.update_plan_enabled
                && turn_context.config.model_catalog.is_none()
                && (incremental_tools
                    || turn_context.config.base_instructions.is_none()
                    || matches!(
                        turn_context.config.base_instructions_provenance,
                        Some(BaseInstructionsProvenance::Model { .. })
                    ))
            {
                codex_prompts::without_update_plan_instructions(&instructions)
            } else {
                instructions
            }
        };
        let model_state = if incremental_tools {
            let base_instructions = self.get_base_instructions().await;
            let instructions = match base_instructions.provenance {
                Some(BaseInstructionsProvenance::Model { .. }) => render_current_instructions(),
                Some(BaseInstructionsProvenance::Custom) | None => base_instructions.text,
            };
            ModelInstructionsState::for_incremental_tools(
                &model_info.slug,
                instructions,
                turn_context
                    .config
                    .features
                    .enabled(Feature::SuppressModelChangeAnnouncements),
            )
        } else {
            let model_instructions = render_current_instructions();
            let base_instructions = self.get_prompt_base_instructions().await.text;
            let previous_model = {
                let state = self.state.lock().await;
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
                    })
            };
            ModelInstructionsState::new(
                &model_info.slug,
                previous_model.as_deref(),
                model_instructions,
            )
        };
        let environment_subagents = if turn_context.config.include_environment_context
            && matches!(self.agent_runtime, AgentRuntimeServices::Controller)
        {
            match turn_context.multi_agent_version {
                MultiAgentVersion::V2 => format_agent_paths(
                    self.services
                        .agent_control
                        .child_agent_paths(self.thread_id)
                        .await
                        .into_iter(),
                ),
                MultiAgentVersion::Disabled | MultiAgentVersion::V1 => {
                    self.services
                        .local_agent_runtime
                        .format_legacy_environment_context_subagents(self.thread_id)
                        .await
                }
            }
        } else {
            String::new()
        };
        let mut world_state = WorldState::default();
        if turn_context
            .config
            .features
            .enabled(Feature::CommandSessionsWorldState)
        {
            let mut sessions =
                crate::unified_exec::command_sessions_state(&self.services.unified_exec_manager)
                    .await;
            if step_context
                .tool_router
                .exposes_tool(&ToolName::plain("write_stdin"))
            {
                world_state.add_section(sessions);
            } else {
                sessions.polling_unavailable = true;
                world_state.add_section(crate::context::world_state::DisabledCommandSessionsState(
                    sessions,
                ));
            }
        } else {
            world_state
                .add_section(crate::context::world_state::DisabledCommandSessionsState::default());
        }
        let extension_metrics = super::extension_metrics::from_session_telemetry(
            step_context.session_telemetry.clone(),
        );
        let preserve_parent_tool_declarations =
            !new_window && self.preserve_parent_tool_declarations();
        if incremental_tools && !preserve_parent_tool_declarations {
            let specs = step_context.tool_router.model_visible_specs();
            let specs = specs
                .iter()
                .filter(|tool| !matches!(tool, ToolSpec::WebSearch { .. }))
                .cloned()
                .collect::<Vec<_>>();
            let definitions = codex_tools::create_tools_json_for_responses_lite(&specs)?;
            let mut top_level = TopLevelToolsState::new(
                model_messages.incremental_tools(),
                definitions,
                Some(Arc::clone(&extension_metrics)),
            )?;
            if let Some(entries) = step_context.tool_router.exec_definitions() {
                top_level = top_level.with_incremental_exec_tools(Some(entries));
            }
            world_state.add_section(top_level);
        }
        world_state.add_section(model_state);
        let token_budget_enabled = turn_context.config.features.enabled(Feature::TokenBudget)
            && step_context
                .settings
                .model_info
                .resolved_context_window()
                .is_some();
        if token_budget_enabled {
            let window_ids = self.state.lock().await.auto_compact_window_ids().clone();
            world_state.add_section(TokenBudgetContext::new(
                turn_context
                    .session_source
                    .get_agent_path()
                    .unwrap_or_else(codex_protocol::AgentPath::root),
                window_ids.first_window_id,
                window_ids.previous_window_id,
                window_ids.window_id,
            ));
        }
        let guidance = step_context
            .token_budget
            .as_ref()
            .and_then(|config| config.guidance_message.as_deref())
            .filter(|_| token_budget_enabled);
        world_state.add_section(ContextWindowGuidanceState::new(guidance));
        let realtime = &step_context.realtime;
        world_state.add_section(RealtimeState::new(
            realtime.active,
            realtime
                .mode_instructions
                .as_ref()
                .and_then(|instructions| instructions.start.as_deref())
                .or(turn_context
                    .config
                    .experimental_realtime_start_instructions
                    .as_deref()),
            realtime
                .mode_instructions
                .as_ref()
                .and_then(|instructions| instructions.end.as_deref()),
        ));
        world_state.add_section(AgentsMdState::new(step_context.loaded_agents_md.as_deref()));
        let exec_policy = self
            .services
            .exec_policy
            .current_for_prefix_rules(turn_context.allow_prefix_rules());
        let approval_context =
            ApprovalPromptContext::new(settings.approvals_reviewer(), model_messages);
        if turn_context.config.include_permissions_instructions {
            let environment = step_context.environments.primary();
            let sandbox = environment
                .filter(|environment| environment.environment().is_remote())
                .map(|environment| {
                    environment.sandbox_context(/*additional_permissions*/ None)
                });
            let paths = sandbox
                .as_ref()
                .map(FileSystemSandboxContext::policy_context);
            let permission_profile =
                turn_context.permission_profile_for_environments(&step_context.environments);
            #[allow(deprecated)]
            let cwd = environment
                .and_then(|environment| environment.cwd().to_abs_path().ok())
                .unwrap_or_else(|| turn_context.cwd.clone());
            world_state.add_section(PermissionsState::new(
                &permission_profile,
                settings.approval_policy(),
                approval_context,
                exec_policy.as_ref(),
                &cwd,
                paths.as_ref(),
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
            world_state.add_section(CompactPermissionsState::new(
                exec_policy.as_ref(),
                approval_context.approved_command_prefix_saved_heading(),
            ));
        }
        if turn_context.config.include_collaboration_mode_instructions {
            world_state.add_section(CollaborationModeState::from_collaboration_mode(
                &settings.effective_collaboration_mode(),
                model_messages.collaboration_modes(),
                turn_context.config.update_plan_enabled,
                turn_context.config.model_catalog.is_some(),
            ));
        }
        if !crate::guardian::is_basic_session_source(&turn_context.session_source) {
            let send_user_message_async_available = !turn_context.root_capabilities_restricted
                && step_context
                    .settings
                    .model_info
                    .experimental_supported_tools
                    .iter()
                    .any(|tool| tool == "send_user_message_async");
            world_state.add_section(PersistentModeState::new(
                turn_context.config.features.persistent_execution_enabled(
                    step_context.settings.effective_reasoning_effort().as_ref(),
                ),
                model_messages.persistent_instructions(),
                send_user_message_async_available,
            ));
        }
        if turn_context.config.include_environment_context {
            world_state.host_roster_reader = self.host_roster_reader(turn_context).cloned();
            let current_date = if turn_context.config.include_environment_context_time {
                self.read_clock_for_context(turn_context, "environment_date")
                    .await?
                    .map(|current_time| {
                        current_time
                            .with_timezone(&chrono::Local)
                            .format("%Y-%m-%d")
                            .to_string()
                    })
            } else {
                None
            };
            world_state.add_section(
                EnvironmentsState::from_turn_context_with_environments(
                    turn_context,
                    &step_context.environments,
                    current_date,
                )
                .await
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
            apps_available && model_info.include_apps_usage_instructions;
        let apps_messages = if turn_context
            .config
            .features
            .enabled(Feature::CatalogAppsInstructions)
        {
            model_messages
        } else {
            ResolvedModelMessages::bundled()
        };
        world_state.add_section(AppsInstructionsState::new(
            apps_usage_instructions_available,
            apps_messages.apps_usage_instructions(CODEX_APPS_MCP_SERVER_NAME),
        ));
        let plugins_usage_instructions_available =
            step_context.mcp.plugins_available() && model_info.include_plugin_usage_instructions;
        let plugins_state = {
            let state = self.state.lock().await;
            PluginsInstructionsState::new(
                plugins_usage_instructions_available,
                model_messages.generic_plugin(),
                state.history.raw_items(),
            )
        };
        world_state.add_section(plugins_state);
        if turn_context
            .config
            .features
            .enabled(Feature::DeferredToolWorldState)
        {
            world_state.add_section(ToolsState::new(
                step_context.tool_router.deferred_tool_namespaces(),
                Arc::clone(&extension_metrics),
            ));
        }
        let environments = step_context.environments.to_selections();
        let previous_world_state = self.state.lock().await.history.world_state_checkpoint();
        for contributor in self.services.extensions.context_contributors() {
            for section in contributor
                .contribute_world_state(WorldStateContributionInput {
                    thread_id: self.thread_id(),
                    turn_id: turn_context.sub_id.as_str(),
                    model_info: &step_context.settings.model_info,
                    environments: &environments,
                    extension_metrics: Some(Arc::clone(&extension_metrics)),
                    session_store: &self.services.session_extension_data,
                    thread_store: &self.services.thread_extension_data,
                    turn_store: turn_context.extension_data.as_ref(),
                    step_store: &step_context.extension_data,
                    previous_world_state: previous_world_state.as_ref().map(|state| &state.state),
                })
                .await
            {
                world_state.add_extension_section(section);
            }
        }
        let v2_namespace = turn_context
            .config
            .multi_agent_v2
            .tool_namespace
            .clone()
            .map(ContentItemNamespace::from)
            .unwrap_or(ContentItemNamespace::Functions);
        let mut multi_agent_mode = MultiAgentModeState::new(
            super::multi_agents::effective_multi_agent_mode(step_context),
        )
        .with_namespace(v2_namespace.clone());
        if let Some(usage_hint_text) = super::multi_agents::usage_hint_text(step_context) {
            let usage_hint =
                MultiAgentUsageHintState::new(usage_hint_text).with_namespace(v2_namespace.clone());
            multi_agent_mode = multi_agent_mode.with_usage_hint(&usage_hint);
            world_state.add_section(usage_hint);
        }
        world_state.add_section(multi_agent_mode);
        let spawn_tool = match turn_context.multi_agent_version {
            MultiAgentVersion::Disabled => None,
            MultiAgentVersion::V1 => Some(ToolName::new(
                Some(MULTI_AGENT_V1_NAMESPACE.to_string()),
                "spawn_agent",
            )),
            MultiAgentVersion::V2 => turn_context
                .config
                .multi_agent_v2
                .expose_spawn_agent_model_overrides
                .then(|| {
                    ToolName::new(
                        turn_context.config.multi_agent_v2.tool_namespace.clone(),
                        "spawn_agent",
                    )
                }),
        };
        let model_overrides_available =
            spawn_tool.is_some_and(|name| step_context.tool_router.exposes_tool(&name));
        world_state.add_section(
            if model_overrides_available
                && turn_context
                    .config
                    .features
                    .enabled(Feature::ModelCatalogInContext)
            {
                ModelCatalogState::new(
                    &turn_context.available_models,
                    turn_context.multi_agent_version,
                )
            } else {
                ModelCatalogState::default()
            }
            .with_namespace(match turn_context.multi_agent_version {
                MultiAgentVersion::Disabled => None,
                MultiAgentVersion::V1 => Some(ContentItemNamespace::MultiAgentV1),
                MultiAgentVersion::V2 => Some(v2_namespace),
            }),
        );
        if !crate::guardian::is_basic_session_source(&turn_context.session_source) {
            world_state.add_section(ManagedDeveloperInstructionsState::new(
                turn_context
                    .config
                    .config_layer_stack
                    .requirements()
                    .additional_developer_instructions
                    .as_ref(),
            ));
        }
        Ok(world_state)
    }
}
