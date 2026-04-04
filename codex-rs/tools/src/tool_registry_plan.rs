use crate::CommandToolOptions;
use crate::REQUEST_USER_INPUT_TOOL_NAME;
use crate::ShellToolOptions;
use crate::SpawnAgentToolOptions;
use crate::TOOL_SEARCH_DEFAULT_LIMIT;
use crate::TOOL_SEARCH_TOOL_NAME;
use crate::TOOL_SUGGEST_TOOL_NAME;
use crate::ToolHandlerKind;
use crate::ToolRegistryPlan;
use crate::ToolRegistryPlanParams;
use crate::ToolSearchAppSource;
use crate::ToolSpec;
use crate::ToolsConfig;
use crate::ViewImageToolOptions;
use crate::WebSearchToolOptions;
use crate::collect_code_mode_tool_definitions;
use crate::collect_tool_search_app_infos;
use crate::collect_tool_suggest_entries;
use crate::create_apply_patch_freeform_tool;
use crate::create_apply_patch_json_tool;
use crate::create_assign_task_tool;
use crate::create_close_agent_tool_v1;
use crate::create_close_agent_tool_v2;
use crate::create_code_mode_tool;
use crate::create_exec_command_tool;
use crate::create_image_generation_tool;
use crate::create_js_repl_reset_tool;
use crate::create_js_repl_tool;
use crate::create_list_agents_tool;
use crate::create_list_dir_tool;
use crate::create_list_mcp_resource_templates_tool;
use crate::create_list_mcp_resources_tool;
use crate::create_local_shell_tool;
use crate::create_read_mcp_resource_tool;
use crate::create_report_agent_job_result_tool;
use crate::create_request_permissions_tool;
use crate::create_request_user_input_tool;
use crate::create_resume_agent_tool;
use crate::create_send_input_tool_v1;
use crate::create_send_message_tool;
use crate::create_shell_command_tool;
use crate::create_shell_tool;
use crate::create_spawn_agent_tool_v1;
use crate::create_spawn_agent_tool_v2;
use crate::create_spawn_agents_on_csv_tool;
use crate::create_test_sync_tool;
use crate::create_tool_search_tool;
use crate::create_tool_suggest_tool;
use crate::create_update_plan_tool;
use crate::create_view_image_tool;
use crate::create_wait_agent_tool_v1;
use crate::create_wait_agent_tool_v2;
use crate::create_wait_tool;
use crate::create_web_search_tool;
use crate::create_write_stdin_tool;
use crate::dynamic_tool_to_responses_api_tool;
use crate::mcp_tool_to_responses_api_tool;
use crate::request_permissions_tool_description;
use crate::request_user_input_tool_description;
use crate::tool_registry_plan_types::agent_type_description;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use rmcp::model::Tool as McpTool;

pub fn build_tool_registry_plan(
    config: &ToolsConfig,
    params: ToolRegistryPlanParams<'_>,
) -> ToolRegistryPlan {
    let mut plan = ToolRegistryPlan::new();
    let exec_permission_approvals_enabled = config.exec_permission_approvals_enabled;

    if config.code_mode_enabled {
        let nested_config = config.for_code_mode_nested_tools();
        let nested_plan = build_tool_registry_plan(
            &nested_config,
            ToolRegistryPlanParams {
                discoverable_tools: None,
                ..params
            },
        );
        let enabled_tools = collect_code_mode_tool_definitions(
            nested_plan
                .specs
                .iter()
                .map(|configured_tool| &configured_tool.spec),
        )
        .into_iter()
        .map(|tool| (tool.name, tool.description))
        .collect::<Vec<_>>();
        plan.push_spec(
            create_code_mode_tool(&enabled_tools, config.code_mode_only_enabled),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        plan.register_handler(
            codex_code_mode::PUBLIC_TOOL_NAME,
            ToolHandlerKind::CodeModeExecute,
        );
        plan.push_spec(
            create_wait_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        plan.register_handler(
            codex_code_mode::WAIT_TOOL_NAME,
            ToolHandlerKind::CodeModeWait,
        );
    }

    match &config.shell_type {
        ConfigShellToolType::Default => {
            plan.push_spec(
                create_shell_tool(ShellToolOptions {
                    exec_permission_approvals_enabled,
                }),
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
        }
        ConfigShellToolType::Local => {
            plan.push_spec(
                create_local_shell_tool(),
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
        }
        ConfigShellToolType::UnifiedExec => {
            plan.push_spec(
                create_exec_command_tool(CommandToolOptions {
                    allow_login_shell: config.allow_login_shell,
                    exec_permission_approvals_enabled,
                }),
                /*supports_parallel_tool_calls*/ true,
                config.code_mode_enabled,
            );
            plan.push_spec(
                create_write_stdin_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.register_handler("exec_command", ToolHandlerKind::UnifiedExec);
            plan.register_handler("write_stdin", ToolHandlerKind::UnifiedExec);
        }
        ConfigShellToolType::Disabled => {}
        ConfigShellToolType::ShellCommand => {
            plan.push_spec(
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
        plan.register_handler("shell", ToolHandlerKind::Shell);
        plan.register_handler("container.exec", ToolHandlerKind::Shell);
        plan.register_handler("local_shell", ToolHandlerKind::Shell);
        plan.register_handler("shell_command", ToolHandlerKind::ShellCommand);
    }

    if params.mcp_tools.is_some() {
        plan.push_spec(
            create_list_mcp_resources_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        plan.push_spec(
            create_list_mcp_resource_templates_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        plan.push_spec(
            create_read_mcp_resource_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        plan.register_handler("list_mcp_resources", ToolHandlerKind::McpResource);
        plan.register_handler("list_mcp_resource_templates", ToolHandlerKind::McpResource);
        plan.register_handler("read_mcp_resource", ToolHandlerKind::McpResource);
    }

    plan.push_spec(
        create_update_plan_tool(),
        /*supports_parallel_tool_calls*/ false,
        config.code_mode_enabled,
    );
    plan.register_handler("update_plan", ToolHandlerKind::Plan);

    if config.js_repl_enabled {
        plan.push_spec(
            create_js_repl_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        plan.push_spec(
            create_js_repl_reset_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        plan.register_handler("js_repl", ToolHandlerKind::JsRepl);
        plan.register_handler("js_repl_reset", ToolHandlerKind::JsReplReset);
    }

    if config.request_user_input {
        plan.push_spec(
            create_request_user_input_tool(request_user_input_tool_description(
                config.default_mode_request_user_input,
            )),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        plan.register_handler(
            REQUEST_USER_INPUT_TOOL_NAME,
            ToolHandlerKind::RequestUserInput,
        );
    }

    if config.request_permissions_tool_enabled {
        plan.push_spec(
            create_request_permissions_tool(request_permissions_tool_description()),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        plan.register_handler("request_permissions", ToolHandlerKind::RequestPermissions);
    }

    if config.search_tool
        && let Some(app_tools) = params.app_tools
    {
        let search_app_infos = collect_tool_search_app_infos(
            app_tools.iter().map(|tool| ToolSearchAppSource {
                server_name: tool.server_name,
                connector_name: tool.connector_name,
                connector_description: tool.connector_description,
            }),
            params.codex_apps_mcp_server_name,
        );
        plan.push_spec(
            create_tool_search_tool(&search_app_infos, TOOL_SEARCH_DEFAULT_LIMIT),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        plan.register_handler(TOOL_SEARCH_TOOL_NAME, ToolHandlerKind::ToolSearch);

        for tool in app_tools {
            plan.register_handler(
                format!("{}:{}", tool.tool_namespace, tool.tool_name),
                ToolHandlerKind::Mcp,
            );
        }
    }

    if config.tool_suggest
        && let Some(discoverable_tools) =
            params.discoverable_tools.filter(|tools| !tools.is_empty())
    {
        plan.push_spec(
            create_tool_suggest_tool(&collect_tool_suggest_entries(discoverable_tools)),
            /*supports_parallel_tool_calls*/ true,
            /*code_mode_enabled*/ false,
        );
        plan.register_handler(TOOL_SUGGEST_TOOL_NAME, ToolHandlerKind::ToolSuggest);
    }

    if let Some(apply_patch_tool_type) = &config.apply_patch_tool_type {
        match apply_patch_tool_type {
            ApplyPatchToolType::Freeform => {
                plan.push_spec(
                    create_apply_patch_freeform_tool(),
                    /*supports_parallel_tool_calls*/ false,
                    config.code_mode_enabled,
                );
            }
            ApplyPatchToolType::Function => {
                plan.push_spec(
                    create_apply_patch_json_tool(),
                    /*supports_parallel_tool_calls*/ false,
                    config.code_mode_enabled,
                );
            }
        }
        plan.register_handler("apply_patch", ToolHandlerKind::ApplyPatch);
    }

    if config
        .experimental_supported_tools
        .iter()
        .any(|tool| tool == "list_dir")
    {
        plan.push_spec(
            create_list_dir_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        plan.register_handler("list_dir", ToolHandlerKind::ListDir);
    }

    if config
        .experimental_supported_tools
        .iter()
        .any(|tool| tool == "test_sync_tool")
    {
        plan.push_spec(
            create_test_sync_tool(),
            /*supports_parallel_tool_calls*/ true,
            config.code_mode_enabled,
        );
        plan.register_handler("test_sync_tool", ToolHandlerKind::TestSync);
    }

    if let Some(web_search_tool) = create_web_search_tool(WebSearchToolOptions {
        web_search_mode: config.web_search_mode,
        web_search_config: config.web_search_config.as_ref(),
        web_search_tool_type: config.web_search_tool_type,
    }) {
        plan.push_spec(
            web_search_tool,
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
    }

    if config.image_gen_tool {
        plan.push_spec(
            create_image_generation_tool("png"),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
    }

    plan.push_spec(
        create_view_image_tool(ViewImageToolOptions {
            can_request_original_image_detail: config.can_request_original_image_detail,
        }),
        /*supports_parallel_tool_calls*/ true,
        config.code_mode_enabled,
    );
    plan.register_handler("view_image", ToolHandlerKind::ViewImage);

    if config.collab_tools {
        if config.multi_agent_v2 {
            let agent_type_description =
                agent_type_description(config, params.default_agent_type_description);
            plan.push_spec(
                create_spawn_agent_tool_v2(SpawnAgentToolOptions {
                    available_models: &config.available_models,
                    agent_type_description,
                }),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.push_spec(
                create_send_message_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.push_spec(
                create_assign_task_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.push_spec(
                create_wait_agent_tool_v2(params.wait_agent_timeouts),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.push_spec(
                create_close_agent_tool_v2(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.push_spec(
                create_list_agents_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.register_handler("spawn_agent", ToolHandlerKind::SpawnAgentV2);
            plan.register_handler("send_message", ToolHandlerKind::SendMessageV2);
            plan.register_handler("assign_task", ToolHandlerKind::AssignTaskV2);
            plan.register_handler("wait_agent", ToolHandlerKind::WaitAgentV2);
            plan.register_handler("close_agent", ToolHandlerKind::CloseAgentV2);
            plan.register_handler("list_agents", ToolHandlerKind::ListAgentsV2);
        } else {
            let agent_type_description =
                agent_type_description(config, params.default_agent_type_description);
            plan.push_spec(
                create_spawn_agent_tool_v1(SpawnAgentToolOptions {
                    available_models: &config.available_models,
                    agent_type_description,
                }),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.push_spec(
                create_send_input_tool_v1(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.push_spec(
                create_resume_agent_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.register_handler("resume_agent", ToolHandlerKind::ResumeAgentV1);
            plan.push_spec(
                create_wait_agent_tool_v1(params.wait_agent_timeouts),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.push_spec(
                create_close_agent_tool_v1(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.register_handler("spawn_agent", ToolHandlerKind::SpawnAgentV1);
            plan.register_handler("send_input", ToolHandlerKind::SendInputV1);
            plan.register_handler("wait_agent", ToolHandlerKind::WaitAgentV1);
            plan.register_handler("close_agent", ToolHandlerKind::CloseAgentV1);
        }
    }

    if config.agent_jobs_tools {
        plan.push_spec(
            create_spawn_agents_on_csv_tool(),
            /*supports_parallel_tool_calls*/ false,
            config.code_mode_enabled,
        );
        plan.register_handler("spawn_agents_on_csv", ToolHandlerKind::AgentJobs);
        if config.agent_jobs_worker_tools {
            plan.push_spec(
                create_report_agent_job_result_tool(),
                /*supports_parallel_tool_calls*/ false,
                config.code_mode_enabled,
            );
            plan.register_handler("report_agent_job_result", ToolHandlerKind::AgentJobs);
        }
    }

    if let Some(mcp_tools) = params.mcp_tools {
        let mut entries: Vec<(String, &McpTool)> = mcp_tools
            .iter()
            .map(|(name, tool)| (name.clone(), tool))
            .collect();
        entries.sort_by(|left, right| left.0.cmp(&right.0));

        for (name, tool) in entries {
            match mcp_tool_to_responses_api_tool(name.clone(), tool) {
                Ok(converted_tool) => {
                    plan.push_spec(
                        ToolSpec::Function(converted_tool),
                        /*supports_parallel_tool_calls*/ false,
                        config.code_mode_enabled,
                    );
                    plan.register_handler(name, ToolHandlerKind::Mcp);
                }
                Err(error) => {
                    tracing::error!(
                        "Failed to convert {name:?} MCP tool to OpenAI tool: {error:?}"
                    );
                }
            }
        }
    }

    for tool in params.dynamic_tools {
        match dynamic_tool_to_responses_api_tool(tool) {
            Ok(converted_tool) => {
                plan.push_spec(
                    ToolSpec::Function(converted_tool),
                    /*supports_parallel_tool_calls*/ false,
                    config.code_mode_enabled,
                );
                plan.register_handler(tool.name.clone(), ToolHandlerKind::DynamicTool);
            }
            Err(error) => {
                tracing::error!(
                    "Failed to convert dynamic tool {:?} to OpenAI tool: {error:?}",
                    tool.name
                );
            }
        }
    }

    plan
}

#[cfg(test)]
#[path = "tool_registry_plan_tests.rs"]
mod tests;
