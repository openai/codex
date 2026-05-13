use crate::tools::code_mode::execute_spec::create_code_mode_tool;
use crate::tools::handlers::ApplyPatchHandler;
use crate::tools::handlers::CodeModeExecuteHandler;
use crate::tools::handlers::CodeModeWaitHandler;
use crate::tools::handlers::CreateGoalHandler;
use crate::tools::handlers::DynamicToolHandler;
use crate::tools::handlers::ExecCommandHandler;
use crate::tools::handlers::ExecCommandHandlerOptions;
use crate::tools::handlers::GetGoalHandler;
use crate::tools::handlers::ListMcpResourceTemplatesHandler;
use crate::tools::handlers::ListMcpResourcesHandler;
use crate::tools::handlers::McpHandler;
use crate::tools::handlers::PlanHandler;
use crate::tools::handlers::ReadMcpResourceHandler;
use crate::tools::handlers::RequestPermissionsHandler;
use crate::tools::handlers::RequestPluginInstallHandler;
use crate::tools::handlers::RequestUserInputHandler;
use crate::tools::handlers::ShellCommandHandler;
use crate::tools::handlers::ShellCommandHandlerOptions;
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
use crate::tools::handlers::view_image_spec::ViewImageToolOptions;
use crate::tools::hosted_spec::WebSearchToolOptions;
use crate::tools::hosted_spec::create_image_generation_tool;
use crate::tools::hosted_spec::create_web_search_tool;
use crate::tools::registry::RegisteredTool;
use crate::tools::registry::ToolExposure;
use crate::tools::registry::ToolRegistryBuilder;
use crate::tools::spec_plan_types::ToolRegistryBuildParams;
use crate::tools::spec_plan_types::agent_type_description;
use codex_extension_api::ExtensionToolExecutor;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolEnvironmentMode;
use codex_tools::ToolSpec;
use codex_tools::ToolsConfig;
use codex_tools::collect_code_mode_exec_prompt_tool_definitions;
use codex_tools::default_namespace_description;
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn build_tool_registry_builder(
    config: &ToolsConfig,
    params: ToolRegistryBuildParams<'_>,
) -> ToolRegistryBuilder {
    let mut builder = ToolRegistryBuilder::new();
    let handlers = collect_handler_tools(config, params);
    let deferred_tools_available = handlers
        .iter()
        .any(|handler| handler.exposure() == ToolExposure::Deferred);

    for handler in build_code_mode_handlers(
        config,
        &handlers,
        params.extension_tool_executors,
        config.search_tool && deferred_tools_available,
    ) {
        builder.register_tool(handler);
    }

    let mut non_deferred_specs = Vec::new();
    let mut deferred_search_infos = Vec::new();
    for handler in &handlers {
        match handler.exposure() {
            ToolExposure::Direct => {
                if let Some(spec) = handler.spec() {
                    non_deferred_specs.push(spec);
                }
            }
            ToolExposure::Deferred => {
                if let Some(search_info) = handler.search_info() {
                    deferred_search_infos.push(search_info);
                }
            }
        }
    }

    if let Some(web_search_tool) = create_web_search_tool(WebSearchToolOptions {
        web_search_mode: config.web_search_mode,
        web_search_config: config.web_search_config.as_ref(),
        web_search_tool_type: config.web_search_tool_type,
    }) {
        non_deferred_specs.push(web_search_tool);
    }
    if config.image_gen_tool {
        non_deferred_specs.push(create_image_generation_tool("png"));
    }

    for spec in merge_into_namespaces(non_deferred_specs) {
        if !config.namespace_tools && matches!(spec, ToolSpec::Namespace(_)) {
            continue;
        }
        let spec = if config.code_mode_enabled {
            codex_tools::augment_tool_spec_for_code_mode(spec)
        } else {
            spec
        };
        builder.push_spec(spec);
    }

    for handler in handlers {
        builder.register_tool_without_spec(handler);
    }

    if config.search_tool && config.namespace_tools && !deferred_search_infos.is_empty() {
        builder.register_handler(Arc::new(ToolSearchHandler::new(deferred_search_infos)));
    }

    for executor in params.extension_tool_executors.iter().cloned() {
        builder.register_extension_tool_executor(executor);
    }

    builder
}

fn build_code_mode_handlers(
    config: &ToolsConfig,
    handlers: &[Arc<dyn RegisteredTool>],
    extension_tool_executors: &[Arc<dyn ExtensionToolExecutor>],
    deferred_tools_available: bool,
) -> Vec<Arc<dyn RegisteredTool>> {
    if !config.code_mode_enabled {
        return vec![];
    }

    let mut code_mode_nested_tool_specs = handlers
        .iter()
        .filter_map(|handler| handler.spec())
        .collect::<Vec<_>>();
    code_mode_nested_tool_specs.extend(
        extension_tool_executors
            .iter()
            .filter_map(|executor| executor.spec()),
    );
    let namespace_descriptions = code_mode_namespace_descriptions(&code_mode_nested_tool_specs);
    let mut enabled_tools =
        collect_code_mode_exec_prompt_tool_definitions(code_mode_nested_tool_specs.iter());
    enabled_tools
        .sort_by(|left, right| compare_code_mode_tools(left, right, &namespace_descriptions));

    vec![
        Arc::new(CodeModeExecuteHandler::new(
            create_code_mode_tool(
                &enabled_tools,
                &namespace_descriptions,
                config.code_mode_only_enabled,
                deferred_tools_available,
            ),
            code_mode_nested_tool_specs,
        )),
        Arc::new(CodeModeWaitHandler),
    ]
}

fn merge_into_namespaces(specs: Vec<ToolSpec>) -> Vec<ToolSpec> {
    let mut merged_specs = Vec::with_capacity(specs.len());
    let mut namespace_indices = BTreeMap::<String, usize>::new();
    for spec in specs {
        match spec {
            ToolSpec::Namespace(mut namespace) => {
                if let Some(index) = namespace_indices.get(&namespace.name).copied() {
                    let ToolSpec::Namespace(existing_namespace) = &mut merged_specs[index] else {
                        unreachable!("namespace index must point to a namespace spec");
                    };
                    if existing_namespace.description.trim().is_empty()
                        && !namespace.description.trim().is_empty()
                    {
                        existing_namespace.description = namespace.description;
                    }
                    existing_namespace.tools.append(&mut namespace.tools);
                    continue;
                }

                namespace_indices.insert(namespace.name.clone(), merged_specs.len());
                merged_specs.push(ToolSpec::Namespace(namespace));
            }
            spec => merged_specs.push(spec),
        }
    }

    for spec in &mut merged_specs {
        let ToolSpec::Namespace(namespace) = spec else {
            continue;
        };

        namespace.tools.sort_by(|left, right| match (left, right) {
            (
                ResponsesApiNamespaceTool::Function(left),
                ResponsesApiNamespaceTool::Function(right),
            ) => left.name.cmp(&right.name),
        });

        if namespace.description.trim().is_empty() {
            namespace.description = default_namespace_description(&namespace.name);
        }
    }

    merged_specs
}

fn code_mode_namespace_descriptions(
    specs: &[ToolSpec],
) -> BTreeMap<String, codex_code_mode::ToolNamespaceDescription> {
    let mut namespace_descriptions = BTreeMap::new();
    for spec in specs {
        let ToolSpec::Namespace(namespace) = spec else {
            continue;
        };

        let entry = namespace_descriptions
            .entry(namespace.name.clone())
            .or_insert_with(|| codex_code_mode::ToolNamespaceDescription {
                name: namespace.name.clone(),
                description: namespace.description.clone(),
            });
        if entry.description.trim().is_empty() && !namespace.description.trim().is_empty() {
            entry.description = namespace.description.clone();
        }
    }
    namespace_descriptions
}

fn collect_handler_tools(
    config: &ToolsConfig,
    params: ToolRegistryBuildParams<'_>,
) -> Vec<Arc<dyn RegisteredTool>> {
    let exec_permission_approvals_enabled = config.exec_permission_approvals_enabled;
    let mut handlers = Vec::<Arc<dyn RegisteredTool>>::new();

    if config.environment_mode.has_environment() {
        let include_environment_id =
            matches!(config.environment_mode, ToolEnvironmentMode::Multiple);
        match &config.shell_type {
            ConfigShellToolType::UnifiedExec => {
                handlers.push(Arc::new(ExecCommandHandler::new(
                    ExecCommandHandlerOptions {
                        allow_login_shell: config.allow_login_shell,
                        exec_permission_approvals_enabled,
                        include_environment_id,
                    },
                )));
                handlers.push(Arc::new(WriteStdinHandler));
            }
            ConfigShellToolType::Disabled => {}
            ConfigShellToolType::Default
            | ConfigShellToolType::Local
            | ConfigShellToolType::ShellCommand => {
                handlers.push(Arc::new(ShellCommandHandler::new(
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
            ConfigShellToolType::UnifiedExec => {
                handlers.push(Arc::new(ShellCommandHandler::from(
                    config.shell_command_backend,
                )));
            }
            ConfigShellToolType::Default
            | ConfigShellToolType::Local
            | ConfigShellToolType::ShellCommand
            | ConfigShellToolType::Disabled => {}
        }
    }

    if params.mcp_tools.is_some() {
        handlers.push(Arc::new(ListMcpResourcesHandler));
        handlers.push(Arc::new(ListMcpResourceTemplatesHandler));
        handlers.push(Arc::new(ReadMcpResourceHandler));
    }

    handlers.push(Arc::new(PlanHandler));
    if config.goal_tools {
        handlers.push(Arc::new(GetGoalHandler));
        handlers.push(Arc::new(CreateGoalHandler));
        handlers.push(Arc::new(UpdateGoalHandler));
    }

    handlers.push(Arc::new(RequestUserInputHandler {
        available_modes: config.request_user_input_available_modes.clone(),
    }));

    if config.request_permissions_tool_enabled {
        handlers.push(Arc::new(RequestPermissionsHandler));
    }

    if config.tool_suggest
        && let Some(discoverable_tools) =
            params.discoverable_tools.filter(|tools| !tools.is_empty())
    {
        handlers.push(Arc::new(RequestPluginInstallHandler::new(
            discoverable_tools,
        )));
    }

    if config.environment_mode.has_environment() && config.apply_patch_tool_type.is_some() {
        let include_environment_id =
            matches!(config.environment_mode, ToolEnvironmentMode::Multiple);
        handlers.push(Arc::new(ApplyPatchHandler::new(include_environment_id)));
    }

    if config
        .experimental_supported_tools
        .iter()
        .any(|tool| tool == "test_sync_tool")
    {
        handlers.push(Arc::new(TestSyncHandler));
    }

    if config.environment_mode.has_environment() {
        let include_environment_id =
            matches!(config.environment_mode, ToolEnvironmentMode::Multiple);
        handlers.push(Arc::new(ViewImageHandler::new(ViewImageToolOptions {
            can_request_original_image_detail: config.can_request_original_image_detail,
            include_environment_id,
        })));
    }

    if config.collab_tools {
        if config.multi_agent_v2 {
            let agent_type_description =
                agent_type_description(config, params.default_agent_type_description);
            handlers.push(Arc::new(SpawnAgentHandlerV2::new(SpawnAgentToolOptions {
                available_models: config.available_models.clone(),
                agent_type_description,
                hide_agent_type_model_reasoning: config.hide_spawn_agent_metadata,
                include_usage_hint: config.spawn_agent_usage_hint,
                usage_hint_text: config.spawn_agent_usage_hint_text.clone(),
                max_concurrent_threads_per_session: config.max_concurrent_threads_per_session,
            })));
            handlers.push(Arc::new(SendMessageHandlerV2));
            handlers.push(Arc::new(FollowupTaskHandlerV2));
            handlers.push(Arc::new(WaitAgentHandlerV2::new(
                params.wait_agent_timeouts,
            )));
            handlers.push(Arc::new(CloseAgentHandlerV2));
            handlers.push(Arc::new(ListAgentsHandlerV2));
        } else {
            let agent_type_description =
                agent_type_description(config, params.default_agent_type_description);
            handlers.push(Arc::new(SpawnAgentHandler::new(SpawnAgentToolOptions {
                available_models: config.available_models.clone(),
                agent_type_description,
                hide_agent_type_model_reasoning: config.hide_spawn_agent_metadata,
                include_usage_hint: config.spawn_agent_usage_hint,
                usage_hint_text: config.spawn_agent_usage_hint_text.clone(),
                max_concurrent_threads_per_session: config.max_concurrent_threads_per_session,
            })));
            handlers.push(Arc::new(SendInputHandler));
            handlers.push(Arc::new(ResumeAgentHandler));
            handlers.push(Arc::new(WaitAgentHandler::new(params.wait_agent_timeouts)));
            handlers.push(Arc::new(CloseAgentHandler));
        }
    }

    if config.agent_jobs_tools {
        handlers.push(Arc::new(SpawnAgentsOnCsvHandler));
        if config.agent_jobs_worker_tools {
            handlers.push(Arc::new(ReportAgentJobResultHandler));
        }
    }

    if let Some(mcp_tools) = params.mcp_tools {
        for tool in mcp_tools {
            handlers.push(Arc::new(McpHandler::new(tool.clone())));
        }
    }

    if let Some(deferred_mcp_tools) = params.deferred_mcp_tools {
        for tool in deferred_mcp_tools {
            handlers.push(Arc::new(McpHandler::with_exposure(
                tool.clone(),
                ToolExposure::Deferred,
            )));
        }
    }

    for tool in params.dynamic_tools {
        let Some(handler) = DynamicToolHandler::new(tool).map(Arc::new) else {
            tracing::error!(
                "Failed to convert dynamic tool {:?} to OpenAI tool",
                tool.name
            );
            continue;
        };

        handlers.push(handler);
    }

    handlers
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
