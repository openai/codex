use crate::session::turn_context::TurnContext;
use crate::tools::code_mode::execute_spec::create_code_mode_tool;
use crate::tools::handlers::ApplyPatchHandler;
use crate::tools::handlers::CodeModeExecuteHandler;
use crate::tools::handlers::CodeModeWaitHandler;
use crate::tools::handlers::CreateGoalHandler;
use crate::tools::handlers::DynamicToolHandler;
use crate::tools::handlers::GetGoalHandler;
use crate::tools::handlers::ListMcpResourceTemplatesHandler;
use crate::tools::handlers::ListMcpResourcesHandler;
use crate::tools::handlers::McpHandler;
use crate::tools::handlers::PlanHandler;
use crate::tools::handlers::ReadMcpResourceHandler;
use crate::tools::handlers::RequestPermissionsHandler;
use crate::tools::handlers::RequestPluginInstallHandler;
use crate::tools::handlers::RequestUserInputHandler;
use crate::tools::handlers::TestSyncHandler;
use crate::tools::handlers::ToolSearchHandler;
use crate::tools::handlers::UpdateGoalHandler;
use crate::tools::handlers::ViewImageHandler;
use crate::tools::handlers::agent_jobs::ReportAgentJobResultHandler;
use crate::tools::handlers::agent_jobs::SpawnAgentsOnCsvHandler;
use crate::tools::handlers::extension_tools::ExtensionToolAdapter;
use crate::tools::handlers::multi_agents::CloseAgentHandler;
use crate::tools::handlers::multi_agents::ResumeAgentHandler;
use crate::tools::handlers::multi_agents::SendInputHandler;
use crate::tools::handlers::multi_agents::SpawnAgentHandler;
use crate::tools::handlers::multi_agents::WaitAgentHandler;
use crate::tools::handlers::multi_agents_common::DEFAULT_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MAX_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MIN_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_spec::SpawnAgentToolOptions;
use crate::tools::handlers::multi_agents_spec::WaitAgentTimeoutOptions;
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
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExposure;
use crate::tools::registry::ToolRegistry;
use crate::tools::registry::override_tool_exposure;
use crate::tools::router::ToolRouter;
use crate::tools::router::ToolRouterParams;
use crate::tools::tool_family::shell::ShellToolsOptions;
use crate::tools::tool_family::shell::register_shell_tools;
use crate::tools::tool_set::ToolSet;
use crate::tools::tool_set::ToolSetBuilder;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_mcp::ToolInfo;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::openai_models::InputModality;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_tools::DiscoverableTool;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolCall as ExtensionToolCall;
use codex_tools::ToolEnvironmentMode;
use codex_tools::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_tools::can_request_original_image_detail;
use codex_tools::collect_code_mode_exec_prompt_tool_definitions;
use codex_tools::default_namespace_description;
use codex_tools::request_user_input_available_modes;
use codex_tools::shell_command_backend_for_features;
use codex_tools::shell_type_for_model_and_features;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::warn;

#[derive(Clone, Copy)]
struct ToolSetBuildParams<'a> {
    mcp_tools: Option<&'a [ToolInfo]>,
    deferred_mcp_tools: Option<&'a [ToolInfo]>,
    discoverable_tools: Option<&'a [DiscoverableTool]>,
    extension_tool_executors: &'a [Arc<dyn ToolExecutor<ExtensionToolCall>>],
    dynamic_tools: &'a [DynamicToolSpec],
    default_agent_type_description: &'a str,
    wait_agent_timeouts: WaitAgentTimeoutOptions,
}

pub(crate) fn build_tool_router(
    turn_context: &TurnContext,
    params: ToolRouterParams<'_>,
) -> ToolRouter {
    let (model_visible_specs, registry) = build_tool_specs_and_registry(turn_context, params);
    ToolRouter::from_parts(registry, model_visible_specs)
}

fn build_tool_specs_and_registry(
    turn_context: &TurnContext,
    params: ToolRouterParams<'_>,
) -> (Vec<ToolSpec>, ToolRegistry) {
    let ToolRouterParams {
        mcp_tools,
        deferred_mcp_tools,
        discoverable_tools,
        extension_tool_executors,
        dynamic_tools,
    } = params;
    let default_agent_type_description =
        crate::agent::role::spawn_tool_spec::build(&std::collections::BTreeMap::new());
    let mut tool_set = ToolSetBuilder::new();
    register_tool_runtimes(
        turn_context,
        ToolSetBuildParams {
            mcp_tools: mcp_tools.as_deref(),
            deferred_mcp_tools: deferred_mcp_tools.as_deref(),
            discoverable_tools: discoverable_tools.as_deref(),
            extension_tool_executors: &extension_tool_executors,
            dynamic_tools,
            default_agent_type_description: &default_agent_type_description,
            wait_agent_timeouts: wait_agent_timeout_options(turn_context),
        },
        &mut tool_set,
    );
    tool_set.extend_hosted_specs(hosted_model_tool_specs(turn_context));
    append_tool_search_executor(turn_context, &mut tool_set);
    prepend_code_mode_executors(turn_context, &mut tool_set);
    build_model_visible_specs_and_registry(turn_context, tool_set.finish())
}

fn build_model_visible_specs_and_registry(
    turn_context: &TurnContext,
    tool_set: ToolSet,
) -> (Vec<ToolSpec>, ToolRegistry) {
    let (runtimes, hosted_specs) = tool_set.into_parts();
    let mut specs = Vec::new();
    let mut seen_tool_names = HashSet::new();
    for runtime in &runtimes {
        if !seen_tool_names.insert(runtime.tool_name()) {
            continue;
        }
        if runtime.exposure().is_direct()
            && let Some(spec) = runtime.spec()
        {
            specs.push(spec_for_model_request(
                turn_context,
                runtime.exposure(),
                spec,
            ));
        }
    }
    specs.extend(hosted_specs);

    let registry = ToolRegistry::from_tools(runtimes);
    let model_visible_specs = merge_into_namespaces(specs)
        .into_iter()
        .filter(|spec| {
            namespace_tools_enabled(turn_context) || !matches!(spec, ToolSpec::Namespace(_))
        })
        .filter(|spec| !is_hidden_by_code_mode_only(turn_context, &registry, spec))
        .collect();

    (model_visible_specs, registry)
}

fn spec_for_model_request(
    turn_context: &TurnContext,
    exposure: ToolExposure,
    spec: ToolSpec,
) -> ToolSpec {
    if code_mode_enabled(turn_context)
        && exposure != ToolExposure::DirectModelOnly
        && codex_code_mode::is_code_mode_nested_tool(spec.name())
    {
        codex_tools::augment_tool_spec_for_code_mode(spec)
    } else {
        spec
    }
}

pub(crate) fn hosted_model_tool_specs(turn_context: &TurnContext) -> Vec<ToolSpec> {
    let mut specs = Vec::new();
    let provider_capabilities = turn_context.provider.capabilities();
    let web_search_mode = provider_capabilities
        .web_search
        .then_some(turn_context.config.web_search_mode.value());
    let web_search_config = if provider_capabilities.web_search {
        turn_context.config.web_search_config.as_ref()
    } else {
        None
    };
    if let Some(web_search_tool) = create_web_search_tool(WebSearchToolOptions {
        web_search_mode,
        web_search_config,
        web_search_tool_type: turn_context.model_info.web_search_tool_type,
    }) {
        specs.push(web_search_tool);
    }
    if image_generation_tool_enabled(turn_context) {
        specs.push(create_image_generation_tool("png"));
    }
    specs
}

pub(crate) fn search_tool_enabled(turn_context: &TurnContext) -> bool {
    turn_context.model_info.supports_search_tool
        && turn_context.features.get().enabled(Feature::ToolSearch)
}

pub(crate) fn tool_suggest_enabled(turn_context: &TurnContext) -> bool {
    let features = turn_context.features.get();
    features.enabled(Feature::ToolSuggest)
        && features.enabled(Feature::Apps)
        && features.enabled(Feature::Plugins)
}

fn namespace_tools_enabled(turn_context: &TurnContext) -> bool {
    turn_context.provider.capabilities().namespace_tools
}

fn code_mode_enabled(turn_context: &TurnContext) -> bool {
    turn_context.features.get().enabled(Feature::CodeMode)
}

fn code_mode_only_enabled(turn_context: &TurnContext) -> bool {
    code_mode_enabled(turn_context) && turn_context.features.get().enabled(Feature::CodeModeOnly)
}

fn multi_agent_v2_enabled(turn_context: &TurnContext) -> bool {
    turn_context.features.get().enabled(Feature::MultiAgentV2)
}

fn collab_tools_enabled(turn_context: &TurnContext) -> bool {
    multi_agent_v2_enabled(turn_context) || turn_context.features.get().enabled(Feature::Collab)
}

fn agent_jobs_tools_enabled(turn_context: &TurnContext) -> bool {
    turn_context.features.get().enabled(Feature::SpawnCsv)
}

fn agent_jobs_worker_tools_enabled(turn_context: &TurnContext) -> bool {
    agent_jobs_tools_enabled(turn_context)
        && matches!(
            &turn_context.session_source,
            SessionSource::SubAgent(SubAgentSource::Other(label))
                if label.starts_with("agent_job:")
        )
}

fn image_generation_tool_enabled(turn_context: &TurnContext) -> bool {
    turn_context
        .auth_manager
        .as_deref()
        .is_some_and(AuthManager::current_auth_uses_codex_backend)
        && turn_context.provider.capabilities().image_generation
        && turn_context
            .features
            .get()
            .enabled(Feature::ImageGeneration)
        && turn_context
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
}

fn wait_agent_timeout_options(turn_context: &TurnContext) -> WaitAgentTimeoutOptions {
    if multi_agent_v2_enabled(turn_context) {
        return WaitAgentTimeoutOptions {
            default_timeout_ms: turn_context.config.multi_agent_v2.default_wait_timeout_ms,
            min_timeout_ms: turn_context.config.multi_agent_v2.min_wait_timeout_ms,
            max_timeout_ms: turn_context.config.multi_agent_v2.max_wait_timeout_ms,
        };
    }

    WaitAgentTimeoutOptions {
        default_timeout_ms: DEFAULT_WAIT_TIMEOUT_MS,
        min_timeout_ms: MIN_WAIT_TIMEOUT_MS,
        max_timeout_ms: MAX_WAIT_TIMEOUT_MS,
    }
}

fn max_concurrent_threads_per_session(turn_context: &TurnContext) -> Option<usize> {
    multi_agent_v2_enabled(turn_context).then_some(
        turn_context
            .config
            .multi_agent_v2
            .max_concurrent_threads_per_session,
    )
}

fn agent_type_description(
    turn_context: &TurnContext,
    default_agent_type_description: &str,
) -> String {
    let agent_type_description =
        crate::agent::role::spawn_tool_spec::build(&turn_context.config.agent_roles);
    if agent_type_description.is_empty() {
        default_agent_type_description.to_string()
    } else {
        agent_type_description
    }
}

fn is_hidden_by_code_mode_only(
    turn_context: &TurnContext,
    registry: &ToolRegistry,
    spec: &ToolSpec,
) -> bool {
    if !code_mode_only_enabled(turn_context)
        || !codex_code_mode::is_code_mode_nested_tool(spec.name())
    {
        return false;
    }

    let exposure = registry
        .tool_exposure(&ToolName::plain(spec.name()))
        .unwrap_or(ToolExposure::Direct);
    exposure != ToolExposure::DirectModelOnly
}

fn build_code_mode_executors(
    turn_context: &TurnContext,
    executors: &[Arc<dyn CoreToolRuntime>],
    deferred_tools_available: bool,
) -> Vec<Arc<dyn CoreToolRuntime>> {
    if !code_mode_enabled(turn_context) {
        return vec![];
    }

    let code_mode_nested_tool_specs = executors
        .iter()
        .filter_map(|executor| {
            if executor.exposure() == ToolExposure::DirectModelOnly {
                return None;
            }

            executor.spec()
        })
        .collect::<Vec<_>>();
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
                code_mode_only_enabled(turn_context),
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

fn register_tool_runtimes(
    turn_context: &TurnContext,
    params: ToolSetBuildParams<'_>,
    tool_set: &mut ToolSetBuilder,
) {
    let features = turn_context.features.get();
    let environment_mode = turn_context.tool_environment_mode();
    register_shell_tools(
        tool_set,
        ShellToolsOptions {
            shell_type: shell_type_for_model_and_features(&turn_context.model_info, features),
            shell_command_backend: shell_command_backend_for_features(features),
            environment_mode,
            allow_login_shell: turn_context.config.permissions.allow_login_shell,
            exec_permission_approvals_enabled: features.enabled(Feature::ExecPermissionApprovals),
        },
    );

    if params.mcp_tools.is_some() {
        tool_set.add_runtime(ListMcpResourcesHandler);
        tool_set.add_runtime(ListMcpResourceTemplatesHandler);
        tool_set.add_runtime(ReadMcpResourceHandler);
    }

    tool_set.add_runtime(PlanHandler);
    if turn_context.goal_tools_enabled() {
        tool_set.add_runtime(GetGoalHandler);
        tool_set.add_runtime(CreateGoalHandler);
        tool_set.add_runtime(UpdateGoalHandler);
    }

    tool_set.add_runtime(RequestUserInputHandler {
        available_modes: request_user_input_available_modes(features),
    });

    if features.enabled(Feature::RequestPermissionsTool) {
        tool_set.add_runtime(RequestPermissionsHandler);
    }

    if tool_suggest_enabled(turn_context)
        && let Some(discoverable_tools) =
            params.discoverable_tools.filter(|tools| !tools.is_empty())
    {
        tool_set.add_runtime(RequestPluginInstallHandler::new(discoverable_tools));
    }

    if environment_mode.has_environment() && turn_context.model_info.apply_patch_tool_type.is_some()
    {
        let include_environment_id = matches!(environment_mode, ToolEnvironmentMode::Multiple);
        tool_set.add_runtime(ApplyPatchHandler::new(include_environment_id));
    }

    if turn_context
        .model_info
        .experimental_supported_tools
        .iter()
        .any(|tool| tool == "test_sync_tool")
    {
        tool_set.add_runtime(TestSyncHandler);
    }

    if environment_mode.has_environment() {
        let include_environment_id = matches!(environment_mode, ToolEnvironmentMode::Multiple);
        tool_set.add_runtime(ViewImageHandler::new(ViewImageToolOptions {
            can_request_original_image_detail: can_request_original_image_detail(
                &turn_context.model_info,
            ),
            include_environment_id,
        }));
    }

    if collab_tools_enabled(turn_context) {
        if multi_agent_v2_enabled(turn_context) {
            let exposure = if turn_context.config.multi_agent_v2.non_code_mode_only {
                ToolExposure::DirectModelOnly
            } else {
                ToolExposure::Direct
            };
            let agent_type_description =
                agent_type_description(turn_context, params.default_agent_type_description);
            tool_set.add_runtime_arc(multi_agent_v2_handler(
                SpawnAgentHandlerV2::new(SpawnAgentToolOptions {
                    available_models: turn_context.available_models.clone(),
                    agent_type_description,
                    hide_agent_type_model_reasoning: turn_context
                        .config
                        .multi_agent_v2
                        .hide_spawn_agent_metadata,
                    include_usage_hint: turn_context.config.multi_agent_v2.usage_hint_enabled,
                    usage_hint_text: turn_context.config.multi_agent_v2.usage_hint_text.clone(),
                    max_concurrent_threads_per_session: max_concurrent_threads_per_session(
                        turn_context,
                    ),
                }),
                exposure,
            ));
            tool_set.add_runtime_arc(multi_agent_v2_handler(SendMessageHandlerV2, exposure));
            tool_set.add_runtime_arc(multi_agent_v2_handler(FollowupTaskHandlerV2, exposure));
            tool_set.add_runtime_arc(multi_agent_v2_handler(
                WaitAgentHandlerV2::new(params.wait_agent_timeouts),
                exposure,
            ));
            tool_set.add_runtime_arc(multi_agent_v2_handler(CloseAgentHandlerV2, exposure));
            tool_set.add_runtime_arc(multi_agent_v2_handler(ListAgentsHandlerV2, exposure));
        } else {
            let agent_type_description =
                agent_type_description(turn_context, params.default_agent_type_description);
            tool_set.add_runtime(SpawnAgentHandler::new(SpawnAgentToolOptions {
                available_models: turn_context.available_models.clone(),
                agent_type_description,
                hide_agent_type_model_reasoning: turn_context
                    .config
                    .multi_agent_v2
                    .hide_spawn_agent_metadata,
                include_usage_hint: turn_context.config.multi_agent_v2.usage_hint_enabled,
                usage_hint_text: turn_context.config.multi_agent_v2.usage_hint_text.clone(),
                max_concurrent_threads_per_session: max_concurrent_threads_per_session(
                    turn_context,
                ),
            }));
            tool_set.add_runtime(SendInputHandler);
            tool_set.add_runtime(ResumeAgentHandler);
            tool_set.add_runtime(WaitAgentHandler::new(params.wait_agent_timeouts));
            tool_set.add_runtime(CloseAgentHandler);
        }
    }

    if agent_jobs_tools_enabled(turn_context) {
        tool_set.add_runtime(SpawnAgentsOnCsvHandler);
        if agent_jobs_worker_tools_enabled(turn_context) {
            tool_set.add_runtime(ReportAgentJobResultHandler);
        }
    }

    if let Some(mcp_tools) = params.mcp_tools {
        for tool in mcp_tools {
            tool_set.add_runtime(McpHandler::new(tool.clone()));
        }
    }

    if let Some(deferred_mcp_tools) = params.deferred_mcp_tools {
        for tool in deferred_mcp_tools {
            tool_set.add_runtime(McpHandler::with_exposure(
                tool.clone(),
                ToolExposure::Deferred,
            ));
        }
    }

    for tool in params.dynamic_tools {
        let Some(handler) = DynamicToolHandler::new(tool) else {
            tracing::error!(
                "Failed to convert dynamic tool {:?} to OpenAI tool",
                tool.name
            );
            continue;
        };

        tool_set.add_runtime(handler);
    }

    append_extension_tool_executors(turn_context, params.extension_tool_executors, tool_set);
}

fn append_tool_search_executor(turn_context: &TurnContext, tool_set: &mut ToolSetBuilder) {
    if !(search_tool_enabled(turn_context) && namespace_tools_enabled(turn_context)) {
        return;
    }

    let search_infos = tool_set
        .runtimes()
        .iter()
        .filter(|executor| executor.exposure() == ToolExposure::Deferred)
        .filter_map(|executor| executor.search_info())
        .collect::<Vec<_>>();
    if search_infos.is_empty() {
        return;
    }

    tool_set.add_runtime(ToolSearchHandler::new(search_infos));
}

fn prepend_code_mode_executors(turn_context: &TurnContext, tool_set: &mut ToolSetBuilder) {
    let deferred_tools_available = search_tool_enabled(turn_context)
        && tool_set
            .runtimes()
            .iter()
            .any(|executor| executor.exposure() == ToolExposure::Deferred);
    let code_mode_executors =
        build_code_mode_executors(turn_context, tool_set.runtimes(), deferred_tools_available);
    tool_set.prepend_runtime_arcs(code_mode_executors);
}

fn append_extension_tool_executors(
    turn_context: &TurnContext,
    executors: &[Arc<dyn ToolExecutor<ExtensionToolCall>>],
    tool_set: &mut ToolSetBuilder,
) {
    if executors.is_empty() {
        return;
    }

    let mut reserved_tool_names = tool_set
        .runtimes()
        .iter()
        .map(|executor| executor.tool_name())
        .collect::<HashSet<_>>();
    if code_mode_enabled(turn_context) {
        reserved_tool_names.insert(ToolName::plain(codex_code_mode::PUBLIC_TOOL_NAME));
        reserved_tool_names.insert(ToolName::plain(codex_code_mode::WAIT_TOOL_NAME));
    }
    if search_tool_enabled(turn_context)
        && namespace_tools_enabled(turn_context)
        && tool_set
            .runtimes()
            .iter()
            .any(|executor| executor.exposure() == ToolExposure::Deferred)
    {
        reserved_tool_names.insert(ToolName::plain(TOOL_SEARCH_TOOL_NAME));
    }

    for executor in executors.iter().cloned() {
        let tool_name = executor.tool_name();
        if !reserved_tool_names.insert(tool_name.clone()) {
            warn!("Skipping extension tool `{tool_name}`: tool already registered");
            continue;
        }
        tool_set.add_runtime(ExtensionToolAdapter::new(executor));
    }
}

fn multi_agent_v2_handler(
    handler: impl CoreToolRuntime + 'static,
    exposure: ToolExposure,
) -> Arc<dyn CoreToolRuntime> {
    override_tool_exposure(Arc::new(handler), exposure)
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
