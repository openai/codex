use crate::tools::code_mode::execute_spec::create_code_mode_tool;
use crate::tools::code_mode::wait_spec::create_wait_tool;
use crate::tools::handlers::ApplyPatchHandler;
use crate::tools::handlers::CodeModeExecuteHandler;
use crate::tools::handlers::CodeModeWaitHandler;
use crate::tools::handlers::ContainerExecHandler;
use crate::tools::handlers::CreateGoalHandler;
use crate::tools::handlers::DynamicToolHandler;
use crate::tools::handlers::ExecCommandHandler;
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
use crate::tools::handlers::ShellHandler;
use crate::tools::handlers::TestSyncHandler;
use crate::tools::handlers::ToolSearchHandler;
use crate::tools::handlers::UpdateGoalHandler;
use crate::tools::handlers::ViewImageHandler;
use crate::tools::handlers::WriteStdinHandler;
use crate::tools::handlers::agent_jobs::ReportAgentJobResultHandler;
use crate::tools::handlers::agent_jobs::SpawnAgentsOnCsvHandler;
use crate::tools::handlers::agent_jobs_spec::create_report_agent_job_result_tool;
use crate::tools::handlers::agent_jobs_spec::create_spawn_agents_on_csv_tool;
use crate::tools::handlers::apply_patch_spec::create_apply_patch_freeform_tool;
use crate::tools::handlers::extension_tools::ExtensionToolHandler;
use crate::tools::handlers::goal_spec::create_create_goal_tool;
use crate::tools::handlers::goal_spec::create_get_goal_tool;
use crate::tools::handlers::goal_spec::create_update_goal_tool;
use crate::tools::handlers::mcp_resource_spec::create_list_mcp_resource_templates_tool;
use crate::tools::handlers::mcp_resource_spec::create_list_mcp_resources_tool;
use crate::tools::handlers::mcp_resource_spec::create_read_mcp_resource_tool;
use crate::tools::handlers::multi_agents::CloseAgentHandler;
use crate::tools::handlers::multi_agents::ResumeAgentHandler;
use crate::tools::handlers::multi_agents::SendInputHandler;
use crate::tools::handlers::multi_agents::SpawnAgentHandler;
use crate::tools::handlers::multi_agents::WaitAgentHandler;
use crate::tools::handlers::multi_agents_spec::SpawnAgentToolOptions;
use crate::tools::handlers::multi_agents_spec::create_close_agent_tool_v1;
use crate::tools::handlers::multi_agents_spec::create_close_agent_tool_v2;
use crate::tools::handlers::multi_agents_spec::create_followup_task_tool;
use crate::tools::handlers::multi_agents_spec::create_list_agents_tool;
use crate::tools::handlers::multi_agents_spec::create_resume_agent_tool;
use crate::tools::handlers::multi_agents_spec::create_send_input_tool_v1;
use crate::tools::handlers::multi_agents_spec::create_send_message_tool;
use crate::tools::handlers::multi_agents_spec::create_spawn_agent_tool_v1;
use crate::tools::handlers::multi_agents_spec::create_spawn_agent_tool_v2;
use crate::tools::handlers::multi_agents_spec::create_wait_agent_tool_v1;
use crate::tools::handlers::multi_agents_spec::create_wait_agent_tool_v2;
use crate::tools::handlers::multi_agents_v2::CloseAgentHandler as CloseAgentHandlerV2;
use crate::tools::handlers::multi_agents_v2::FollowupTaskHandler as FollowupTaskHandlerV2;
use crate::tools::handlers::multi_agents_v2::ListAgentsHandler as ListAgentsHandlerV2;
use crate::tools::handlers::multi_agents_v2::SendMessageHandler as SendMessageHandlerV2;
use crate::tools::handlers::multi_agents_v2::SpawnAgentHandler as SpawnAgentHandlerV2;
use crate::tools::handlers::multi_agents_v2::WaitAgentHandler as WaitAgentHandlerV2;
use crate::tools::handlers::plan_spec::create_update_plan_tool;
use crate::tools::handlers::request_plugin_install_spec::create_request_plugin_install_tool;
use crate::tools::handlers::request_user_input_spec::create_request_user_input_tool;
use crate::tools::handlers::request_user_input_spec::request_user_input_tool_description;
use crate::tools::handlers::shell_spec::CommandToolOptions;
use crate::tools::handlers::shell_spec::ShellToolOptions;
use crate::tools::handlers::shell_spec::create_exec_command_tool_with_environment_id;
use crate::tools::handlers::shell_spec::create_local_shell_tool;
use crate::tools::handlers::shell_spec::create_request_permissions_tool;
use crate::tools::handlers::shell_spec::create_shell_command_tool;
use crate::tools::handlers::shell_spec::create_shell_tool;
use crate::tools::handlers::shell_spec::create_write_stdin_tool;
use crate::tools::handlers::shell_spec::request_permissions_tool_description;
use crate::tools::handlers::test_sync_spec::create_test_sync_tool;
use crate::tools::handlers::tool_search_spec::create_tool_search_tool;
use crate::tools::handlers::view_image_spec::ViewImageToolOptions;
use crate::tools::handlers::view_image_spec::create_view_image_tool;
use crate::tools::hosted_spec::WebSearchToolOptions;
use crate::tools::hosted_spec::create_image_generation_tool;
use crate::tools::hosted_spec::create_web_search_tool;
use crate::tools::registry::AnyToolHandler;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolRegistryBuilder;
use crate::tools::spec_plan_types::ToolNamespace;
use crate::tools::spec_plan_types::ToolRegistryBuildParams;
use crate::tools::spec_plan_types::agent_type_description;
use crate::tools::tool_search_entry::ToolSearchEntry;
use crate::tools::tool_search_entry::dynamic_tool_search_entry;
use crate::tools::tool_search_entry::mcp_tool_search_entry;
use codex_mcp::ToolInfo;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_tool_api::ToolDefinition;
use codex_tool_api::ToolExecutor;
use codex_tool_api::ToolExposure;
use codex_tools::ToolEnvironmentMode;
use codex_tools::ToolName;
use codex_tools::ToolSearchSource;
use codex_tools::ToolSearchSourceInfo;
use codex_tools::ToolSpec;
use codex_tools::ToolsConfig;
use codex_tools::collect_code_mode_exec_prompt_tool_definitions;
use codex_tools::collect_request_plugin_install_entries;
use codex_tools::collect_tool_search_source_infos;
use codex_tools::default_namespace_description;
use codex_tools::mcp_tool_definition;
use codex_tools::parse_dynamic_tool;
use codex_tools::tool_definition_to_loadable_tool_spec;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::Arc;

pub fn build_tool_registry_builder(
    config: &ToolsConfig,
    params: ToolRegistryBuildParams<'_>,
) -> ToolRegistryBuilder {
    let mut builder = ToolRegistryBuilder::new(config.code_mode_enabled);
    let exec_permission_approvals_enabled = config.exec_permission_approvals_enabled;
    let mcp_definitions = mcp_tool_definitions(params.mcp_tools);
    let dynamic_definitions = dynamic_tool_definitions(params.dynamic_tools);
    let deferred_mcp_definitions = deferred_mcp_tool_definitions(params.deferred_mcp_tools);
    let tool_search_plan = build_tool_search_plan(
        config,
        params.deferred_mcp_tools,
        &deferred_mcp_definitions,
        params.dynamic_tools,
        &dynamic_definitions,
    );

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
        let mut enabled_tools =
            collect_code_mode_exec_prompt_tool_definitions(nested_builder.specs().iter());
        enabled_tools
            .sort_by(|left, right| compare_code_mode_tools(left, right, &namespace_descriptions));
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(
                CodeModeExecuteHandler,
                create_code_mode_tool(
                    &enabled_tools,
                    &namespace_descriptions,
                    config.code_mode_only_enabled,
                    tool_search_plan.deferred_tools_available(),
                ),
            ),
        );
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(CodeModeWaitHandler, create_wait_tool()),
        );
    }

    if config.environment_mode.has_environment() {
        let include_environment_id =
            matches!(config.environment_mode, ToolEnvironmentMode::Multiple);
        match &config.shell_type {
            ConfigShellToolType::Default => {
                let shell_options = ShellToolOptions {
                    exec_permission_approvals_enabled,
                };
                register_and_publish_tool_definition(
                    &mut builder,
                    config,
                    runtime_tool_definition(ShellHandler::new(), create_shell_tool(shell_options)),
                );
            }
            ConfigShellToolType::Local => {
                register_and_publish_tool_definition(
                    &mut builder,
                    config,
                    runtime_tool_definition(LocalShellHandler::new(), create_local_shell_tool()),
                );
            }
            ConfigShellToolType::UnifiedExec => {
                register_and_publish_tool_definition(
                    &mut builder,
                    config,
                    runtime_tool_definition(
                        ExecCommandHandler,
                        create_exec_command_tool_with_environment_id(
                            CommandToolOptions {
                                allow_login_shell: config.allow_login_shell,
                                exec_permission_approvals_enabled,
                            },
                            include_environment_id,
                        ),
                    ),
                );
                register_and_publish_tool_definition(
                    &mut builder,
                    config,
                    runtime_tool_definition(WriteStdinHandler, create_write_stdin_tool()),
                );
            }
            ConfigShellToolType::Disabled => {}
            ConfigShellToolType::ShellCommand => {
                let shell_command_options = CommandToolOptions {
                    allow_login_shell: config.allow_login_shell,
                    exec_permission_approvals_enabled,
                };
                register_and_publish_tool_definition(
                    &mut builder,
                    config,
                    runtime_tool_definition(
                        ShellCommandHandler::new(config.shell_command_backend),
                        create_shell_command_tool(shell_command_options),
                    ),
                );
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
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(ListMcpResourcesHandler, create_list_mcp_resources_tool()),
        );
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(
                ListMcpResourceTemplatesHandler,
                create_list_mcp_resource_templates_tool(),
            ),
        );
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(ReadMcpResourceHandler, create_read_mcp_resource_tool()),
        );
    }

    register_and_publish_tool_definition(
        &mut builder,
        config,
        runtime_tool_definition(PlanHandler, create_update_plan_tool()),
    );
    if config.goal_tools {
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(GetGoalHandler, create_get_goal_tool()),
        );
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(CreateGoalHandler, create_create_goal_tool()),
        );
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(UpdateGoalHandler, create_update_goal_tool()),
        );
    }

    let available_modes = config.request_user_input_available_modes.clone();
    register_and_publish_tool_definition(
        &mut builder,
        config,
        runtime_tool_definition(
            RequestUserInputHandler {
                available_modes: available_modes.clone(),
            },
            create_request_user_input_tool(request_user_input_tool_description(&available_modes)),
        ),
    );

    if config.request_permissions_tool_enabled {
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(
                RequestPermissionsHandler,
                create_request_permissions_tool(request_permissions_tool_description()),
            ),
        );
    }

    if tool_search_plan.should_register() {
        let ToolSearchPlan {
            entries,
            source_infos,
            ..
        } = tool_search_plan;
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(
                ToolSearchHandler::new(entries),
                create_tool_search_tool(&source_infos, codex_tools::TOOL_SEARCH_DEFAULT_LIMIT),
            ),
        );
    }

    if config.tool_suggest
        && let Some(discoverable_tools) =
            params.discoverable_tools.filter(|tools| !tools.is_empty())
    {
        let discoverable_tool_entries = collect_request_plugin_install_entries(discoverable_tools);
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(
                RequestPluginInstallHandler,
                create_request_plugin_install_tool(&discoverable_tool_entries),
            ),
        );
    }

    if config.environment_mode.has_environment() && config.apply_patch_tool_type.is_some() {
        let include_environment_id =
            matches!(config.environment_mode, ToolEnvironmentMode::Multiple);
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(
                ApplyPatchHandler::new(include_environment_id),
                create_apply_patch_freeform_tool(include_environment_id),
            ),
        );
    }

    if config
        .experimental_supported_tools
        .iter()
        .any(|tool| tool == "test_sync_tool")
    {
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(TestSyncHandler, create_test_sync_tool()),
        );
    }

    if let Some(web_search_tool) = create_web_search_tool(WebSearchToolOptions {
        web_search_mode: config.web_search_mode,
        web_search_config: config.web_search_config.as_ref(),
        web_search_tool_type: config.web_search_tool_type,
    }) {
        builder.push_spec(web_search_tool);
    }

    if config.image_gen_tool {
        builder.push_spec(create_image_generation_tool("png"));
    }

    if config.environment_mode.has_environment() {
        let include_environment_id =
            matches!(config.environment_mode, ToolEnvironmentMode::Multiple);
        let view_image_options = ViewImageToolOptions {
            can_request_original_image_detail: config.can_request_original_image_detail,
            include_environment_id,
        };
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(ViewImageHandler, create_view_image_tool(view_image_options)),
        );
    }

    if config.collab_tools {
        if config.multi_agent_v2 {
            let agent_type_description =
                agent_type_description(config, params.default_agent_type_description);
            let spawn_agent_options = SpawnAgentToolOptions {
                available_models: config.available_models.clone(),
                agent_type_description,
                hide_agent_type_model_reasoning: config.hide_spawn_agent_metadata,
                include_usage_hint: config.spawn_agent_usage_hint,
                usage_hint_text: config.spawn_agent_usage_hint_text.clone(),
                max_concurrent_threads_per_session: config.max_concurrent_threads_per_session,
            };
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(
                    SpawnAgentHandlerV2,
                    create_spawn_agent_tool_v2(spawn_agent_options),
                ),
            );
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(SendMessageHandlerV2, create_send_message_tool()),
            );
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(FollowupTaskHandlerV2, create_followup_task_tool()),
            );
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(
                    WaitAgentHandlerV2,
                    create_wait_agent_tool_v2(params.wait_agent_timeouts),
                ),
            );
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(CloseAgentHandlerV2, create_close_agent_tool_v2()),
            );
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(ListAgentsHandlerV2, create_list_agents_tool()),
            );
        } else {
            let agent_type_description =
                agent_type_description(config, params.default_agent_type_description);
            let spawn_agent_options = SpawnAgentToolOptions {
                available_models: config.available_models.clone(),
                agent_type_description,
                hide_agent_type_model_reasoning: config.hide_spawn_agent_metadata,
                include_usage_hint: config.spawn_agent_usage_hint,
                usage_hint_text: config.spawn_agent_usage_hint_text.clone(),
                max_concurrent_threads_per_session: config.max_concurrent_threads_per_session,
            };
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(
                    SpawnAgentHandler,
                    create_spawn_agent_tool_v1(spawn_agent_options),
                ),
            );
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(SendInputHandler, create_send_input_tool_v1()),
            );
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(ResumeAgentHandler, create_resume_agent_tool()),
            );
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(
                    WaitAgentHandler,
                    create_wait_agent_tool_v1(params.wait_agent_timeouts),
                ),
            );
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(CloseAgentHandler, create_close_agent_tool_v1()),
            );
        }
    }

    if config.agent_jobs_tools {
        register_and_publish_tool_definition(
            &mut builder,
            config,
            runtime_tool_definition(SpawnAgentsOnCsvHandler, create_spawn_agents_on_csv_tool()),
        );
        if config.agent_jobs_worker_tools {
            register_and_publish_tool_definition(
                &mut builder,
                config,
                runtime_tool_definition(
                    ReportAgentJobResultHandler,
                    create_report_agent_job_result_tool(),
                ),
            );
        }
    }

    register_and_publish_function_tool_definitions(
        &mut builder,
        config,
        params.tool_namespaces,
        mcp_definitions
            .into_iter()
            .chain(dynamic_definitions)
            .chain(extension_tool_definitions(
                params.extension_tool_definitions,
            )),
    );

    if !deferred_mcp_definitions.is_empty() {
        let directly_registered_mcp_tools = params
            .mcp_tools
            .into_iter()
            .flatten()
            .map(ToolInfo::canonical_tool_name)
            .collect::<HashSet<_>>();
        for definition in deferred_mcp_definitions {
            if !directly_registered_mcp_tools.contains(definition.tool_name()) {
                register_tool_definition_handler(&mut builder, &definition);
            }
        }
    }

    builder
}

type FunctionRuntimeToolDefinition = ToolDefinition<Arc<dyn AnyToolHandler>>;
type RuntimeToolDefinition = ToolDefinition<Arc<dyn AnyToolHandler>, ToolSpec>;

struct ToolSearchPlan {
    register: bool,
    entries: Vec<ToolSearchEntry>,
    source_infos: Vec<ToolSearchSourceInfo>,
}

impl ToolSearchPlan {
    fn should_register(&self) -> bool {
        self.register
    }

    fn deferred_tools_available(&self) -> bool {
        self.register && !self.entries.is_empty()
    }
}

fn build_tool_search_plan(
    config: &ToolsConfig,
    deferred_mcp_tools: Option<&[ToolInfo]>,
    deferred_mcp_definitions: &[FunctionRuntimeToolDefinition],
    dynamic_tools: &[codex_protocol::dynamic_tools::DynamicToolSpec],
    dynamic_definitions: &[FunctionRuntimeToolDefinition],
) -> ToolSearchPlan {
    debug_assert_eq!(
        deferred_mcp_tools.map_or(0, <[codex_mcp::ToolInfo]>::len),
        deferred_mcp_definitions.len()
    );
    debug_assert_eq!(dynamic_tools.len(), dynamic_definitions.len());

    let mut entries = Vec::new();
    let mcp_search_enabled = config.namespace_tools && deferred_mcp_tools.is_some();
    let deferred_mcp_tools = deferred_mcp_tools.unwrap_or_default();
    if config.namespace_tools {
        let mut searchable_mcp_tools = deferred_mcp_tools
            .iter()
            .zip(deferred_mcp_definitions.iter())
            .collect::<Vec<_>>();
        searchable_mcp_tools.sort_by_key(|(tool, _)| tool.canonical_tool_name());
        for (tool, definition) in searchable_mcp_tools {
            match mcp_tool_search_entry(tool, definition) {
                Ok(entry) => entries.push(entry),
                Err(error) => {
                    let tool_name = tool.canonical_tool_name();
                    tracing::error!(
                        "Failed to convert deferred MCP tool `{tool_name}` to OpenAI tool: {error:?}"
                    );
                }
            }
        }
    }

    let mut searchable_dynamic_tools = dynamic_tools
        .iter()
        .zip(dynamic_definitions.iter())
        .filter(|(_, definition)| {
            matches!(definition.exposure(), ToolExposure::Deferred)
                && (config.namespace_tools || definition.tool_name().namespace.is_none())
        })
        .collect::<Vec<_>>();
    let has_searchable_dynamic_tools = !searchable_dynamic_tools.is_empty();
    searchable_dynamic_tools.sort_by(|(left, _), (right, _)| {
        left.namespace
            .cmp(&right.namespace)
            .then(left.name.cmp(&right.name))
    });
    for (tool, definition) in searchable_dynamic_tools {
        match dynamic_tool_search_entry(tool, definition) {
            Ok(entry) => entries.push(entry),
            Err(error) => {
                tracing::error!(
                    "Failed to convert deferred dynamic tool {:?} to OpenAI tool: {error:?}",
                    tool.name
                );
            }
        }
    }

    let mut source_infos = if config.namespace_tools {
        collect_tool_search_source_infos(deferred_mcp_tools.iter().map(|tool| ToolSearchSource {
            server_name: tool.server_name.as_str(),
            connector_name: tool.connector_name.as_deref(),
            description: tool.namespace_description.as_deref(),
        }))
    } else {
        Vec::new()
    };
    if has_searchable_dynamic_tools {
        source_infos.push(ToolSearchSourceInfo {
            name: "Dynamic tools".to_string(),
            description: Some("Tools provided by the current Codex thread.".to_string()),
        });
    }

    ToolSearchPlan {
        register: config.search_tool && (mcp_search_enabled || has_searchable_dynamic_tools),
        entries,
        source_infos,
    }
}

fn runtime_tool_definition<H>(handler: H, spec: ToolSpec) -> RuntimeToolDefinition
where
    H: ToolHandler + 'static,
{
    let handler = Arc::new(handler);
    let tool_name = handler.tool_name();
    let runtime: Arc<dyn AnyToolHandler> = handler;
    ToolDefinition::new(tool_name, spec, runtime)
}

fn register_and_publish_tool_definition(
    builder: &mut ToolRegistryBuilder,
    config: &ToolsConfig,
    definition: RuntimeToolDefinition,
) {
    register_and_publish_tool_definitions(builder, config, std::iter::once(definition));
}

fn register_and_publish_tool_definitions(
    builder: &mut ToolRegistryBuilder,
    config: &ToolsConfig,
    definitions: impl IntoIterator<Item = RuntimeToolDefinition>,
) {
    let mut specs = Vec::new();

    for definition in definitions {
        if register_tool_definition_handler(builder, &definition) {
            specs.push(definition.spec().clone());
        }
    }

    for spec in coalesce_tool_specs(specs) {
        if config.namespace_tools || !matches!(spec, ToolSpec::Namespace(_)) {
            builder.push_spec(spec);
        }
    }
}

fn register_and_publish_function_tool_definitions(
    builder: &mut ToolRegistryBuilder,
    config: &ToolsConfig,
    tool_namespaces: Option<&std::collections::HashMap<String, ToolNamespace>>,
    definitions: impl IntoIterator<Item = FunctionRuntimeToolDefinition>,
) {
    register_and_publish_tool_definitions(
        builder,
        config,
        definitions
            .into_iter()
            .filter_map(|definition| render_function_tool_definition(definition, tool_namespaces)),
    );
}

fn render_function_tool_definition(
    definition: FunctionRuntimeToolDefinition,
    tool_namespaces: Option<&std::collections::HashMap<String, ToolNamespace>>,
) -> Option<RuntimeToolDefinition> {
    let tool_name = definition.tool_name().clone();
    let namespace_description = namespace_description_for_tool(&tool_name, tool_namespaces);
    let spec = match tool_definition_to_loadable_tool_spec(&definition, namespace_description) {
        Ok(spec) => ToolSpec::from(spec),
        Err(error) => {
            tracing::error!("Failed to convert tool `{tool_name}` to OpenAI tool: {error:?}");
            return None;
        }
    };
    let exposure = definition.exposure();
    let mut rendered = ToolDefinition::new(tool_name, spec, Arc::clone(definition.runtime()));
    if matches!(exposure, ToolExposure::Deferred) {
        rendered = rendered.deferred();
    }
    Some(rendered)
}

fn coalesce_tool_specs(specs: Vec<ToolSpec>) -> Vec<ToolSpec> {
    let mut coalesced_specs = Vec::new();
    for spec in specs {
        match spec {
            ToolSpec::Namespace(mut namespace) => {
                if let Some(existing_namespace) =
                    coalesced_specs.iter_mut().find_map(|spec| match spec {
                        ToolSpec::Namespace(existing_namespace)
                            if existing_namespace.name == namespace.name =>
                        {
                            Some(existing_namespace)
                        }
                        _ => None,
                    })
                {
                    existing_namespace.tools.append(&mut namespace.tools);
                } else {
                    coalesced_specs.push(ToolSpec::Namespace(namespace));
                }
            }
            spec => coalesced_specs.push(spec),
        }
    }
    coalesced_specs
}

fn register_tool_definition_handler<S>(
    builder: &mut ToolRegistryBuilder,
    definition: &ToolDefinition<Arc<dyn AnyToolHandler>, S>,
) -> bool {
    builder.register_erased_handler(
        definition.tool_name().clone(),
        Arc::clone(definition.runtime()),
    )
}

fn mcp_tool_definitions(mcp_tools: Option<&[ToolInfo]>) -> Vec<FunctionRuntimeToolDefinition> {
    let mut tools = mcp_tools.into_iter().flatten().collect::<Vec<_>>();
    tools.sort_by_key(|tool| tool.canonical_tool_name());

    tools
        .into_iter()
        .filter_map(|tool| {
            let tool_name = tool.canonical_tool_name();
            if tool_name.namespace.is_none() {
                tracing::error!("Skipping MCP tool `{tool_name}`: MCP tools must be namespaced");
                return None;
            }
            Some(
                mcp_tool_definition(tool_name, &tool.tool)
                    .with_runtime(
                        Arc::new(McpHandler::new(tool.clone())) as Arc<dyn AnyToolHandler>
                    ),
            )
        })
        .collect()
}

fn deferred_mcp_tool_definitions(
    mcp_tools: Option<&[ToolInfo]>,
) -> Vec<FunctionRuntimeToolDefinition> {
    mcp_tools
        .into_iter()
        .flatten()
        .map(|tool| {
            mcp_tool_definition(tool.canonical_tool_name(), &tool.tool)
                .deferred()
                .with_runtime(Arc::new(McpHandler::new(tool.clone())) as Arc<dyn AnyToolHandler>)
        })
        .collect()
}

fn dynamic_tool_definitions(
    dynamic_tools: &[codex_protocol::dynamic_tools::DynamicToolSpec],
) -> Vec<FunctionRuntimeToolDefinition> {
    dynamic_tools
        .iter()
        .map(|tool| {
            let definition = parse_dynamic_tool(tool);
            let handler = Arc::new(DynamicToolHandler::new(definition.tool_name().clone()))
                as Arc<dyn AnyToolHandler>;
            definition.with_runtime(handler)
        })
        .collect()
}

fn extension_tool_definitions(
    definitions: &[ToolDefinition<Arc<dyn ToolExecutor>>],
) -> Vec<FunctionRuntimeToolDefinition> {
    definitions
        .iter()
        .map(|definition| {
            let handler =
                Arc::new(ExtensionToolHandler::new(definition.clone())) as Arc<dyn AnyToolHandler>;
            definition.clone().with_runtime(handler)
        })
        .collect()
}

fn namespace_description_for_tool(
    tool_name: &ToolName,
    tool_namespaces: Option<&std::collections::HashMap<String, ToolNamespace>>,
) -> Option<String> {
    let namespace = tool_name.namespace.as_ref()?;
    let tool_namespace = tool_namespaces.and_then(|namespaces| namespaces.get(namespace));
    tool_namespace.map(|tool_namespace| {
        tool_namespace
            .description
            .as_deref()
            .map(str::trim)
            .filter(|description| !description.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| default_namespace_description(tool_namespace.name.as_str()))
    })
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
