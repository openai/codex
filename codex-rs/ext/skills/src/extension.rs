use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use codex_analytics::AnalyticsEventsClient;
use codex_analytics::InvocationType;
use codex_analytics::SkillInvocation;
use codex_analytics::build_track_events_context;
use codex_connectors::ExplicitConnectorMentions;
use codex_core_plugins::PluginLoadOutcome;
use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::SkillMetadata;
use codex_core_skills::SkillsService;
use codex_core_skills::detect_implicit_skill_invocation_for_command;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContextContributionContext;
use codex_extension_api::ContextContributor;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolDispatchInput;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputContributor;
use codex_mcp::McpResourceClient;
use codex_mcp::McpServerDependencies;
use codex_mcp::McpServerDependency;
use codex_otel::SessionTelemetry;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WarningEvent;
use codex_tools::ToolPayload;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::tool_mentions::ToolMentionKind;
use codex_utils_plugins::tool_mentions::app_id_from_path;
use codex_utils_plugins::tool_mentions::extract_tool_mentions;
use codex_utils_plugins::tool_mentions::tool_kind_for_path;
use serde::Deserialize;

use crate::SkillsExtensionConfig;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillSourceKind;
use crate::fragments::SkillInstructions;
use crate::provider::HostSkillProvider;
use crate::provider::SkillListQuery;
use crate::provider::SkillReadRequest;
use crate::render::MAX_SKILL_NAME_BYTES;
use crate::render::MAX_SKILL_PATH_BYTES;
use crate::render::available_skills_fragment;
use crate::render::truncate_main_prompt_contents;
use crate::render::truncate_utf8_to_bytes;
use crate::selection::collect_explicit_skill_mentions;
use crate::sources::SkillProviders;
use crate::state::ImplicitSkillInvocationState;
use crate::state::SkillsThreadState;
use crate::state::SkillsTurnState;
use crate::tools::skill_tools;

struct SkillsExtension<C> {
    providers: SkillProviders,
    host_provider: Option<Arc<HostSkillProvider>>,
    event_sink: Arc<dyn ExtensionEventSink>,
    config_from_host: Arc<dyn Fn(&C) -> SkillsExtensionConfig + Send + Sync>,
}

impl<C> ThreadLifecycleContributor<C> for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_thread_start<'a>(&'a self, input: ThreadStartInput<'a, C>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let selected_roots = input
                .thread_store
                .get::<Vec<SelectedCapabilityRoot>>()
                .map(|selected_roots| selected_roots.as_ref().clone())
                .unwrap_or_default();
            let orchestrator_skills_enabled = !input
                .environments
                .iter()
                .any(|environment| environment.environment_id == LOCAL_ENVIRONMENT_ID);
            input.thread_store.insert(SkillsThreadState::new(
                (self.config_from_host)(input.config),
                selected_roots,
                orchestrator_skills_enabled,
                input.session_source.restriction_product(),
            ));
        })
    }
}

impl<C> ConfigContributor<C> for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &C,
        new_config: &C,
    ) {
        let next_config = (self.config_from_host)(new_config);
        if let Some(state) = thread_store.get::<SkillsThreadState>() {
            state.set_config(next_config);
        } else {
            let orchestrator_skills_enabled = true;
            thread_store.insert(SkillsThreadState::new(
                next_config,
                Vec::new(),
                orchestrator_skills_enabled,
                /*restriction_product*/ None,
            ));
        }
    }
}

impl<C> ContextContributor for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn contribute<'a>(
        &'a self,
        context: ContextContributionContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<PromptFragment>> + Send + 'a>> {
        Box::pin(async move {
            let session_store = context.session_store;
            let thread_store = context.thread_store;
            let turn_store = context.turn_store;
            let Some(thread_state) = thread_store.get::<SkillsThreadState>() else {
                return Vec::new();
            };
            let config = thread_state.config();
            if !config.include_instructions {
                return Vec::new();
            }
            let host_snapshot = self.host_snapshot(turn_store, &thread_state).await;
            let catalog = self
                .list_skills(
                    SkillListQuery {
                        turn_id: turn_store.level_id().to_string(),
                        executor_roots: thread_state.selected_roots().to_vec(),
                        host_snapshot: host_snapshot.clone(),
                        include_host_skills: true,
                        include_bundled_skills: config.bundled_skills_enabled,
                        include_orchestrator_skills: thread_state.orchestrator_skills_enabled(),
                        mcp_resources: session_store.get::<McpResourceClient>(),
                    },
                    &thread_state,
                )
                .await;
            for warning in &catalog.warnings {
                self.emit_warning(
                    context.thread_id,
                    context.turn_store.level_id(),
                    warning.clone(),
                );
            }
            let session_telemetry = turn_store.get::<SessionTelemetry>();
            let Some((fragment, warning)) = available_skills_fragment(
                &catalog,
                host_snapshot.as_deref(),
                context.model_context_window,
                session_telemetry.as_deref(),
            ) else {
                return Vec::new();
            };
            if let Some(warning) = warning {
                self.emit_warning(context.thread_id, context.turn_store.level_id(), warning);
            }
            vec![PromptFragment::developer_capability(fragment.render())]
        })
    }
}

impl<C> ToolContributor for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn tools(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        let Some(thread_state) = thread_store.get::<SkillsThreadState>() else {
            return Vec::new();
        };
        if !self.providers.has_orchestrator_provider()
            || !thread_state.orchestrator_skills_enabled()
        {
            return Vec::new();
        }

        skill_tools(
            self.providers.clone(),
            session_store.get::<McpResourceClient>(),
            thread_state,
        )
    }
}

impl<C> TurnInputContributor for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn contribute<'a>(
        &'a self,
        input: TurnInputContext,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
        turn_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<Box<dyn ContextualUserFragment + Send>>> {
        Box::pin(async move {
            let Some(thread_state) = thread_store.get::<SkillsThreadState>() else {
                return Vec::new();
            };

            let config = thread_state.config();
            let host_snapshot = self.host_snapshot(turn_store, &thread_state).await;
            let query = SkillListQuery {
                turn_id: input.turn_id.clone(),
                executor_roots: thread_state.selected_roots().to_vec(),
                host_snapshot: host_snapshot.clone(),
                include_host_skills: true,
                include_bundled_skills: config.bundled_skills_enabled,
                include_orchestrator_skills: thread_state.orchestrator_skills_enabled(),
                mcp_resources: session_store.get::<McpResourceClient>(),
            };
            let catalog = self.list_skills(query, &thread_state).await;
            for warning in &catalog.warnings {
                self.emit_warning(input.thread_id, &input.turn_id, warning.clone());
            }

            let selected_entries = collect_explicit_skill_mentions(
                &input.user_input,
                &catalog,
                &input.reserved_plain_tool_names,
            );
            let mut fragments: Vec<Box<dyn ContextualUserFragment + Send>> = Vec::new();
            let skill_names = catalog
                .entries
                .iter()
                .filter(|entry| entry.enabled)
                .map(|entry| entry.name.to_ascii_lowercase())
                .collect::<HashSet<_>>();
            let mut mcp_dependencies = McpServerDependencies::default();
            for entry in &selected_entries {
                for dependency in entry
                    .dependencies
                    .iter()
                    .flat_map(|dependencies| &dependencies.tools)
                    .filter(|dependency| dependency.r#type.eq_ignore_ascii_case("mcp"))
                {
                    mcp_dependencies.push(McpServerDependency {
                        source_name: entry.name.clone(),
                        name: dependency.value.clone(),
                        transport: dependency.transport.clone(),
                        command: dependency.command.clone(),
                        url: dependency.url.clone(),
                    });
                }
            }
            if !mcp_dependencies.is_empty() {
                turn_store.insert(mcp_dependencies);
            }

            let mut warnings = catalog.warnings.clone();
            let mut main_prompts_injected = false;
            let mut connector_mentions = ExplicitConnectorMentions::default();
            let mut skill_invocations = Vec::new();
            for entry in &selected_entries {
                match self
                    .read_main_prompt(entry, host_snapshot.clone(), session_store, &thread_state)
                    .await
                {
                    Ok(read_result) => {
                        let (contents, truncated) =
                            truncate_main_prompt_contents(read_result.contents.as_str());
                        let tool_mentions = extract_tool_mentions(&contents);
                        for path in tool_mentions.paths() {
                            if tool_kind_for_path(path) == ToolMentionKind::App
                                && let Some(connector_id) = app_id_from_path(path)
                            {
                                connector_mentions.insert_connector_id(connector_id);
                            }
                        }
                        for name in tool_mentions.plain_names() {
                            if !skill_names.contains(&name.to_ascii_lowercase()) {
                                connector_mentions.insert_plain_name(name);
                            }
                        }
                        if let Some(skill) = host_skill_for_entry(host_snapshot.as_deref(), entry) {
                            if let Some(telemetry) = turn_store.get::<SessionTelemetry>() {
                                telemetry.counter(
                                    "codex.skill.injected",
                                    /*inc*/ 1,
                                    &[("status", "ok"), ("skill", skill.name.as_str())],
                                );
                            }
                            skill_invocations.push(SkillInvocation {
                                skill_name: skill.name.clone(),
                                skill_scope: skill.scope,
                                skill_path: skill.path_to_skills_md.to_path_buf(),
                                plugin_id: skill.plugin_id.clone(),
                                invocation_type: InvocationType::Explicit,
                            });
                        }
                        if truncated {
                            let warning = format!(
                                "Skill `{}` exceeded the main prompt context limit and was truncated.",
                                entry.name
                            );
                            self.emit_warning(input.thread_id, &input.turn_id, warning.clone());
                            warnings.push(warning);
                        }
                        let fragment = SkillInstructions {
                            name: truncate_utf8_to_bytes(&entry.name, MAX_SKILL_NAME_BYTES).0,
                            path: truncate_utf8_to_bytes(
                                entry.rendered_path(),
                                MAX_SKILL_PATH_BYTES,
                            )
                            .0,
                            contents,
                        };
                        fragments.push(Box::new(fragment));
                        main_prompts_injected = true;
                    }
                    Err(message) => {
                        if let Some(skill) = host_skill_for_entry(host_snapshot.as_deref(), entry)
                            && let Some(telemetry) = turn_store.get::<SessionTelemetry>()
                        {
                            telemetry.counter(
                                "codex.skill.injected",
                                /*inc*/ 1,
                                &[("status", "error"), ("skill", skill.name.as_str())],
                            );
                        }
                        let warning = format!("Failed to load skill `{}`: {message}", entry.name);
                        self.emit_warning(input.thread_id, &input.turn_id, warning.clone());
                        warnings.push(warning);
                    }
                }
            }
            if !connector_mentions.is_empty() {
                turn_store.insert(connector_mentions);
            }
            if let Some(analytics) = session_store.get::<AnalyticsEventsClient>() {
                analytics.track_skill_invocations(
                    build_track_events_context(
                        input.model.clone(),
                        thread_store.level_id().to_string(),
                        input.turn_id.clone(),
                    ),
                    skill_invocations,
                );
            }

            turn_store.insert(SkillsTurnState {
                catalog,
                selected_entries,
                warnings,
                main_prompts_injected,
            });
            turn_store.insert(ImplicitSkillInvocationState {
                model: input.model,
                environment_cwds: input
                    .environments
                    .iter()
                    .map(|environment| {
                        (environment.environment_id.clone(), environment.cwd.clone())
                    })
                    .collect(),
                primary_environment_id: input
                    .environments
                    .iter()
                    .find(|environment| environment.is_primary)
                    .map(|environment| environment.environment_id.clone()),
                seen_skills: Default::default(),
            });

            fragments
        })
    }
}

impl<C> SkillsExtension<C> {
    async fn host_snapshot(
        &self,
        turn_store: &ExtensionData,
        thread_state: &SkillsThreadState,
    ) -> Option<Arc<HostSkillsSnapshot>> {
        if let Some(snapshot) = turn_store.get::<HostSkillsSnapshot>() {
            return Some(snapshot);
        }
        let provider = self.host_provider.as_ref()?;
        let config = thread_state.config();
        let host_config = config.host.as_ref()?;
        let plugins = turn_store.get::<PluginLoadOutcome>();
        let fs = turn_store
            .get::<Arc<dyn ExecutorFileSystem>>()
            .map(|fs| Arc::clone(fs.as_ref()));
        let snapshot = provider
            .snapshot_for_turn(
                host_config,
                thread_state.restriction_product(),
                plugins.as_deref(),
                fs,
            )
            .await;
        turn_store.insert(snapshot.clone());
        turn_store.get::<HostSkillsSnapshot>()
    }

    async fn list_skills(
        &self,
        mut query: SkillListQuery,
        thread_state: &SkillsThreadState,
    ) -> SkillCatalog {
        let include_orchestrator_skills = query.include_orchestrator_skills;
        let orchestrator_query = query.clone();
        let mcp_resources = orchestrator_query.mcp_resources.clone();
        query.include_orchestrator_skills = false;

        let mut catalog = self.providers.list_for_turn(query).await;
        if include_orchestrator_skills {
            let orchestrator_catalog = thread_state
                .orchestrator_catalog_snapshot(
                    mcp_resources.as_deref(),
                    self.providers
                        .list_orchestrator_for_turn(orchestrator_query),
                )
                .await;
            catalog.extend(orchestrator_catalog);
        }
        catalog
    }

    async fn read_main_prompt(
        &self,
        entry: &SkillCatalogEntry,
        host_snapshot: Option<Arc<HostSkillsSnapshot>>,
        session_store: &ExtensionData,
        thread_state: &SkillsThreadState,
    ) -> Result<SkillReadResult, String> {
        thread_state
            .read_skill(
                &self.providers,
                SkillReadRequest {
                    authority: entry.authority.clone(),
                    package: entry.id.clone(),
                    resource: entry.main_prompt.clone(),
                    host_snapshot,
                    mcp_resources: session_store.get::<McpResourceClient>(),
                },
            )
            .await
            .map_err(|err| err.message)
    }

    fn emit_warning(&self, thread_id: codex_protocol::ThreadId, turn_id: &str, message: String) {
        self.event_sink.emit(
            thread_id,
            Event {
                id: turn_id.to_string(),
                msg: EventMsg::Warning(WarningEvent { message }),
            },
        );
    }
}

impl<C> ToolLifecycleContributor for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_tool_dispatch<'a>(&'a self, input: ToolDispatchInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            let Some((command, workdir)) = implicit_command_invocation(&input) else {
                return;
            };
            let Some(snapshot) = input.turn_store.get::<HostSkillsSnapshot>() else {
                return;
            };
            let Some(skill) = detect_implicit_skill_invocation_for_command(
                snapshot.outcome(),
                &command,
                &workdir,
            ) else {
                return;
            };
            let Some(state) = input.turn_store.get::<ImplicitSkillInvocationState>() else {
                return;
            };
            let skill_scope = match skill.scope {
                codex_protocol::protocol::SkillScope::User => "user",
                codex_protocol::protocol::SkillScope::Repo => "repo",
                codex_protocol::protocol::SkillScope::System => "system",
                codex_protocol::protocol::SkillScope::Admin => "admin",
            };
            let skill_path = skill.path_to_skills_md.to_string_lossy();
            let seen_key = format!("{skill_scope}:{skill_path}:{}", skill.name);
            if !state
                .seen_skills
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(seen_key)
            {
                return;
            }
            if let Some(telemetry) = input.turn_store.get::<SessionTelemetry>() {
                telemetry.counter(
                    "codex.skill.injected",
                    /*inc*/ 1,
                    &[
                        ("status", "ok"),
                        ("skill", skill.name.as_str()),
                        ("invoke_type", "implicit"),
                    ],
                );
            }
            if let Some(analytics) = input.session_store.get::<AnalyticsEventsClient>() {
                analytics.track_skill_invocations(
                    build_track_events_context(
                        state.model.clone(),
                        input.thread_store.level_id().to_string(),
                        input.turn_id.to_string(),
                    ),
                    vec![SkillInvocation {
                        skill_name: skill.name,
                        skill_scope: skill.scope,
                        skill_path: skill.path_to_skills_md.to_path_buf(),
                        plugin_id: skill.plugin_id,
                        invocation_type: InvocationType::Implicit,
                    }],
                );
            }
        })
    }
}

#[derive(Deserialize)]
struct ExecCommandInvocation {
    cmd: String,
    workdir: Option<String>,
    environment_id: Option<String>,
}

fn implicit_command_invocation(input: &ToolDispatchInput<'_>) -> Option<(String, AbsolutePathBuf)> {
    if input.tool_name.namespace.is_some() {
        return None;
    }
    let ToolPayload::Function { arguments } = input.payload else {
        return None;
    };
    let state = input.turn_store.get::<ImplicitSkillInvocationState>()?;
    let (command, workdir, environment_id) = match input.tool_name.name.as_str() {
        "exec_command" => {
            let invocation = serde_json::from_str::<ExecCommandInvocation>(arguments).ok()?;
            (
                invocation.cmd,
                invocation.workdir,
                invocation.environment_id,
            )
        }
        "shell_command" => {
            let invocation = serde_json::from_str::<ShellCommandToolCallParams>(arguments).ok()?;
            (invocation.command, invocation.workdir, None)
        }
        _ => return None,
    };
    let environment_id = environment_id.or_else(|| state.primary_environment_id.clone())?;
    let base = state.environment_cwds.get(&environment_id)?;
    let workdir = workdir.map(PathBuf::from).unwrap_or_else(|| base.clone());
    let workdir = if workdir.is_absolute() {
        workdir
    } else {
        base.join(workdir)
    };
    AbsolutePathBuf::try_from(workdir)
        .ok()
        .map(|workdir| (command, workdir))
}

fn host_skill_for_entry<'a>(
    host_snapshot: Option<&'a HostSkillsSnapshot>,
    entry: &SkillCatalogEntry,
) -> Option<&'a SkillMetadata> {
    if entry.authority.kind != SkillSourceKind::Host {
        return None;
    }
    host_snapshot?.outcome().skills.iter().find(|skill| {
        let path = skill.path_to_skills_md.to_string_lossy();
        path == entry.main_prompt.as_str() || path.replace('\\', "/") == entry.main_prompt.as_str()
    })
}

pub fn install<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
    config_from_host: impl Fn(&C) -> SkillsExtensionConfig + Send + Sync + 'static,
) where
    C: Send + Sync + 'static,
{
    let host_provider = Arc::new(HostSkillProvider::new());
    install_inner(
        registry,
        SkillProviders::new().with_host_provider(host_provider.clone()),
        Some(host_provider),
        config_from_host,
    );
}

pub fn install_with_host_service<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
    service: Arc<SkillsService>,
    providers: SkillProviders,
    config_from_host: impl Fn(&C) -> SkillsExtensionConfig + Send + Sync + 'static,
) where
    C: Send + Sync + 'static,
{
    let host_provider = Arc::new(HostSkillProvider::with_service(service));
    install_inner(
        registry,
        providers.with_host_provider(host_provider.clone()),
        Some(host_provider),
        config_from_host,
    );
}

pub fn install_with_providers<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
    providers: SkillProviders,
    config_from_host: impl Fn(&C) -> SkillsExtensionConfig + Send + Sync + 'static,
) where
    C: Send + Sync + 'static,
{
    install_inner(
        registry,
        providers,
        /*host_provider*/ None,
        config_from_host,
    );
}

fn install_inner<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
    providers: SkillProviders,
    host_provider: Option<Arc<HostSkillProvider>>,
    config_from_host: impl Fn(&C) -> SkillsExtensionConfig + Send + Sync + 'static,
) where
    C: Send + Sync + 'static,
{
    let extension = Arc::new(SkillsExtension {
        providers,
        host_provider,
        event_sink: registry.event_sink(),
        config_from_host: Arc::new(config_from_host),
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.turn_input_contributor(extension.clone());
    registry.tool_contributor(extension.clone());
    registry.tool_lifecycle_contributor(extension);
}
