use std::sync::Arc;

use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::injection::InjectedHostSkillPrompts;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContextContributor;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::SkillInvocationContributor;
use codex_extension_api::SkillInvocationInput;
use codex_extension_api::SkillInvocationKind;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputContributor;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_mcp::McpResourceClient;
use codex_otel::MetricsClient;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WarningEvent;

use crate::SkillsExtensionConfig;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillSourceKind;
use crate::fragments::SkillInstructions;
use crate::provider::HostSkillProvider;
use crate::provider::SkillListQuery;
use crate::provider::SkillProvider;
use crate::provider::SkillReadRequest;
use crate::render::MAX_SKILL_NAME_BYTES;
use crate::render::MAX_SKILL_PATH_BYTES;
use crate::render::available_skills_fragment;
use crate::render::truncate_main_prompt_contents;
use crate::render::truncate_utf8_to_bytes;
use crate::selection::collect_explicit_skill_mentions;
use crate::shadow_selection_experiment::ShadowSelectionExperiment;
use crate::sources::SkillProviders;
use crate::state::ExecutorSkillsStepState;
use crate::state::SkillsThreadState;
use crate::state::SkillsTurnState;
use crate::tools::skill_tools;
use crate::world_state::executor_skills_world_state_section;

struct SkillsExtension<C> {
    providers: SkillProviders,
    event_sink: Arc<dyn ExtensionEventSink>,
    config_from_host: Arc<dyn Fn(&C) -> SkillsExtensionConfig + Send + Sync>,
    shadow_selection: Arc<ShadowSelectionExperiment>,
}

impl<C> ThreadLifecycleContributor<C> for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_thread_start<'a>(&'a self, input: ThreadStartInput<'a, C>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let orchestrator_skills_available = !input
                .environments
                .iter()
                .any(|environment| environment.environment_id == LOCAL_ENVIRONMENT_ID);
            input.thread_store.insert(SkillsThreadState::new(
                (self.config_from_host)(input.config),
                orchestrator_skills_available,
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
            let orchestrator_skills_available = true;
            thread_store.insert(SkillsThreadState::new(
                next_config,
                orchestrator_skills_available,
            ));
        }
    }
}

impl<C> ContextContributor for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn contribute_thread_context<'a>(
        &'a self,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<PromptFragment>> + Send + 'a>> {
        Box::pin(async move {
            let Some(thread_state) = thread_store.get::<SkillsThreadState>() else {
                return Vec::new();
            };
            let config = thread_state.config();
            if !config.include_instructions {
                return Vec::new();
            }
            let catalog = self
                .list_skills(
                    SkillListQuery {
                        turn_id: thread_store.level_id().to_string(),
                        executor_roots: Vec::new(),
                        host_snapshot: None,
                        include_host_skills: false,
                        include_bundled_skills: config.bundled_skills_enabled,
                        include_orchestrator_skills: thread_state.orchestrator_skills_enabled(),
                        mcp_resources: session_store.get::<McpResourceClient>(),
                    },
                    &thread_state,
                )
                .await;
            for warning in &catalog.warnings {
                self.emit_warning(thread_store.level_id(), warning.clone());
            }
            let include_usage = thread_store
                .get::<ModelInfo>()
                .is_some_and(|model_info| model_info.include_skills_usage_instructions);
            available_skills_fragment(&catalog, include_usage)
                .map(|fragment| PromptFragment::developer_capability(fragment.render()))
                .into_iter()
                .collect()
        })
    }

    fn contribute_world_state<'a>(
        &'a self,
        input: WorldStateContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
        Box::pin(async move {
            let Some(thread_state) = input.thread_store.get::<SkillsThreadState>() else {
                return Vec::new();
            };
            let config = thread_state.config();
            let catalog = thread_state
                .executor_catalog_snapshot(
                    &self.providers,
                    SkillListQuery {
                        turn_id: input.turn_id.to_string(),
                        executor_roots: input.ready_selected_capability_roots.to_vec(),
                        host_snapshot: None,
                        include_host_skills: false,
                        include_bundled_skills: config.bundled_skills_enabled,
                        include_orchestrator_skills: false,
                        mcp_resources: input.session_store.get::<McpResourceClient>(),
                    },
                )
                .await;
            input
                .turn_store
                .insert(ExecutorSkillsStepState(catalog.clone()));
            let include_usage = input
                .thread_store
                .get::<ModelInfo>()
                .is_some_and(|model_info| model_info.include_skills_usage_instructions);
            vec![executor_skills_world_state_section(
                &catalog,
                config.include_instructions,
                include_usage,
            )]
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
            Arc::clone(&self.shadow_selection),
        )
    }
}

impl<C> SkillInvocationContributor for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_skill_invocation<'a>(
        &'a self,
        input: SkillInvocationInput<'a>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            match input.kind {
                SkillInvocationKind::Implicit => {
                    if let Some(state) = input
                        .thread_store
                        .get::<SkillsThreadState>()
                        .and_then(|state| state.shadow_selection_turn(input.turn_id))
                    {
                        self.shadow_selection
                            .record_invocation(&state, input.skill_resource);
                    }
                }
                SkillInvocationKind::Explicit => {}
            }
        })
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
            let host_snapshot = turn_store.get::<HostSkillsSnapshot>();
            let query = SkillListQuery {
                turn_id: input.turn_id.clone(),
                executor_roots: Vec::new(),
                host_snapshot: host_snapshot.clone(),
                include_host_skills: true,
                include_bundled_skills: config.bundled_skills_enabled,
                include_orchestrator_skills: thread_state.orchestrator_skills_enabled(),
                mcp_resources: session_store.get::<McpResourceClient>(),
            };
            let host_query = query.clone();
            let mut catalog = self.list_skills(query, &thread_state).await;
            if let Some(executor_skills) = turn_store.get::<ExecutorSkillsStepState>() {
                catalog.extend(executor_skills.0.clone());
            }
            for warning in &catalog.warnings {
                self.emit_warning(&input.turn_id, warning.clone());
            }

            let selected_entries = collect_explicit_skill_mentions(&input.user_input, &catalog);
            let shadow_selection_turn = if config.shadow_selection_enabled {
                // App-server leaves host discovery in core, so its configured providers omit the
                // host snapshot. Extend only the experiment catalog to keep rendered context intact.
                let mut shadow_catalog = catalog.clone();
                if host_snapshot.is_some()
                    && let Ok(host_catalog) = HostSkillProvider::new().list(host_query).await
                {
                    shadow_catalog.extend(host_catalog);
                }
                Some(
                    self.shadow_selection
                        .run(&input.user_input, &shadow_catalog),
                )
            } else {
                None
            };
            thread_state
                .replace_shadow_selection_turn(input.turn_id.clone(), shadow_selection_turn);
            let mut fragments: Vec<Box<dyn ContextualUserFragment + Send>> = Vec::new();
            if config.include_instructions {
                let mut turn_catalog = catalog.clone();
                turn_catalog.entries.retain(|entry| {
                    entry.authority.kind != SkillSourceKind::Executor
                        && entry.authority.kind != SkillSourceKind::Orchestrator
                });
                let include_usage = thread_store
                    .get::<ModelInfo>()
                    .is_some_and(|model_info| model_info.include_skills_usage_instructions);
                if let Some(fragment) = available_skills_fragment(&turn_catalog, include_usage) {
                    fragments.push(Box::new(fragment));
                }
            }

            let mut warnings = catalog.warnings.clone();
            let mut main_prompts_injected = false;
            let mut injected_host_skill_prompts = InjectedHostSkillPrompts::default();
            for entry in &selected_entries {
                match self
                    .read_main_prompt(entry, host_snapshot.clone(), session_store, &thread_state)
                    .await
                {
                    Ok(read_result) => {
                        let (contents, truncated) =
                            truncate_main_prompt_contents(read_result.contents.as_str());
                        if truncated {
                            let warning = format!(
                                "Skill `{}` exceeded the main prompt context limit and was truncated.",
                                entry.name
                            );
                            self.emit_warning(&input.turn_id, warning.clone());
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
                        if entry.authority.kind == SkillSourceKind::Host {
                            injected_host_skill_prompts.insert_path(entry.main_prompt.as_str());
                        }
                    }
                    Err(message) => {
                        let warning = format!("Failed to load skill `{}`: {message}", entry.name);
                        self.emit_warning(&input.turn_id, warning.clone());
                        warnings.push(warning);
                    }
                }
            }

            if let Some(host_snapshot) = &host_snapshot {
                for entry in selected_entries
                    .iter()
                    .filter(|entry| entry.authority.kind != SkillSourceKind::Host)
                {
                    for host_skill in host_snapshot
                        .outcome()
                        .skills
                        .iter()
                        .filter(|host_skill| host_skill.name == entry.name)
                    {
                        injected_host_skill_prompts
                            .insert_path(host_skill.path_to_skills_md.to_string_lossy());
                    }
                }
            }

            turn_store.insert(SkillsTurnState {
                catalog,
                selected_entries,
                warnings,
                main_prompts_injected,
            });
            if !injected_host_skill_prompts.is_empty() {
                turn_store.insert(injected_host_skill_prompts);
            }

            fragments
        })
    }
}

impl<C> SkillsExtension<C> {
    #[tracing::instrument(level = "trace", skip_all)]
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

    #[tracing::instrument(level = "trace", skip_all, fields(skill = %entry.name))]
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

    fn emit_warning(&self, turn_id: &str, message: String) {
        self.event_sink.emit(Event {
            id: turn_id.to_string(),
            msg: EventMsg::Warning(WarningEvent { message }),
        });
    }
}

pub fn install<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
    config_from_host: impl Fn(&C) -> SkillsExtensionConfig + Send + Sync + 'static,
) where
    C: Send + Sync + 'static,
{
    install_with_providers(
        registry,
        SkillProviders::new().with_host_provider(Arc::new(HostSkillProvider::new())),
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
    install_with_providers_and_metrics(
        registry,
        providers,
        /*metrics_client*/ None,
        config_from_host,
    );
}

pub fn install_with_providers_and_metrics<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
    providers: SkillProviders,
    metrics_client: Option<MetricsClient>,
    config_from_host: impl Fn(&C) -> SkillsExtensionConfig + Send + Sync + 'static,
) where
    C: Send + Sync + 'static,
{
    let extension = Arc::new(SkillsExtension {
        providers,
        event_sink: registry.event_sink(),
        config_from_host: Arc::new(config_from_host),
        shadow_selection: Arc::new(ShadowSelectionExperiment::new(metrics_client)),
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.turn_input_contributor(extension.clone());
    registry.skill_invocation_contributor(extension.clone());
    registry.tool_contributor(extension);
}
