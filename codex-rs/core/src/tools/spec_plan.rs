use crate::agent::exceeds_thread_spawn_depth_limit;
use crate::agent::next_thread_spawn_depth;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::image_preparation::unified_image_budget_enabled;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::code_mode::execute_spec::create_code_mode_tool;
use crate::tools::context::ToolInvocation;
use crate::tools::effective_tool_mode;
use crate::tools::handlers::ApplyPatchHandler;
use crate::tools::handlers::CodeModeExecuteHandler;
use crate::tools::handlers::CodeModeWaitHandler;
use crate::tools::handlers::CurrentTimeHandler;
use crate::tools::handlers::DynamicToolHandler;
use crate::tools::handlers::ExecCommandHandler;
use crate::tools::handlers::ExecCommandHandlerOptions;
use crate::tools::handlers::GetContextRemainingHandler;
use crate::tools::handlers::ListAvailablePluginsToInstallHandler;
use crate::tools::handlers::ListMcpResourceTemplatesHandler;
use crate::tools::handlers::ListMcpResourcesHandler;
use crate::tools::handlers::NewContextWindowHandler;
use crate::tools::handlers::PlanHandler;
use crate::tools::handlers::ReadMcpResourceHandler;
use crate::tools::handlers::RequestPermissionsHandler;
use crate::tools::handlers::RequestPluginInstallHandler;
use crate::tools::handlers::RequestUserInputHandler;
use crate::tools::handlers::SendUserMessageAsyncHandler;
use crate::tools::handlers::SleepHandler;
use crate::tools::handlers::TestSyncHandler;
use crate::tools::handlers::ToolSearchHandlerCache;
use crate::tools::handlers::ViewImageHandler;
use crate::tools::handlers::WaitForEnvironmentHandler;
use crate::tools::handlers::WriteStdinHandler;
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
use crate::tools::handlers::multi_agents_v2::FollowupTaskHandler as FollowupTaskHandlerV2;
use crate::tools::handlers::multi_agents_v2::InterruptAgentHandler;
use crate::tools::handlers::multi_agents_v2::ListAgentsHandler as ListAgentsHandlerV2;
use crate::tools::handlers::multi_agents_v2::SendMessageHandler as SendMessageHandlerV2;
use crate::tools::handlers::multi_agents_v2::SpawnAgentHandler as SpawnAgentHandlerV2;
use crate::tools::handlers::multi_agents_v2::WaitAgentHandler as WaitAgentHandlerV2;
use crate::tools::handlers::tool_search_spec::ToolSearchSourceListing;
use crate::tools::handlers::view_image_spec::ViewImageToolOptions;
use crate::tools::hosted_spec::WebSearchToolOptions;
use crate::tools::hosted_spec::create_web_search_tool;
use crate::tools::registry::CoreToolRuntime;
#[cfg(test)]
use crate::tools::registry::RegisteredTool;
use crate::tools::registry::ToolExposure;
use crate::tools::registry::ToolRegistry;
use crate::tools::router::ToolRouter;
use crate::tools::tool_namespaces_info::collect_tool_namespaces_info;
use codex_extension_api::ExtensionData;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_protocol::DEFAULT_FUNCTION_NAMESPACE;
use codex_protocol::account::PlanType;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::dynamic_tools::DynamicToolNamespaceTool;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::MultiAgentVersion;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolCall as ExtensionToolCall;
use codex_tools::ToolEnvironmentMode;
use codex_tools::ToolExecutor;
use codex_tools::ToolExposures;
use codex_tools::ToolName;
use codex_tools::ToolSearchInfo;
use codex_tools::ToolSpec;
use codex_tools::UnifiedExecShellMode;
use codex_tools::can_request_original_image_detail;
use codex_tools::collect_code_mode_exec_prompt_tool_definitions;
use codex_tools::collect_request_plugin_install_entries;
use codex_tools::default_namespace_description;
use codex_tools::request_user_input_available_modes;
use futures::future::BoxFuture;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::btree_map::Entry;
use std::sync::Arc;
use tracing::instrument;

const MULTI_AGENT_V2_NAMESPACE_DESCRIPTION: &str = "Tools for spawning and managing sub-agents.";
const IMAGE_GEN_NAMESPACE: &str = "image_gen";
const IMAGEGEN_TOOL_NAME: &str = "imagegen";

#[derive(Clone, Copy)]
struct CoreToolPlanContext<'a> {
    turn_context: &'a TurnContext,
    environments: &'a TurnEnvironmentSnapshot,
    mcp: &'a codex_mcp::McpBinding,
    tool_suggest_candidates: Option<&'a crate::tools::router::ToolSuggestCandidates>,
    wait_for_environment_tool_config: Option<&'a Arc<crate::WaitForEnvironmentToolConfig>>,
    default_agent_type_description: &'a str,
    wait_agent_timeouts: WaitAgentTimeoutOptions,
}

#[instrument(level = "trace", skip_all)]
pub(crate) fn build_tool_router(
    session: &Session,
    turn_context: &TurnContext,
    environments: &TurnEnvironmentSnapshot,
    mcp: &Arc<codex_mcp::McpBinding>,
    apps_enabled: bool,
    step_store: &ExtensionData,
    tool_suggest_candidates: Option<&crate::tools::router::ToolSuggestCandidates>,
) -> CodexResult<ToolRouter> {
    let default_agent_type_description =
        crate::agent::role::spawn_tool_spec::build(&std::collections::BTreeMap::new());
    let wait_for_environment_tool_config = session
        .services
        .thread_extension_data
        .get::<crate::WaitForEnvironmentToolConfig>();
    let context = CoreToolPlanContext {
        turn_context,
        environments,
        mcp,
        tool_suggest_candidates,
        wait_for_environment_tool_config: wait_for_environment_tool_config.as_ref(),
        default_agent_type_description: &default_agent_type_description,
        wait_agent_timeouts: wait_agent_timeout_options(turn_context),
    };
    let mut registry = ToolRegistry::default();
    add_core_tool_sources(&context, &mut registry);

    let hosted_specs = if crate::guardian::is_guardian_reviewer_source(&turn_context.session_source)
    {
        Vec::new()
    } else {
        let registered_mcp_tools = session.services.mcp_handler_cache.append_mcp_tools(
            mcp,
            &turn_context.config,
            apps_enabled,
            &mcp.config().mcp_server_catalog,
            search_tool_enabled(turn_context),
            &mut registry,
        );
        apply_mcp_tool_exposure_policy(turn_context, mcp, &registered_mcp_tools, &mut registry);
        let standalone_web_search_tool = append_extension_tool_executors(
            turn_context,
            extension_tool_executors(session, step_store),
            &mut registry,
        );
        append_dynamic_tool_runtimes(&turn_context.dynamic_tools, &mut registry);
        hosted_model_tool_specs(turn_context, standalone_web_search_tool.as_slice())
    };

    finalize_tool_router(
        turn_context,
        registry,
        hosted_specs,
        &session.services.tool_search_handler_cache,
    )
}

fn apply_mcp_tool_exposure_policy(
    turn_context: &TurnContext,
    mcp: &codex_mcp::McpBinding,
    registered_mcp_tools: &HashSet<ToolName>,
    registry: &mut ToolRegistry,
) {
    let mut omitted_exposures_by_tool = HashMap::new();
    for tool in mcp.tools() {
        let tool_name = tool.canonical_tool_name();
        if !registered_mcp_tools.contains(&tool_name) {
            continue;
        }
        let Some(server) = mcp.config().mcp_server_catalog.server(&tool.server_name) else {
            continue;
        };
        omitted_exposures_by_tool
            .entry(tool_name)
            .or_insert_with(|| {
                server
                    .config()
                    .omit_tools_from
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .copied()
                    .collect::<ToolExposures>()
            });
    }

    for tool in registry.entries_mut() {
        let tool_name = tool.runtime.tool_name();
        let Some(omitted_exposures) = omitted_exposures_by_tool.get(&tool_name) else {
            continue;
        };
        let tool_name = tool_name.with_default_namespace();

        let mut exposures = ToolExposures::ALL.difference(*omitted_exposures);
        if tool_name.namespace.as_ref().is_some_and(|namespace| {
            turn_context
                .config
                .code_mode
                .direct_only_tool_namespaces
                .contains(namespace)
        }) {
            exposures = exposures.difference(ToolExposures::DEFERRED | ToolExposures::CODE_MODE);
        }

        exposures = if search_tool_enabled(turn_context)
            && exposures.contains(ToolExposures::DEFERRED)
            && (effective_tool_mode(turn_context) != ToolMode::CodeModeOnly
                || exposures.contains(ToolExposures::CODE_MODE))
        {
            exposures.difference(ToolExposures::DIRECT)
        } else {
            exposures.difference(ToolExposures::DEFERRED)
        };

        tool.exposure = match (
            exposures.contains(ToolExposures::DIRECT),
            exposures.contains(ToolExposures::DEFERRED),
            exposures.contains(ToolExposures::CODE_MODE),
        ) {
            (false, false, false) => ToolExposure::Hidden,
            (false, false, true) => ToolExposure::CodeModeOnly,
            (true, false, false) => ToolExposure::DirectModelOnly,
            (true, false, true) => ToolExposure::Direct,
            (false, true, false) => ToolExposure::DeferredModelOnly,
            (false, true, true) => ToolExposure::Deferred,
            (true, true, _) => unreachable!("direct and deferred exposure are mutually exclusive"),
        };
    }
}

#[cfg(test)]
#[instrument(level = "trace", skip_all)]
pub(crate) fn build_core_tool_registry(
    turn_context: &TurnContext,
    environments: &TurnEnvironmentSnapshot,
    mcp: &codex_mcp::McpBinding,
    tool_suggest_candidates: Option<&crate::tools::router::ToolSuggestCandidates>,
    wait_for_environment_tool_config: Option<&Arc<crate::WaitForEnvironmentToolConfig>>,
) -> ToolRegistry {
    let default_agent_type_description =
        crate::agent::role::spawn_tool_spec::build(&std::collections::BTreeMap::new());
    let context = CoreToolPlanContext {
        turn_context,
        environments,
        mcp,
        tool_suggest_candidates,
        wait_for_environment_tool_config,
        default_agent_type_description: &default_agent_type_description,
        wait_agent_timeouts: wait_agent_timeout_options(turn_context),
    };
    let mut registry = ToolRegistry::default();
    add_core_tool_sources(&context, &mut registry);
    registry
}

#[cfg(test)]
#[instrument(level = "trace", skip_all)]
pub(crate) fn append_source_tools(
    turn_context: &TurnContext,
    registry: &mut ToolRegistry,
    mcp_tools: Vec<RegisteredTool>,
    extension_tool_executors: impl IntoIterator<Item = Arc<dyn ToolExecutor<ExtensionToolCall>>>,
    dynamic_tools: &[DynamicToolSpec],
) -> Vec<ToolSpec> {
    if crate::guardian::is_guardian_reviewer_source(&turn_context.session_source) {
        return Vec::new();
    }

    for tool in mcp_tools {
        registry.register_external_with_exposure(tool.runtime, tool.exposure);
    }
    let standalone_web_search_tool =
        append_extension_tool_executors(turn_context, extension_tool_executors, registry);
    append_dynamic_tool_runtimes(dynamic_tools, registry);
    hosted_model_tool_specs(turn_context, standalone_web_search_tool.as_slice())
}

#[instrument(level = "trace", skip_all)]
pub(crate) fn extension_tool_executors<'a>(
    session: &'a Session,
    step_store: &'a ExtensionData,
) -> impl Iterator<Item = Arc<dyn ToolExecutor<ExtensionToolCall>>> + 'a {
    session
        .services
        .extensions
        .tool_contributors()
        .iter()
        .flat_map(move |contributor| {
            contributor.tools_for_step(
                &session.services.session_extension_data,
                &session.services.thread_extension_data,
                step_store,
            )
        })
}

#[instrument(level = "trace", skip_all)]
pub(crate) fn finalize_tool_router(
    turn_context: &TurnContext,
    mut registry: ToolRegistry,
    hosted_specs: Vec<ToolSpec>,
    tool_search_handler_cache: &ToolSearchHandlerCache,
) -> CodexResult<ToolRouter> {
    apply_direct_model_only_namespace_overrides(turn_context, &mut registry);
    let code_mode_enabled = matches!(
        effective_tool_mode(turn_context),
        ToolMode::CodeMode | ToolMode::CodeModeOnly
    );
    if code_mode_enabled {
        for tool_name in [
            ToolName::plain(codex_code_mode::PUBLIC_TOOL_NAME),
            ToolName::plain(codex_code_mode::WAIT_TOOL_NAME),
        ] {
            if registry.remove(&tool_name).is_some() {
                registry.record_collision(tool_name);
            }
        }
    }
    let tool_search_name = ToolName::plain(TOOL_SEARCH_TOOL_NAME);
    if search_tool_enabled(turn_context)
        && registry.entries().any(|tool| {
            tool.runtime.tool_name() != tool_search_name
                && tool.exposure.is_deferred()
                && tool.runtime.search_info().is_some()
        })
    {
        if registry.remove(&tool_search_name).is_some() {
            registry.record_collision(tool_search_name.clone());
        }
        // Special model tools own the namespace matching their wire identity, so
        // regular namespace tools cannot advertise that same model-visible surface.
        let conflicting_tool_names = registry
            .entries()
            .filter_map(|tool| {
                let owned_spec;
                let spec = if let Some(spec) = tool.runtime.immutable_spec() {
                    spec.as_ref()
                } else {
                    owned_spec = tool.runtime.spec();
                    &owned_spec
                };
                let ToolSpec::Namespace(namespace) = spec else {
                    return None;
                };
                (namespace.name == tool_search_name.name).then(|| tool.runtime.tool_name())
            })
            .collect::<Vec<_>>();
        for tool_name in conflicting_tool_names {
            if registry.remove(&tool_name).is_some() {
                registry.record_collision(tool_name);
            }
        }
        append_tool_search_executor(turn_context, &mut registry, tool_search_handler_cache);
    }

    let code_mode_tool_names = register_code_mode_executors(turn_context, &mut registry);
    let include_tool_namespaces_info = turn_context
        .config
        .tool_registry
        .turn_metadata_includes_tool_info
        && turn_context.model_info.use_responses_lite;

    if turn_context.config.tool_registry.error_on_tool_collisions {
        if let Some(tool_name) = registry.first_collision() {
            let namespace = tool_name.namespace.as_deref().unwrap_or("functions");
            let name = format!("{namespace}.{}", tool_name.name);
            return Err(CodexErrorDetails::ToolCollision(name).into());
        }

        let mut namespace_descriptions = BTreeMap::new();
        let mut namespace_owners = BTreeMap::new();
        for tool in registry.entries() {
            let owned_spec;
            let spec = if let Some(spec) = tool.runtime.immutable_spec() {
                spec.as_ref()
            } else {
                owned_spec = tool.runtime.spec();
                &owned_spec
            };
            if include_tool_namespaces_info && tool.exposure != ToolExposure::Hidden {
                let namespace_name = match spec {
                    ToolSpec::Namespace(namespace) => namespace.name.as_str(),
                    ToolSpec::Function(_) | ToolSpec::Freeform(_) => DEFAULT_FUNCTION_NAMESPACE,
                    ToolSpec::ToolSearch { .. } => TOOL_SEARCH_TOOL_NAME,
                    ToolSpec::WebSearch { .. } => continue,
                };
                let owner = tool.runtime.mcp_server_name();
                match namespace_owners.get(namespace_name) {
                    Some(existing_owner) if existing_owner != &owner => {
                        return Err(
                            CodexErrorDetails::ToolCollision(namespace_name.to_string()).into()
                        );
                    }
                    Some(_) => {}
                    None => {
                        namespace_owners.insert(namespace_name.to_string(), owner);
                    }
                }
            }

            let ToolSpec::Namespace(namespace) = spec else {
                continue;
            };
            if namespace.description.trim().is_empty() {
                continue;
            }
            match namespace_descriptions.entry(namespace.name.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(namespace.description.clone());
                }
                Entry::Occupied(entry) if entry.get() != &namespace.description => {
                    return Err(CodexErrorDetails::ToolCollision(entry.key().clone()).into());
                }
                Entry::Occupied(_) => {}
            }
        }
    }

    let model_visible_specs =
        build_model_visible_specs(turn_context, &registry, &code_mode_tool_names, hosted_specs);
    if include_tool_namespaces_info {
        turn_context
            .turn_metadata_state
            .set_tool_namespaces_info(collect_tool_namespaces_info(
                &registry,
                &code_mode_tool_names,
                &model_visible_specs,
            ));
    }

    Ok(ToolRouter::from_parts(registry, model_visible_specs))
}

fn apply_direct_model_only_namespace_overrides(
    turn_context: &TurnContext,
    registry: &mut ToolRegistry,
) {
    if turn_context
        .config
        .code_mode
        .direct_only_tool_namespaces
        .is_empty()
    {
        return;
    }

    for tool in registry.entries_mut() {
        let configured = tool
            .runtime
            .tool_name()
            .with_default_namespace()
            .namespace
            .as_ref()
            .is_some_and(|namespace| {
                turn_context
                    .config
                    .code_mode
                    .direct_only_tool_namespaces
                    .contains(namespace)
            });
        if configured && tool.exposure.is_available_in_code_mode() {
            tool.exposure = ToolExposure::DirectModelOnly;
        }
    }
}

#[instrument(level = "trace", skip_all)]
fn build_model_visible_specs(
    turn_context: &TurnContext,
    registry: &ToolRegistry,
    code_mode_tool_names: &BTreeMap<String, ToolName>,
    hosted_specs: Vec<ToolSpec>,
) -> Vec<ToolSpec> {
    let mut specs = Vec::new();
    for tool in registry.entries() {
        let exposure = tool.exposure;
        if !exposure.is_direct() {
            continue;
        }

        let tool_name = tool.runtime.tool_name();
        if is_hidden_by_code_mode_only(turn_context, &tool_name, exposure) {
            continue;
        }

        let spec = tool.runtime.spec();
        specs.push(spec_for_model_request(
            turn_context,
            exposure,
            &tool_name,
            code_mode_tool_names,
            spec,
        ));
    }
    specs.extend(hosted_specs);

    merge_into_namespaces(specs)
        .into_iter()
        .filter(|spec| {
            namespace_tools_enabled(turn_context) || !matches!(spec, ToolSpec::Namespace(_))
        })
        .collect()
}

fn spec_for_model_request(
    turn_context: &TurnContext,
    exposure: ToolExposure,
    tool_name: &ToolName,
    code_mode_tool_names: &BTreeMap<String, ToolName>,
    spec: ToolSpec,
) -> ToolSpec {
    let tool_mode = effective_tool_mode(turn_context);
    if matches!(tool_mode, ToolMode::CodeMode | ToolMode::CodeModeOnly)
        && exposure.is_available_in_code_mode()
        && !is_excluded_from_code_mode(turn_context, tool_name)
        && codex_code_mode::is_code_mode_nested_tool(spec.name())
        && code_mode_tool_names
            .get(&codex_code_mode::normalize_code_mode_identifier(
                &codex_tools::code_mode_name_for_tool_name(tool_name),
            ))
            .is_some_and(|winner| winner == tool_name)
    {
        codex_tools::augment_tool_spec_for_code_mode(spec)
    } else {
        spec
    }
}

#[instrument(level = "trace", skip_all)]
fn hosted_model_tool_specs(
    turn_context: &TurnContext,
    registered_extension_tool_names: &[ToolName],
) -> Vec<ToolSpec> {
    // Responses Lite accepts schemas for client-executed tools, not hosted Responses tools.
    if turn_context.model_info.use_responses_lite
        || crate::guardian::is_guardian_reviewer_source(&turn_context.session_source)
    {
        return Vec::new();
    }

    let mut specs = Vec::new();
    let standalone_web_search_available = standalone_web_search_enabled(turn_context)
        && registered_extension_tool_names.contains(&ToolName::namespaced("web", "run"));
    // `Some(Cached/Live/Disabled)` are the options for mode when standalone search is unavailable
    // and the provider supports hosted search. `None` prevents emitting a hosted search tool.
    let web_search_mode = (!standalone_web_search_available
        && turn_context.provider.capabilities().web_search)
        .then_some(turn_context.config.web_search_mode.value());
    let web_search_config = web_search_mode
        .as_ref()
        .and(turn_context.config.web_search_config.as_ref());
    if let Some(hosted_web_search_tool) = create_web_search_tool(WebSearchToolOptions {
        web_search_mode,
        web_search_config,
        web_search_tool_type: turn_context.model_info.web_search_tool_type,
    }) {
        specs.push(hosted_web_search_tool);
    }
    specs
}

pub(crate) fn search_tool_enabled(turn_context: &TurnContext) -> bool {
    turn_context.model_info.supports_search_tool && namespace_tools_enabled(turn_context)
}

pub(crate) fn tool_suggest_enabled(turn_context: &TurnContext) -> bool {
    let features = turn_context.config.features.get();
    features.enabled(Feature::ToolSuggest)
        && features.enabled(Feature::Apps)
        && features.enabled(Feature::Plugins)
}

fn namespace_tools_enabled(turn_context: &TurnContext) -> bool {
    turn_context.provider.capabilities().namespace_tools
}

fn multi_agent_v2_enabled(turn_context: &TurnContext) -> bool {
    turn_context.multi_agent_version == MultiAgentVersion::V2
}

fn collab_tools_enabled(turn_context: &TurnContext) -> bool {
    match turn_context.multi_agent_version {
        MultiAgentVersion::Disabled => false,
        MultiAgentVersion::V1 => !exceeds_thread_spawn_depth_limit(
            next_thread_spawn_depth(&turn_context.session_source),
            turn_context.config.agent_max_depth,
        ),
        MultiAgentVersion::V2 => {
            turn_context.session_source.get_agent_path().is_none()
                || turn_context.model_info.multi_agent_version == Some(MultiAgentVersion::V2)
        }
    }
}

fn image_generation_available(turn_context: &TurnContext) -> bool {
    if !turn_context
        .config
        .features
        .get()
        .enabled(Feature::ImageGeneration)
    {
        return false;
    }

    if turn_context
        .auth_manager
        .as_deref()
        .and_then(AuthManager::auth_cached)
        .and_then(|auth| auth.account_plan_type())
        == Some(PlanType::Free)
    {
        return false;
    }

    let capabilities = turn_context.provider.capabilities();
    if !capabilities.image_generation || !capabilities.namespace_tools {
        return false;
    }

    if !turn_context
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return false;
    }

    let provider = turn_context.provider.info();
    provider.uses_openai_actor_authorization()
        || (provider.requires_openai_auth
            && turn_context
                .auth_manager
                .as_deref()
                .is_some_and(AuthManager::current_auth_uses_codex_backend))
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
    tool_name: &ToolName,
    exposure: ToolExposure,
) -> bool {
    let tool_mode = effective_tool_mode(turn_context);
    tool_mode == ToolMode::CodeModeOnly
        && exposure.is_available_in_code_mode()
        && codex_code_mode::is_code_mode_nested_tool(&codex_tools::code_mode_name_for_tool_name(
            tool_name,
        ))
}

fn is_excluded_from_code_mode(turn_context: &TurnContext, tool_name: &ToolName) -> bool {
    let tool_name = tool_name.clone().with_default_namespace();
    tool_name.namespace.as_ref().is_some_and(|namespace| {
        turn_context
            .config
            .code_mode
            .excluded_tool_namespaces
            .contains(namespace)
    })
}

fn register_code_mode_executors(
    turn_context: &TurnContext,
    registry: &mut ToolRegistry,
) -> BTreeMap<String, ToolName> {
    let tool_mode = effective_tool_mode(turn_context);
    if !matches!(tool_mode, ToolMode::CodeMode | ToolMode::CodeModeOnly) {
        return BTreeMap::new();
    }

    let mut code_mode_tool_names = BTreeMap::new();
    let mut code_mode_nested_tool_specs = Vec::new();
    let mut exec_prompt_tool_specs = Vec::new();
    let mut deferred_exec_prompt_tool_specs = Vec::new();
    let mut included_deferred_mcp_output_schema = false;
    let deferred_tools_guidance_enabled = search_tool_enabled(turn_context);
    for tool in registry.entries() {
        let exposure = tool.exposure;
        if !exposure.is_available_in_code_mode() {
            continue;
        }

        let tool_name = tool.runtime.tool_name();
        if is_excluded_from_code_mode(turn_context, &tool_name) {
            continue;
        }

        let immutable_spec = tool.runtime.immutable_spec();
        let cached_runtime = immutable_spec.map(|_| Arc::clone(&tool.runtime));
        let spec = immutable_spec
            .map(Arc::clone)
            .unwrap_or_else(|| Arc::new(tool.runtime.spec()));
        // Derive the name without serializing and augmenting every tool schema.
        let code_mode_name = match spec.as_ref() {
            ToolSpec::Function(_) | ToolSpec::Freeform(_) => {
                codex_tools::code_mode_name_for_tool_name(&tool_name)
            }
            ToolSpec::Namespace(namespace) if !namespace.tools.is_empty() => {
                codex_tools::code_mode_name_for_tool_name(&tool_name)
            }
            ToolSpec::Namespace(_) | ToolSpec::ToolSearch { .. } | ToolSpec::WebSearch { .. } => {
                continue;
            }
        };
        if !codex_code_mode::is_code_mode_nested_tool(&code_mode_name) {
            continue;
        }
        match code_mode_tool_names.entry(codex_code_mode::normalize_code_mode_identifier(
            &code_mode_name,
        )) {
            Entry::Vacant(entry) => {
                entry.insert(tool_name);
            }
            Entry::Occupied(_) => {
                tracing::warn!(
                    tool_name = %tool_name,
                    code_mode_name = %code_mode_name,
                    "skipping tool with a duplicate normalized code-mode name"
                );
                continue;
            }
        }

        if exposure == ToolExposure::Deferred {
            if deferred_tools_guidance_enabled
                && (cached_runtime.is_none() || !included_deferred_mcp_output_schema)
            {
                if cached_runtime.is_some() {
                    included_deferred_mcp_output_schema = true;
                }
                deferred_exec_prompt_tool_specs.push(Arc::clone(&spec));
            }
        } else {
            exec_prompt_tool_specs.push(spec.as_ref().clone());
        }
        code_mode_nested_tool_specs.push((spec, cached_runtime));
    }

    let namespace_descriptions = code_mode_namespace_descriptions(&exec_prompt_tool_specs);
    let mut enabled_tools =
        collect_code_mode_exec_prompt_tool_definitions(exec_prompt_tool_specs.iter());
    let deferred_tools = collect_code_mode_exec_prompt_tool_definitions(
        deferred_exec_prompt_tool_specs.iter().map(Arc::as_ref),
    );
    enabled_tools
        .sort_by(|left, right| compare_code_mode_tools(left, right, &namespace_descriptions));
    let execute_handler = CodeModeExecuteHandler::new(
        create_code_mode_tool(
            &enabled_tools,
            &deferred_tools,
            &namespace_descriptions,
            turn_context.config.code_mode.default_exec_yield_time_ms,
            tool_mode == ToolMode::CodeModeOnly,
            if unified_image_budget_enabled(&turn_context.config.features, &turn_context.model_info)
            {
                codex_code_mode::ImageDetailVisibility::Hidden
            } else {
                codex_code_mode::ImageDetailVisibility::Visible
            },
        ),
        code_mode_nested_tool_specs,
    );

    registry.prepend_trusted(Arc::new(CodeModeWaitHandler));
    registry.prepend_trusted(Arc::new(execute_handler));

    code_mode_tool_names
}

#[instrument(level = "trace", skip_all, fields(tool_spec_count = specs.len()))]
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

        namespace.tools.sort_by(|left, right| {
            let left_name = match left {
                ResponsesApiNamespaceTool::Function(tool) => &tool.name,
                ResponsesApiNamespaceTool::Custom(tool) => &tool.name,
            };
            let right_name = match right {
                ResponsesApiNamespaceTool::Function(tool) => &tool.name,
                ResponsesApiNamespaceTool::Custom(tool) => &tool.name,
            };
            left_name.cmp(right_name)
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

#[instrument(level = "trace", skip_all)]
fn add_core_tool_sources(context: &CoreToolPlanContext<'_>, registry: &mut ToolRegistry) {
    // Guardian reviewers receive only `exec_command`, `write_stdin`, and `view_image`
    // when a managed sandbox can enforce the parent's filesystem restrictions;
    // all general tool sources stay excluded.
    if crate::guardian::is_guardian_reviewer_source(&context.turn_context.session_source) {
        let turn_context = context.turn_context;
        if !matches!(
            turn_context.permission_profile(),
            PermissionProfile::Managed { .. }
        ) {
            return;
        }
        let environment_mode = tool_environment_mode(context.environments);
        if environment_mode.has_environment() {
            let include_environment_id = matches!(environment_mode, ToolEnvironmentMode::Multiple);
            registry.add(ExecCommandHandler::new(ExecCommandHandlerOptions {
                allow_login_shell: any_environment_allows_login_shell(context.environments),
                exec_permission_approvals_enabled: false,
                include_environment_id,
                include_shell_parameter: unified_exec_should_include_shell_parameter(
                    turn_context,
                    context.environments,
                ),
            }));
            registry.add(WriteStdinHandler);
            if turn_context.config.features.enabled(Feature::ViewImage) {
                registry.add(ViewImageHandler::new(ViewImageToolOptions {
                    can_request_original_image_detail: can_request_original_image_detail(
                        &turn_context.model_info,
                    ),
                    unified_image_budget: unified_image_budget_enabled(
                        &turn_context.config.features,
                        &turn_context.model_info,
                    ),
                    include_environment_id,
                }));
            }
        }
        return;
    }

    add_shell_tools(context, registry);
    add_mcp_resource_tools(context, registry);
    add_core_utility_tools(context, registry);
    add_collaboration_tools(context, registry);
}

fn standalone_web_search_enabled(turn_context: &TurnContext) -> bool {
    namespace_tools_enabled(turn_context)
        && turn_context.provider.capabilities().web_search
        && (turn_context.model_info.use_responses_lite
            || turn_context
                .config
                .features
                .get()
                .enabled(Feature::StandaloneWebSearch))
}

fn tool_environment_mode(environments: &TurnEnvironmentSnapshot) -> ToolEnvironmentMode {
    ToolEnvironmentMode::from_count(environments.turn_environments().count())
}

fn any_environment_allows_login_shell(environments: &TurnEnvironmentSnapshot) -> bool {
    environments
        .turn_environments()
        .any(|environment| environment.config().allow_login_shell)
}

#[instrument(level = "trace", skip_all)]
fn add_shell_tools(context: &CoreToolPlanContext<'_>, registry: &mut ToolRegistry) {
    let turn_context = context.turn_context;
    let features = turn_context.config.features.get();
    let environment_mode = tool_environment_mode(context.environments);
    if !environment_mode.has_environment()
        || !features.enabled(Feature::ShellTool)
        || !features.enabled(Feature::UnifiedExec)
        || matches!(
            turn_context.model_info.shell_type,
            ConfigShellToolType::Disabled
        )
    {
        return;
    }

    let allow_login_shell = any_environment_allows_login_shell(context.environments);
    let exec_permission_approvals_enabled = features.enabled(Feature::ExecPermissionApprovals);
    let include_environment_id = matches!(environment_mode, ToolEnvironmentMode::Multiple);
    registry.add(ExecCommandHandler::new(ExecCommandHandlerOptions {
        allow_login_shell,
        exec_permission_approvals_enabled,
        include_environment_id,
        include_shell_parameter: unified_exec_should_include_shell_parameter(
            turn_context,
            context.environments,
        ),
    }));
    registry.add(WriteStdinHandler);
}

fn unified_exec_should_include_shell_parameter(
    turn_context: &TurnContext,
    environments: &TurnEnvironmentSnapshot,
) -> bool {
    !matches!(
        &turn_context.unified_exec_shell_mode,
        UnifiedExecShellMode::ZshFork(_)
    ) || environments
        .turn_environments()
        .any(|environment| environment.environment.is_remote())
}

#[instrument(level = "trace", skip_all)]
fn add_mcp_resource_tools(context: &CoreToolPlanContext<'_>, registry: &mut ToolRegistry) {
    if context.mcp.has_servers() {
        registry.add(ListMcpResourcesHandler);
        registry.add(ListMcpResourceTemplatesHandler);
        registry.add(ReadMcpResourceHandler);
    }
}

#[instrument(level = "trace", skip_all)]
fn add_core_utility_tools(context: &CoreToolPlanContext<'_>, registry: &mut ToolRegistry) {
    let turn_context = context.turn_context;
    let features = turn_context.config.features.get();
    let environment_mode = tool_environment_mode(context.environments);

    if turn_context.config.update_plan_enabled {
        registry.add(PlanHandler);
    }

    if features.enabled(Feature::DeferredExecutor) {
        registry.add(
            context
                .wait_for_environment_tool_config
                .map(Arc::as_ref)
                .map_or_else(
                    WaitForEnvironmentHandler::default,
                    WaitForEnvironmentHandler::new,
                ),
        );
    }

    if turn_context.config.experimental_request_user_input_enabled {
        registry.add_with_exposure(
            RequestUserInputHandler {
                available_modes: request_user_input_available_modes(features),
            },
            ToolExposure::DirectModelOnly,
        );
    }

    if !turn_context.session_source.is_non_root_agent()
        && turn_context
            .model_info
            .experimental_supported_tools
            .iter()
            .any(|tool| tool == "send_user_message_async")
    {
        registry.add_with_exposure(SendUserMessageAsyncHandler, ToolExposure::DirectModelOnly);
    }

    if environment_mode.has_environment() && features.enabled(Feature::RequestPermissionsTool) {
        registry.add(RequestPermissionsHandler);
    }

    if features.enabled(Feature::TokenBudget) {
        registry.add_with_exposure(NewContextWindowHandler, ToolExposure::DirectModelOnly);
        registry.add(GetContextRemainingHandler);
    }

    if features.enabled(Feature::CurrentTimeReminder) {
        registry.add(CurrentTimeHandler);
        if turn_context
            .config
            .current_time_reminder
            .as_ref()
            .is_some_and(|config| config.sleep_tool)
        {
            registry.add(SleepHandler);
        }
    }

    if tool_suggest_enabled(turn_context)
        && let Some(candidates) = context
            .tool_suggest_candidates
            .filter(|candidates| !candidates.tools.is_empty())
    {
        if candidates.presentation == crate::tools::router::ToolSuggestPresentation::ListTool {
            registry.add(ListAvailablePluginsToInstallHandler::new(
                collect_request_plugin_install_entries(&candidates.tools),
            ));
        }
        registry.add(RequestPluginInstallHandler::new(
            candidates.tools.clone(),
            candidates.presentation,
        ));
    }

    if environment_mode.has_environment() && turn_context.model_info.apply_patch_tool_type.is_some()
    {
        let include_environment_id = matches!(environment_mode, ToolEnvironmentMode::Multiple);
        registry.add(ApplyPatchHandler::new(include_environment_id));
    }

    if turn_context
        .model_info
        .experimental_supported_tools
        .iter()
        .any(|tool| tool == "test_sync_tool")
    {
        registry.add(TestSyncHandler);
    }

    if environment_mode.has_environment() && features.enabled(Feature::ViewImage) {
        let include_environment_id = matches!(environment_mode, ToolEnvironmentMode::Multiple);
        registry.add(ViewImageHandler::new(ViewImageToolOptions {
            can_request_original_image_detail: can_request_original_image_detail(
                &turn_context.model_info,
            ),
            unified_image_budget: unified_image_budget_enabled(
                &turn_context.config.features,
                &turn_context.model_info,
            ),
            include_environment_id,
        }));
    }
}

#[instrument(level = "trace", skip_all)]
fn add_collaboration_tools(context: &CoreToolPlanContext<'_>, registry: &mut ToolRegistry) {
    let turn_context = context.turn_context;
    if collab_tools_enabled(turn_context) {
        if multi_agent_v2_enabled(turn_context) {
            let exposure = if turn_context.config.multi_agent_v2.non_code_mode_only {
                ToolExposure::DirectModelOnly
            } else {
                ToolExposure::Direct
            };
            let tool_namespace = namespace_tools_enabled(turn_context)
                .then_some(turn_context.config.multi_agent_v2.tool_namespace.as_deref())
                .flatten();
            let agent_type_description =
                agent_type_description(turn_context, context.default_agent_type_description);
            let hide_spawn_agent_metadata =
                turn_context.config.multi_agent_v2.hide_spawn_agent_metadata;
            registry.register_trusted_with_exposure(
                multi_agent_v2_handler(
                    SpawnAgentHandlerV2::new(SpawnAgentToolOptions {
                        available_models: turn_context.available_models.clone(),
                        agent_type_description,
                        expose_agent_type: !turn_context.config.agent_roles.is_empty(),
                        hide_agent_type_model_reasoning: hide_spawn_agent_metadata,
                        expose_spawn_agent_model_overrides: turn_context
                            .config
                            .multi_agent_v2
                            .expose_spawn_agent_model_overrides,
                        multi_agent_version: turn_context.multi_agent_version,
                        usage_hint_text: turn_context.config.multi_agent_v2.usage_hint_text.clone(),
                    }),
                    tool_namespace,
                ),
                exposure,
            );
            registry.register_trusted_with_exposure(
                multi_agent_v2_handler(SendMessageHandlerV2, tool_namespace),
                exposure,
            );
            registry.register_trusted_with_exposure(
                multi_agent_v2_handler(FollowupTaskHandlerV2, tool_namespace),
                exposure,
            );
            if turn_context.config.multi_agent_v2.wait_agent_enabled {
                registry.register_trusted_with_exposure(
                    multi_agent_v2_handler(
                        WaitAgentHandlerV2::new(context.wait_agent_timeouts),
                        tool_namespace,
                    ),
                    exposure,
                );
            }
            registry.register_trusted_with_exposure(
                multi_agent_v2_handler(InterruptAgentHandler, tool_namespace),
                exposure,
            );
            registry.register_trusted_with_exposure(
                multi_agent_v2_handler(ListAgentsHandlerV2, tool_namespace),
                exposure,
            );
        } else {
            let agent_type_description =
                agent_type_description(turn_context, context.default_agent_type_description);
            let exposure = if search_tool_enabled(turn_context) {
                ToolExposure::Deferred
            } else {
                ToolExposure::Direct
            };
            registry.add_with_exposure(
                SpawnAgentHandler::new(SpawnAgentToolOptions {
                    available_models: turn_context.available_models.clone(),
                    agent_type_description,
                    expose_agent_type: !turn_context.config.agent_roles.is_empty(),
                    hide_agent_type_model_reasoning: false,
                    expose_spawn_agent_model_overrides: true,
                    multi_agent_version: turn_context.multi_agent_version,
                    usage_hint_text: turn_context.config.multi_agent_v2.usage_hint_text.clone(),
                }),
                exposure,
            );
            registry.add_with_exposure(SendInputHandler, exposure);
            registry.add_with_exposure(ResumeAgentHandler, exposure);
            registry
                .add_with_exposure(WaitAgentHandler::new(context.wait_agent_timeouts), exposure);
            registry.add_with_exposure(CloseAgentHandler, exposure);
        }
    }
}

#[instrument(level = "trace", skip_all, fields(dynamic_tool_count = dynamic_tools.len()))]
fn append_dynamic_tool_runtimes(dynamic_tools: &[DynamicToolSpec], registry: &mut ToolRegistry) {
    for spec in dynamic_tools {
        match spec {
            DynamicToolSpec::Function(tool) => {
                let Some(handler) = DynamicToolHandler::new(tool) else {
                    tracing::error!(
                        "Failed to convert dynamic tool {:?} to OpenAI tool",
                        tool.name
                    );
                    continue;
                };
                registry.register_external(Arc::new(handler));
            }
            DynamicToolSpec::Namespace(namespace) => {
                for tool in &namespace.tools {
                    let DynamicToolNamespaceTool::Function(tool) = tool;
                    let Some(handler) = DynamicToolHandler::new_in_namespace(namespace, tool)
                    else {
                        tracing::error!(
                            "Failed to convert dynamic tool {:?}.{:?} to OpenAI tool",
                            namespace.name,
                            tool.name
                        );
                        continue;
                    };
                    registry.register_external(Arc::new(handler));
                }
            }
        }
    }
}

#[instrument(level = "trace", skip_all)]
fn append_tool_search_executor(
    turn_context: &TurnContext,
    registry: &mut ToolRegistry,
    tool_search_handler_cache: &ToolSearchHandlerCache,
) {
    let source_listing = if turn_context
        .config
        .features
        .enabled(Feature::DeferredToolWorldState)
    {
        ToolSearchSourceListing::Omit
    } else {
        ToolSearchSourceListing::Include
    };
    let handler = tool_search_handler_cache.get_or_build(registry, source_listing);
    registry.register_trusted(handler);
}

fn append_extension_tool_executors(
    turn_context: &TurnContext,
    executors: impl IntoIterator<Item = Arc<dyn ToolExecutor<ExtensionToolCall>>>,
    registry: &mut ToolRegistry,
) -> Option<ToolName> {
    let standalone_web_search_enabled = standalone_web_search_enabled(turn_context);
    let web_search_mode_on = turn_context.config.web_search_mode.value() != WebSearchMode::Disabled;
    let mut standalone_web_search_tool = None;

    for executor in executors {
        let tool_name = executor.tool_name();
        let is_standalone_web_search = tool_name == ToolName::namespaced("web", "run");
        if is_standalone_web_search && (!standalone_web_search_enabled || !web_search_mode_on) {
            continue;
        }
        if tool_name == ToolName::namespaced(IMAGE_GEN_NAMESPACE, IMAGEGEN_TOOL_NAME)
            && !image_generation_available(turn_context)
        {
            continue;
        }
        let runtime = Arc::new(ExtensionToolAdapter::new(executor));
        if registry.register_external(runtime) && is_standalone_web_search {
            standalone_web_search_tool = Some(tool_name);
        }
    }

    standalone_web_search_tool
}

fn multi_agent_v2_handler(
    handler: impl CoreToolRuntime + 'static,
    namespace: Option<&str>,
) -> Arc<dyn CoreToolRuntime> {
    match namespace {
        Some(namespace) => Arc::new(MultiAgentV2NamespaceOverride {
            handler: Arc::new(handler),
            namespace: namespace.to_string(),
        }),
        None => Arc::new(handler),
    }
}

struct MultiAgentV2NamespaceOverride {
    handler: Arc<dyn CoreToolRuntime>,
    namespace: String,
}

impl ToolExecutor<ToolInvocation> for MultiAgentV2NamespaceOverride {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(self.namespace.clone(), self.handler.tool_name().name)
    }

    fn spec(&self) -> ToolSpec {
        match self.handler.spec() {
            ToolSpec::Function(tool) => ToolSpec::Namespace(ResponsesApiNamespace {
                name: self.namespace.clone(),
                description: MULTI_AGENT_V2_NAMESPACE_DESCRIPTION.to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(tool)],
            }),
            spec => spec,
        }
    }

    fn exposure(&self) -> ToolExposure {
        self.handler.exposure()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.handler.supports_parallel_tool_calls()
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        self.handler.search_info()
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        self.handler.handle(invocation)
    }
}

impl CoreToolRuntime for MultiAgentV2NamespaceOverride {
    fn wait_until_ready<'a>(&'a self, session: &'a Arc<Session>) -> Option<BoxFuture<'a, ()>> {
        self.handler.wait_until_ready(session)
    }

    fn matches_kind(&self, payload: &crate::tools::context::ToolPayload) -> bool {
        self.handler.matches_kind(payload)
    }

    fn create_diff_consumer(
        &self,
    ) -> Option<Box<dyn crate::tools::registry::ToolArgumentDiffConsumer>> {
        self.handler.create_diff_consumer()
    }
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
