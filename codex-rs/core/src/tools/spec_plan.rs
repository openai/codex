use crate::tools::code_mode::execute_spec::create_code_mode_tool;
use crate::tools::handlers::ApplyPatchHandler;
use crate::tools::handlers::CodeModeExecuteHandler;
use crate::tools::handlers::CodeModeWaitHandler;
use crate::tools::handlers::ContainerExecHandler;
use crate::tools::handlers::CreateGoalHandler;
use crate::tools::handlers::DynamicToolHandler;
use crate::tools::handlers::ExecCommandHandler;
use crate::tools::handlers::ExecCommandHandlerOptions;
use crate::tools::handlers::GetGoalHandler;
use crate::tools::handlers::ListMcpResourceTemplatesHandler;
use crate::tools::handlers::ListMcpResourcesHandler;
use crate::tools::handlers::LocalShellHandler;
use crate::tools::handlers::McpHandler;
use crate::tools::handlers::PlanHandler;
use crate::tools::handlers::ReadMcpResourceHandler;
use crate::tools::handlers::RequestPermissionsHandler;
use crate::tools::handlers::RequestPluginInstallHandler;
use crate::tools::handlers::RequestUserInputHandler;
use crate::tools::handlers::ShellCommandHandler;
use crate::tools::handlers::ShellCommandHandlerOptions;
use crate::tools::handlers::ShellHandler;
use crate::tools::handlers::TestSyncHandler;
use crate::tools::handlers::ToolSearchHandler;
use crate::tools::handlers::UpdateGoalHandler;
use crate::tools::handlers::ViewImageHandler;
use crate::tools::handlers::WriteStdinHandler;
use crate::tools::handlers::agent_jobs::ReportAgentJobResultHandler;
use crate::tools::handlers::agent_jobs::SpawnAgentsOnCsvHandler;
use crate::tools::handlers::multi_agents::CloseAgentHandler;
use crate::tools::handlers::multi_agents::ResumeAgentHandler;
use crate::tools::handlers::multi_agents::SendInputHandler;
use crate::tools::handlers::multi_agents::SpawnAgentHandler;
use crate::tools::handlers::multi_agents::WaitAgentHandler;
use crate::tools::handlers::multi_agents_spec::SpawnAgentToolOptions;
use crate::tools::handlers::multi_agents_v2::CloseAgentHandler as CloseAgentHandlerV2;
use crate::tools::handlers::multi_agents_v2::FollowupTaskHandler as FollowupTaskHandlerV2;
use crate::tools::handlers::multi_agents_v2::ListAgentsHandler as ListAgentsHandlerV2;
use crate::tools::handlers::multi_agents_v2::SendMessageHandler as SendMessageHandlerV2;
use crate::tools::handlers::multi_agents_v2::SpawnAgentHandler as SpawnAgentHandlerV2;
use crate::tools::handlers::multi_agents_v2::WaitAgentHandler as WaitAgentHandlerV2;
use crate::tools::handlers::shell_spec::ShellToolOptions;
use crate::tools::handlers::view_image_spec::ViewImageToolOptions;
use crate::tools::hosted_spec::WebSearchToolOptions;
use crate::tools::hosted_spec::create_image_generation_tool;
use crate::tools::hosted_spec::create_web_search_tool;
use crate::tools::registry::ToolRegistryBuilder;
use crate::tools::spec_plan_types::ToolRegistryBuildParams;
use crate::tools::spec_plan_types::agent_type_description;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolEnvironmentMode;
use codex_tools::ToolName;
use codex_tools::ToolSearchSource;
use codex_tools::ToolSearchSourceInfo;
use codex_tools::ToolSpec;
use codex_tools::ToolsConfig;
use codex_tools::coalesce_loadable_tool_specs;
use codex_tools::collect_code_mode_exec_prompt_tool_definitions;
use codex_tools::collect_tool_search_source_infos;
use codex_tools::default_namespace_description;
use codex_tools::dynamic_tool_to_loadable_tool_spec;
use codex_tools::mcp_tool_to_responses_api_tool;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::Arc;

pub fn build_tool_registry_builder(
    config: &ToolsConfig,
    params: ToolRegistryBuildParams<'_>,
) -> ToolRegistryBuilder {
    let mut builder = ToolRegistryBuilder::new(config.code_mode_enabled);
    let exec_permission_approvals_enabled = config.exec_permission_approvals_enabled;

    if config.code_mode_enabled {
        let namespace_descriptions = params
            .tool_namespaces
            .into_iter()
            .flatten()
            .map(|(namespace, detail)| {
                (
                    namespace.clone(),
                    codex_code_mode::ToolNamespaceDescription {
                        name: detail.name.clone(),
                        description: detail.description.clone().unwrap_or_default(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let nested_config = config.for_code_mode_nested_tools();
        let nested_builder = build_tool_registry_builder(
            &nested_config,
            ToolRegistryBuildParams {
                discoverable_tools: None,
                ..params
            },
        );
        let mut enabled_tools = collect_code_mode_exec_prompt_tool_definitions(
            nested_builder
                .specs()
                .iter()
                .map(|configured_tool| &configured_tool.spec),
        );
        enabled_tools
            .sort_by(|left, right| compare_code_mode_tools(left, right, &namespace_descriptions));
        builder.register_handler(Arc::new(CodeModeExecuteHandler::new(
            create_code_mode_tool(
                &enabled_tools,
                &namespace_descriptions,
                config.code_mode_only_enabled,
                config.search_tool
                    && params
                        .deferred_mcp_tools
                        .is_some_and(|tools| !tools.is_empty()),
            ),
        )));
        builder.register_handler(Arc::new(CodeModeWaitHandler));
    }

    if config.environment_mode.has_environment() {
        let include_environment_id =
            matches!(config.environment_mode, ToolEnvironmentMode::Multiple);
        match &config.shell_type {
            ConfigShellToolType::Default => {
                builder.register_handler(Arc::new(ShellHandler::new(ShellToolOptions {
                    exec_permission_approvals_enabled,
                })));
            }
            ConfigShellToolType::Local => {
                builder.register_handler(Arc::new(LocalShellHandler::new()));
            }
            ConfigShellToolType::UnifiedExec => {
                builder.register_handler(Arc::new(ExecCommandHandler::new(
                    ExecCommandHandlerOptions {
                        allow_login_shell: config.allow_login_shell,
                        exec_permission_approvals_enabled,
                        include_environment_id,
                    },
                )));
                builder.register_handler(Arc::new(WriteStdinHandler));
            }
            ConfigShellToolType::Disabled => {}
            ConfigShellToolType::ShellCommand => {
                builder.register_handler(Arc::new(ShellCommandHandler::new(
                    ShellCommandHandlerOptions {
                        backend_config: config.shell_command_backend,
                        allow_login_shell: config.allow_login_shell,
                        exec_permission_approvals_enabled,
                    },
                )));
            }
        }
    }

    if config.environment_mode.has_environment()
        && config.shell_type != ConfigShellToolType::Disabled
    {
        match &config.shell_type {
            ConfigShellToolType::Default => {
                builder.register_handler(Arc::new(ContainerExecHandler));
                builder.register_handler(Arc::new(LocalShellHandler::default()));
                builder.register_handler(Arc::new(ShellCommandHandler::from(
                    config.shell_command_backend,
                )));
            }
            ConfigShellToolType::Local => {
                builder.register_handler(Arc::new(ShellHandler::default()));
                builder.register_handler(Arc::new(ContainerExecHandler));
                builder.register_handler(Arc::new(ShellCommandHandler::from(
                    config.shell_command_backend,
                )));
            }
            ConfigShellToolType::UnifiedExec => {
                builder.register_handler(Arc::new(ShellHandler::default()));
                builder.register_handler(Arc::new(ContainerExecHandler));
                builder.register_handler(Arc::new(LocalShellHandler::default()));
                builder.register_handler(Arc::new(ShellCommandHandler::from(
                    config.shell_command_backend,
                )));
            }
            ConfigShellToolType::ShellCommand => {
                builder.register_handler(Arc::new(ShellHandler::default()));
                builder.register_handler(Arc::new(ContainerExecHandler));
                builder.register_handler(Arc::new(LocalShellHandler::default()));
            }
            ConfigShellToolType::Disabled => {}
        }
    }

    if params.mcp_tools.is_some() {
        builder.register_handler(Arc::new(ListMcpResourcesHandler));
        builder.register_handler(Arc::new(ListMcpResourceTemplatesHandler));
        builder.register_handler(Arc::new(ReadMcpResourceHandler));
    }

    builder.register_handler(Arc::new(PlanHandler));
    if config.goal_tools {
        builder.register_handler(Arc::new(GetGoalHandler));
        builder.register_handler(Arc::new(CreateGoalHandler));
        builder.register_handler(Arc::new(UpdateGoalHandler));
    }

    builder.register_handler(Arc::new(RequestUserInputHandler {
        available_modes: config.request_user_input_available_modes.clone(),
    }));

    if config.request_permissions_tool_enabled {
        builder.register_handler(Arc::new(RequestPermissionsHandler));
    }

    let deferred_dynamic_tools = params
        .dynamic_tools
        .iter()
        .filter(|tool| tool.defer_loading && (config.namespace_tools || tool.namespace.is_none()))
        .collect::<Vec<_>>();
    let deferred_mcp_tools_for_search = if config.namespace_tools {
        params.deferred_mcp_tools
    } else {
        None
    };

    if config.search_tool
        && (deferred_mcp_tools_for_search.is_some() || !deferred_dynamic_tools.is_empty())
    {
        let mut search_source_infos = deferred_mcp_tools_for_search
            .map(|deferred_mcp_tools| {
                collect_tool_search_source_infos(deferred_mcp_tools.iter().map(|tool| {
                    ToolSearchSource {
                        server_name: tool.server_name,
                        connector_name: tool.connector_name,
                        description: tool.description,
                    }
                }))
            })
            .unwrap_or_default();

        if !deferred_dynamic_tools.is_empty() {
            search_source_infos.push(ToolSearchSourceInfo {
                name: "Dynamic tools".to_string(),
                description: Some("Tools provided by the current Codex thread.".to_string()),
            });
        }

        builder.register_handler(Arc::new(ToolSearchHandler::new(
            params.tool_search_entries.to_vec(),
            search_source_infos,
        )));
    }

    if config.tool_suggest
        && let Some(discoverable_tools) =
            params.discoverable_tools.filter(|tools| !tools.is_empty())
    {
        builder.register_handler(Arc::new(RequestPluginInstallHandler::new(
            discoverable_tools,
        )));
    }

    if config.environment_mode.has_environment()
        && let Some(apply_patch_tool_type) = &config.apply_patch_tool_type
    {
        builder.register_handler(Arc::new(ApplyPatchHandler::new(
            apply_patch_tool_type.clone(),
        )));
    }

    if config
        .experimental_supported_tools
        .iter()
        .any(|tool| tool == "test_sync_tool")
    {
        builder.register_handler(Arc::new(TestSyncHandler));
    }

    if let Some(web_search_tool) = create_web_search_tool(WebSearchToolOptions {
        web_search_mode: config.web_search_mode,
        web_search_config: config.web_search_config.as_ref(),
        web_search_tool_type: config.web_search_tool_type,
    }) {
        builder.push_spec(web_search_tool, /*supports_parallel_tool_calls*/ false);
    }

    if config.image_gen_tool {
        builder.push_spec(
            create_image_generation_tool("png"),
            /*supports_parallel_tool_calls*/ false,
        );
    }

    if config.environment_mode.has_environment() {
        let include_environment_id =
            matches!(config.environment_mode, ToolEnvironmentMode::Multiple);
        builder.register_handler(Arc::new(ViewImageHandler::new(ViewImageToolOptions {
            can_request_original_image_detail: config.can_request_original_image_detail,
            include_environment_id,
        })));
    }

    if config.collab_tools {
        if config.multi_agent_v2 {
            let agent_type_description =
                agent_type_description(config, params.default_agent_type_description);
            builder.register_handler(Arc::new(SpawnAgentHandlerV2::new(SpawnAgentToolOptions {
                available_models: config.available_models.clone(),
                agent_type_description,
                hide_agent_type_model_reasoning: config.hide_spawn_agent_metadata,
                include_usage_hint: config.spawn_agent_usage_hint,
                usage_hint_text: config.spawn_agent_usage_hint_text.clone(),
                max_concurrent_threads_per_session: config.max_concurrent_threads_per_session,
            })));
            builder.register_handler(Arc::new(SendMessageHandlerV2));
            builder.register_handler(Arc::new(FollowupTaskHandlerV2));
            builder.register_handler(Arc::new(WaitAgentHandlerV2::new(
                params.wait_agent_timeouts,
            )));
            builder.register_handler(Arc::new(CloseAgentHandlerV2));
            builder.register_handler(Arc::new(ListAgentsHandlerV2));
        } else {
            let agent_type_description =
                agent_type_description(config, params.default_agent_type_description);
            builder.register_handler(Arc::new(SpawnAgentHandler::new(SpawnAgentToolOptions {
                available_models: config.available_models.clone(),
                agent_type_description,
                hide_agent_type_model_reasoning: config.hide_spawn_agent_metadata,
                include_usage_hint: config.spawn_agent_usage_hint,
                usage_hint_text: config.spawn_agent_usage_hint_text.clone(),
                max_concurrent_threads_per_session: config.max_concurrent_threads_per_session,
            })));
            builder.register_handler(Arc::new(SendInputHandler));
            builder.register_handler(Arc::new(ResumeAgentHandler));
            builder.register_handler(Arc::new(WaitAgentHandler::new(params.wait_agent_timeouts)));
            builder.register_handler(Arc::new(CloseAgentHandler));
        }
    }

    if config.agent_jobs_tools {
        builder.register_handler(Arc::new(SpawnAgentsOnCsvHandler));
        if config.agent_jobs_worker_tools {
            builder.register_handler(Arc::new(ReportAgentJobResultHandler));
        }
    }

    if let Some(mcp_tools) = params.mcp_tools {
        let mut entries = mcp_tools.to_vec();
        entries.sort_by_key(|tool| tool.name.display());
        let mut namespace_entries = BTreeMap::new();

        for tool in entries {
            let Some(namespace) = tool.name.namespace.as_ref() else {
                let tool_name = &tool.name;
                tracing::error!("Skipping MCP tool `{tool_name}`: MCP tools must be namespaced");
                continue;
            };
            namespace_entries
                .entry(namespace.clone())
                .or_insert_with(Vec::new)
                .push(tool);
        }

        for (namespace, mut entries) in namespace_entries {
            entries.sort_by_key(|tool| tool.name.name.clone());
            let tool_namespace = params
                .tool_namespaces
                .and_then(|namespaces| namespaces.get(&namespace));
            let description = tool_namespace
                .and_then(|namespace| namespace.description.as_deref())
                .map(str::trim)
                .filter(|description| !description.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    let namespace_name = tool_namespace
                        .map(|namespace| namespace.name.as_str())
                        .unwrap_or(namespace.as_str());
                    default_namespace_description(namespace_name)
                });
            let mut tools = Vec::new();
            for tool in entries {
                match mcp_tool_to_responses_api_tool(&tool.name, tool.tool) {
                    Ok(converted_tool) => {
                        tools.push(ResponsesApiNamespaceTool::Function(converted_tool));
                        builder.register_handler(Arc::new(McpHandler::new(tool.name)));
                    }
                    Err(error) => {
                        let tool_name = &tool.name;
                        tracing::error!(
                            "Failed to convert `{tool_name}` MCP tool to OpenAI tool: {error:?}"
                        );
                    }
                }
            }

            if config.namespace_tools && !tools.is_empty() {
                builder.push_spec(
                    ToolSpec::Namespace(ResponsesApiNamespace {
                        name: namespace,
                        description,
                        tools,
                    }),
                    /*supports_parallel_tool_calls*/ false,
                );
            }
        }
    }

    let mut dynamic_tool_specs = Vec::new();
    for tool in params.dynamic_tools {
        match dynamic_tool_to_loadable_tool_spec(tool) {
            Ok(loadable_tool) => {
                let handler_name = ToolName::new(tool.namespace.clone(), tool.name.clone());
                dynamic_tool_specs.push(loadable_tool);
                builder.register_handler(Arc::new(DynamicToolHandler::new(handler_name)));
            }
            Err(error) => {
                tracing::error!(
                    "Failed to convert dynamic tool {:?} to OpenAI tool: {error:?}",
                    tool.name
                );
            }
        }
    }
    for spec in coalesce_loadable_tool_specs(dynamic_tool_specs) {
        let spec = spec.into();
        if config.namespace_tools || !matches!(spec, ToolSpec::Namespace(_)) {
            builder.push_spec(spec, /*supports_parallel_tool_calls*/ false);
        }
    }

    if let Some(deferred_mcp_tools) = params.deferred_mcp_tools {
        let directly_registered_mcp_tools = params
            .mcp_tools
            .into_iter()
            .flatten()
            .map(|direct| direct.name.clone())
            .collect::<HashSet<_>>();
        for tool in deferred_mcp_tools {
            if !directly_registered_mcp_tools.contains(&tool.name) {
                builder.register_handler(Arc::new(McpHandler::new(tool.name.clone())));
            }
        }
    }

    builder
}

fn compare_code_mode_tools(
    left: &codex_code_mode::ToolDefinition,
    right: &codex_code_mode::ToolDefinition,
    namespace_descriptions: &BTreeMap<String, codex_code_mode::ToolNamespaceDescription>,
) -> std::cmp::Ordering {
    let left_namespace = code_mode_namespace_name(left, namespace_descriptions);
    let right_namespace = code_mode_namespace_name(right, namespace_descriptions);

    left_namespace
        .cmp(&right_namespace)
        .then_with(|| left.tool_name.name.cmp(&right.tool_name.name))
        .then_with(|| left.name.cmp(&right.name))
}

fn code_mode_namespace_name<'a>(
    tool: &codex_code_mode::ToolDefinition,
    namespace_descriptions: &'a BTreeMap<String, codex_code_mode::ToolNamespaceDescription>,
) -> Option<&'a str> {
    tool.tool_name
        .namespace
        .as_ref()
        .and_then(|namespace| namespace_descriptions.get(namespace))
        .map(|namespace_description| namespace_description.name.as_str())
}

#[cfg(test)]
#[path = "spec_plan_tests.rs"]
mod tests;
