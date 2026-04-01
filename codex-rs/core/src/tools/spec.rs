use crate::client_common::tools::ToolSpec;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::tools::code_mode::PUBLIC_TOOL_NAME;
use crate::tools::code_mode::WAIT_TOOL_NAME;
use crate::tools::handlers::PLAN_TOOL;
use crate::tools::handlers::TOOL_SEARCH_DEFAULT_LIMIT;
use crate::tools::handlers::TOOL_SEARCH_TOOL_NAME;
use crate::tools::handlers::TOOL_SUGGEST_TOOL_NAME;
use crate::tools::handlers::agent_jobs::BatchJobHandler;
use crate::tools::handlers::apply_patch::create_apply_patch_freeform_tool;
use crate::tools::handlers::apply_patch::create_apply_patch_json_tool;
use crate::tools::handlers::multi_agents_common::DEFAULT_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MAX_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MIN_WAIT_TIMEOUT_MS;
use crate::tools::handlers::request_permissions_tool_description;
use crate::tools::handlers::request_user_input_tool_description;
use crate::tools::registry::ToolRegistryBuilder;
use crate::tools::registry::tool_handler_key;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::WebSearchToolType;
use codex_tools::CommandToolOptions;
use codex_tools::DiscoverableTool;
use codex_tools::DiscoverableToolType;
use codex_tools::ShellToolOptions;
use codex_tools::SpawnAgentToolOptions;
use codex_tools::ToolSearchAppInfo;
use codex_tools::ToolSuggestEntry;
use codex_tools::ToolUserShellType;
use codex_tools::ViewImageToolOptions;
use codex_tools::WaitAgentTimeoutOptions;
use codex_tools::augment_tool_spec_for_code_mode;
use codex_tools::create_assign_task_tool;
use codex_tools::create_close_agent_tool_v1;
use codex_tools::create_close_agent_tool_v2;
use codex_tools::create_code_mode_tool;
use codex_tools::create_exec_command_tool;
use codex_tools::create_js_repl_reset_tool;
use codex_tools::create_js_repl_tool;
use codex_tools::create_list_agents_tool;
use codex_tools::create_list_dir_tool;
use codex_tools::create_list_mcp_resource_templates_tool;
use codex_tools::create_list_mcp_resources_tool;
use codex_tools::create_read_mcp_resource_tool;
use codex_tools::create_report_agent_job_result_tool;
use codex_tools::create_request_permissions_tool;
use codex_tools::create_request_user_input_tool;
use codex_tools::create_resume_agent_tool;
use codex_tools::create_send_input_tool_v1;
use codex_tools::create_send_message_tool;
use codex_tools::create_shell_command_tool;
use codex_tools::create_shell_tool;
use codex_tools::create_spawn_agent_tool_v1;
use codex_tools::create_spawn_agent_tool_v2;
use codex_tools::create_spawn_agents_on_csv_tool;
use codex_tools::create_test_sync_tool;
use codex_tools::create_tool_search_tool;
use codex_tools::create_tool_suggest_tool;
use codex_tools::create_view_image_tool;
use codex_tools::create_wait_agent_tool_v1;
use codex_tools::create_wait_agent_tool_v2;
use codex_tools::create_wait_tool;
use codex_tools::create_write_stdin_tool;
use codex_tools::dynamic_tool_to_responses_api_tool;
use codex_tools::mcp_tool_to_responses_api_tool;
use codex_tools::tool_spec_to_code_mode_tool_definition;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

pub type JsonSchema = codex_tools::JsonSchema;
pub use codex_tools::ShellCommandBackendConfig;
pub use codex_tools::ToolsConfig;
pub use codex_tools::ToolsConfigParams;
pub use codex_tools::UnifiedExecShellMode;
pub use codex_tools::ZshForkConfig;

#[cfg(test)]
pub(crate) use codex_tools::mcp_call_tool_result_output_schema;

const WEB_SEARCH_CONTENT_TYPES: [&str; 2] = ["text", "image"];

pub(crate) fn tool_user_shell_type(user_shell: &Shell) -> ToolUserShellType {
    match user_shell.shell_type {
        ShellType::Zsh => ToolUserShellType::Zsh,
        ShellType::Bash => ToolUserShellType::Bash,
        ShellType::PowerShell => ToolUserShellType::PowerShell,
        ShellType::Sh => ToolUserShellType::Sh,
        ShellType::Cmd => ToolUserShellType::Cmd,
    }
}

fn agent_type_description(config: &ToolsConfig) -> String {
    if config.agent_type_description.is_empty() {
        crate::agent::role::spawn_tool_spec::build(&std::collections::BTreeMap::new())
    } else {
        config.agent_type_description.clone()
    }
}

/// TODO(dylan): deprecate once we get rid of json tool
#[derive(Serialize, Deserialize)]
pub(crate) struct ApplyPatchToolArgs {
    pub(crate) input: String,
}

fn push_tool_spec(
    builder: &mut ToolRegistryBuilder,
    spec: ToolSpec,
    supports_parallel_tool_calls: bool,
    code_mode_enabled: bool,
) {
    let spec = if code_mode_enabled {
        augment_tool_spec_for_code_mode(spec)
    } else {
        spec
    };
    if supports_parallel_tool_calls {
        builder.push_spec_with_parallel_support(spec, /*supports_parallel_tool_calls*/ true);
    } else {
        builder.push_spec(spec);
    }
}

/// Builds the tool registry builder while collecting tool specs for later serialization.
#[cfg(test)]
pub(crate) fn build_specs(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    app_tools: Option<HashMap<String, ToolInfo>>,
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    build_specs_with_discoverable_tools(
        config,
        mcp_tools,
        app_tools,
        /*discoverable_tools*/ None,
        dynamic_tools,
    )
}

pub(crate) fn build_specs_with_discoverable_tools(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    app_tools: Option<HashMap<String, ToolInfo>>,
    discoverable_tools: Option<Vec<DiscoverableTool>>,
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    use crate::tools::handlers::ApplyPatchHandler;
    use crate::tools::handlers::CodeModeExecuteHandler;
    use crate::tools::handlers::CodeModeWaitHandler;
    use crate::tools::handlers::DynamicToolHandler;
    use crate::tools::handlers::JsReplHandler;
    use crate::tools::handlers::JsReplResetHandler;
    use crate::tools::handlers::ListDirHandler;
    use crate::tools::handlers::McpHandler;
    use crate::tools::handlers::McpResourceHandler;
    use crate::tools::handlers::PlanHandler;
    use crate::tools::handlers::RequestPermissionsHandler;
    use crate::tools::handlers::RequestUserInputHandler;
    use crate::tools::handlers::ShellCommandHandler;
    use crate::tools::handlers::ShellHandler;
    use crate::tools::handlers::TestSyncHandler;
    use crate::tools::handlers::ToolSearchHandler;
    use crate::tools::handlers::ToolSuggestHandler;
    use crate::tools::handlers::UnifiedExecHandler;
    use crate::tools::handlers::ViewImageHandler;
    use crate::tools::handlers::multi_agents::CloseAgentHandler;
    use crate::tools::handlers::multi_agents::ResumeAgentHandler;
    use crate::tools::handlers::multi_agents::SendInputHandler;
    use crate::tools::handlers::multi_agents::SpawnAgentHandler;
    use crate::tools::handlers::multi_agents::WaitAgentHandler;
    use crate::tools::handlers::multi_agents_v2::AssignTaskHandler as AssignTaskHandlerV2;
    use crate::tools::handlers::multi_agents_v2::CloseAgentHandler as CloseAgentHandlerV2;
    use crate::tools::handlers::multi_agents_v2::ListAgentsHandler as ListAgentsHandlerV2;
    use crate::tools::handlers::multi_agents_v2::SendMessageHandler as SendMessageHandlerV2;
    use crate::tools::handlers::multi_agents_v2::SpawnAgentHandler as SpawnAgentHandlerV2;
    use crate::tools::handlers::multi_agents_v2::WaitAgentHandler as WaitAgentHandlerV2;
    use std::sync::Arc;

    let mut builder = ToolRegistryBuilder::new();

    let shell_handler = Arc::new(ShellHandler);
    let unified_exec_handler = Arc::new(UnifiedExecHandler);
    let plan_handler = Arc::new(PlanHandler);
    let apply_patch_handler = Arc::new(ApplyPatchHandler);
    let dynamic_tool_handler = Arc::new(DynamicToolHandler);
    let view_image_handler = Arc::new(ViewImageHandler);
    let mcp_handler = Arc::new(McpHandler);
    let mcp_resource_handler = Arc::new(McpResourceHandler);
    let shell_command_handler = Arc::new(ShellCommandHandler::from(config.shell_command_backend));
    let request_permissions_handler = Arc::new(RequestPermissionsHandler);
    let request_user_input_handler = Arc::new(RequestUserInputHandler {
        default_mode_request_user_input: config.default_mode_request_user_input,
    });
    let tool_suggest_handler = Arc::new(ToolSuggestHandler);
    let code_mode_handler = Arc::new(CodeModeExecuteHandler);
    let code_mode_wait_handler = Arc::new(CodeModeWaitHandler);
    let js_repl_handler = Arc::new(JsReplHandler);
    let js_repl_reset_handler = Arc::new(JsReplResetHandler);
    let exec_permission_approvals_enabled = config.exec_permission_approvals_enabled;

    if config.code_mode_enabled {
        let nested_config = config.for_code_mode_nested_tools();
        let (nested_specs, _) = build_specs_with_discoverable_tools(
            &nested_config,
            mcp_tools.clone(),
            app_tools.clone(),
            /*discoverable_tools*/ None,
            dynamic_tools,
        )
        .build();
        let mut enabled_tools = nested_specs
            .into_iter()
            .filter_map(|spec| tool_spec_to_code_mode_tool_definition(&spec.spec))
            .map(|tool| (tool.name, tool.description))
            .collect::<Vec<_>>();
        enabled_tools.sort_by(|left, right| left.0.cmp(&right.0));
        enabled_tools.dedup_by(|left, right| left.0 == right.0);
        push_tool_spec(
            &mut builder,
            create_code_mode_tool(&enabled_tools, config.code_mode_only_enabled),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler(PUBLIC_TOOL_NAME, code_mode_handler);
        push_tool_spec(
            &mut builder,
            create_wait_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler(WAIT_TOOL_NAME, code_mode_wait_handler);
    }

    match &config.shell_type {
        ConfigShellToolType::Default => {
            push_tool_spec(
                &mut builder,
                create_shell_tool(ShellToolOptions {
                    exec_permission_approvals_enabled,
                }),
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
        }
        ConfigShellToolType::Local => {
            push_tool_spec(
                &mut builder,
                ToolSpec::LocalShell {},
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
        }
        ConfigShellToolType::UnifiedExec => {
            push_tool_spec(
                &mut builder,
                create_exec_command_tool(CommandToolOptions {
                    allow_login_shell: config.allow_login_shell,
                    exec_permission_approvals_enabled,
                }),
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_write_stdin_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            builder.register_handler("exec_command", unified_exec_handler.clone());
            builder.register_handler("write_stdin", unified_exec_handler);
        }
        ConfigShellToolType::Disabled => {
            // Do nothing.
        }
        ConfigShellToolType::ShellCommand => {
            push_tool_spec(
                &mut builder,
                create_shell_command_tool(CommandToolOptions {
                    allow_login_shell: config.allow_login_shell,
                    exec_permission_approvals_enabled,
                }),
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
        }
    }

    if config.shell_type != ConfigShellToolType::Disabled {
        // Always register shell aliases so older prompts remain compatible.
        builder.register_handler("shell", shell_handler.clone());
        builder.register_handler("container.exec", shell_handler.clone());
        builder.register_handler("local_shell", shell_handler);
        builder.register_handler("shell_command", shell_command_handler);
    }

    if mcp_tools.is_some() {
        push_tool_spec(
            &mut builder,
            create_list_mcp_resources_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_list_mcp_resource_templates_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_read_mcp_resource_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        builder.register_handler("list_mcp_resources", mcp_resource_handler.clone());
        builder.register_handler("list_mcp_resource_templates", mcp_resource_handler.clone());
        builder.register_handler("read_mcp_resource", mcp_resource_handler);
    }

    push_tool_spec(
        &mut builder,
        PLAN_TOOL.clone(),
        /*supports_parallel_tool_calls*/ false,
        config.code_mode_enabled,
    );
    builder.register_handler("update_plan", plan_handler);

    if config.js_repl_enabled {
        push_tool_spec(
            &mut builder,
            create_js_repl_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        push_tool_spec(
            &mut builder,
            create_js_repl_reset_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("js_repl", js_repl_handler);
        builder.register_handler("js_repl_reset", js_repl_reset_handler);
    }

    if config.request_user_input {
        push_tool_spec(
            &mut builder,
            create_request_user_input_tool(request_user_input_tool_description(
                config.default_mode_request_user_input,
            )),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("request_user_input", request_user_input_handler);
    }

    if config.request_permissions_tool_enabled {
        push_tool_spec(
            &mut builder,
            create_request_permissions_tool(request_permissions_tool_description()),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("request_permissions", request_permissions_handler);
    }

    if config.search_tool
        && let Some(app_tools) = app_tools
    {
        let search_tool_handler = Arc::new(ToolSearchHandler::new(app_tools.clone()));
        push_tool_spec(
            &mut builder,
            create_tool_search_tool(
                &tool_search_app_infos(&app_tools),
                TOOL_SEARCH_DEFAULT_LIMIT,
            ),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        builder.register_handler(TOOL_SEARCH_TOOL_NAME, search_tool_handler);

        for tool in app_tools.values() {
            let alias_name =
                tool_handler_key(tool.tool_name.as_str(), Some(tool.tool_namespace.as_str()));

            builder.register_handler(alias_name, mcp_handler.clone());
        }
    }

    if config.tool_suggest
        && let Some(discoverable_tools) = discoverable_tools
            .as_ref()
            .filter(|tools| !tools.is_empty())
    {
        builder.push_spec_with_parallel_support(
            create_tool_suggest_tool(&tool_suggest_entries(discoverable_tools)),
            /*supports_parallel_tool_calls*/ true,
        );
        builder.register_handler(TOOL_SUGGEST_TOOL_NAME, tool_suggest_handler);
    }

    if let Some(apply_patch_tool_type) = &config.apply_patch_tool_type {
        match apply_patch_tool_type {
            ApplyPatchToolType::Freeform => {
                push_tool_spec(
                    &mut builder,
                    create_apply_patch_freeform_tool(),
                    /*supports_parallel_tool_calls*/ false,
                    config.code_mode_enabled,
                );
            }
            ApplyPatchToolType::Function => {
                push_tool_spec(
                    &mut builder,
                    create_apply_patch_json_tool(),
                    /*supports_parallel_tool_calls*/ false,
                    config.code_mode_enabled,
                );
            }
        }
        builder.register_handler("apply_patch", apply_patch_handler);
    }

    if config
        .experimental_supported_tools
        .iter()
        .any(|tool| tool == "list_dir")
    {
        let list_dir_handler = Arc::new(ListDirHandler);
        push_tool_spec(
            &mut builder,
            create_list_dir_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        builder.register_handler("list_dir", list_dir_handler);
    }

    if config
        .experimental_supported_tools
        .contains(&"test_sync_tool".to_string())
    {
        let test_sync_handler = Arc::new(TestSyncHandler);
        push_tool_spec(
            &mut builder,
            create_test_sync_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        builder.register_handler("test_sync_tool", test_sync_handler);
    }

    let external_web_access = match config.web_search_mode {
        Some(WebSearchMode::Cached) => Some(false),
        Some(WebSearchMode::Live) => Some(true),
        Some(WebSearchMode::Disabled) | None => None,
    };

    if let Some(external_web_access) = external_web_access {
        let search_content_types = match config.web_search_tool_type {
            WebSearchToolType::Text => None,
            WebSearchToolType::TextAndImage => Some(
                WEB_SEARCH_CONTENT_TYPES
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            ),
        };

        push_tool_spec(
            &mut builder,
            ToolSpec::WebSearch {
                external_web_access: Some(external_web_access),
                filters: config
                    .web_search_config
                    .as_ref()
                    .and_then(|cfg| cfg.filters.clone().map(Into::into)),
                user_location: config
                    .web_search_config
                    .as_ref()
                    .and_then(|cfg| cfg.user_location.clone().map(Into::into)),
                search_context_size: config
                    .web_search_config
                    .as_ref()
                    .and_then(|cfg| cfg.search_context_size),
                search_content_types,
            },
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
    }

    if config.image_gen_tool {
        push_tool_spec(
            &mut builder,
            ToolSpec::ImageGeneration {
                output_format: "png".to_string(),
            },
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
    }

    push_tool_spec(
        &mut builder,
        create_view_image_tool(ViewImageToolOptions {
            can_request_original_image_detail: config.can_request_original_image_detail,
        }),
        /*supports_parallel_tool_calls*/ true,
        config.code_mode_enabled,
    );
    builder.register_handler("view_image", view_image_handler);

    if config.collab_tools {
        if config.multi_agent_v2 {
            let agent_type_description = agent_type_description(config);
            push_tool_spec(
                &mut builder,
                create_spawn_agent_tool_v2(SpawnAgentToolOptions {
                    available_models: &config.available_models,
                    agent_type_description,
                }),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_send_message_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_assign_task_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_wait_agent_tool_v2(WaitAgentTimeoutOptions {
                    default_timeout_ms: DEFAULT_WAIT_TIMEOUT_MS,
                    min_timeout_ms: MIN_WAIT_TIMEOUT_MS,
                    max_timeout_ms: MAX_WAIT_TIMEOUT_MS,
                }),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_close_agent_tool_v2(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_list_agents_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            builder.register_handler("spawn_agent", Arc::new(SpawnAgentHandlerV2));
            builder.register_handler("send_message", Arc::new(SendMessageHandlerV2));
            builder.register_handler("assign_task", Arc::new(AssignTaskHandlerV2));
            builder.register_handler("wait_agent", Arc::new(WaitAgentHandlerV2));
            builder.register_handler("close_agent", Arc::new(CloseAgentHandlerV2));
            builder.register_handler("list_agents", Arc::new(ListAgentsHandlerV2));
        } else {
            let agent_type_description = agent_type_description(config);
            push_tool_spec(
                &mut builder,
                create_spawn_agent_tool_v1(SpawnAgentToolOptions {
                    available_models: &config.available_models,
                    agent_type_description,
                }),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_send_input_tool_v1(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_resume_agent_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            builder.register_handler("resume_agent", Arc::new(ResumeAgentHandler));
            push_tool_spec(
                &mut builder,
                create_wait_agent_tool_v1(WaitAgentTimeoutOptions {
                    default_timeout_ms: DEFAULT_WAIT_TIMEOUT_MS,
                    min_timeout_ms: MIN_WAIT_TIMEOUT_MS,
                    max_timeout_ms: MAX_WAIT_TIMEOUT_MS,
                }),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            push_tool_spec(
                &mut builder,
                create_close_agent_tool_v1(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            builder.register_handler("spawn_agent", Arc::new(SpawnAgentHandler));
            builder.register_handler("send_input", Arc::new(SendInputHandler));
            builder.register_handler("wait_agent", Arc::new(WaitAgentHandler));
            builder.register_handler("close_agent", Arc::new(CloseAgentHandler));
        }
    }

    if config.agent_jobs_tools {
        let agent_jobs_handler = Arc::new(BatchJobHandler);
        push_tool_spec(
            &mut builder,
            create_spawn_agents_on_csv_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        builder.register_handler("spawn_agents_on_csv", agent_jobs_handler.clone());
        if config.agent_jobs_worker_tools {
            push_tool_spec(
                &mut builder,
                create_report_agent_job_result_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            builder.register_handler("report_agent_job_result", agent_jobs_handler);
        }
    }

    if let Some(mcp_tools) = mcp_tools {
        let mut entries: Vec<(String, rmcp::model::Tool)> = mcp_tools.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, tool) in entries.into_iter() {
            match mcp_tool_to_responses_api_tool(name.clone(), &tool) {
                Ok(converted_tool) => {
                    push_tool_spec(
                        &mut builder,
                        ToolSpec::Function(converted_tool),
                        /*supports_parallel_tool_calls*/ false,
                        config.code_mode_enabled,
                    );
                    builder.register_handler(name, mcp_handler.clone());
                }
                Err(e) => {
                    tracing::error!("Failed to convert {name:?} MCP tool to OpenAI tool: {e:?}");
                }
            }
        }
    }

    if !dynamic_tools.is_empty() {
        for tool in dynamic_tools {
            match dynamic_tool_to_responses_api_tool(tool) {
                Ok(converted_tool) => {
                    push_tool_spec(
                        &mut builder,
                        ToolSpec::Function(converted_tool),
                        /*supports_parallel_tool_calls*/ false,
                        config.code_mode_enabled,
                    );
                    builder.register_handler(tool.name.clone(), dynamic_tool_handler.clone());
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to convert dynamic tool {:?} to OpenAI tool: {e:?}",
                        tool.name
                    );
                }
            }
        }
    }

    builder
}

fn tool_search_app_infos(app_tools: &HashMap<String, ToolInfo>) -> Vec<ToolSearchAppInfo> {
    app_tools
        .values()
        .filter(|tool| tool.server_name == CODEX_APPS_MCP_SERVER_NAME)
        .filter_map(|tool| {
            let name = tool
                .connector_name
                .as_deref()
                .map(str::trim)
                .filter(|connector_name| !connector_name.is_empty())?
                .to_string();
            let description = tool
                .connector_description
                .as_deref()
                .map(str::trim)
                .filter(|connector_description| !connector_description.is_empty())
                .map(str::to_string);
            Some(ToolSearchAppInfo { name, description })
        })
        .collect()
}

fn tool_suggest_entries(discoverable_tools: &[DiscoverableTool]) -> Vec<ToolSuggestEntry> {
    discoverable_tools
        .iter()
        .map(|tool| match tool {
            DiscoverableTool::Connector(connector) => ToolSuggestEntry {
                id: connector.id.clone(),
                name: connector.name.clone(),
                description: connector.description.clone(),
                tool_type: DiscoverableToolType::Connector,
                has_skills: false,
                mcp_server_names: Vec::new(),
                app_connector_ids: Vec::new(),
            },
            DiscoverableTool::Plugin(plugin) => ToolSuggestEntry {
                id: plugin.id.clone(),
                name: plugin.name.clone(),
                description: plugin.description.clone(),
                tool_type: DiscoverableToolType::Plugin,
                has_skills: plugin.has_skills,
                mcp_server_names: plugin.mcp_server_names.clone(),
                app_connector_ids: plugin.app_connector_ids.clone(),
            },
        })
        .collect()
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
