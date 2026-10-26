use std::sync::Arc;

use crate::HostSkillsSnapshot;
use crate::InjectedHostSkillPrompts;
use codex_analytics::InvocationType;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContextContributor;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::PromptFragment;
use codex_extension_api::PromptSlot;
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

use crate::SkillsExtensionConfig;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillSourceKind;
use crate::fragments::AvailableSkillsInstructions;
use crate::fragments::SkillInstructions;
use crate::fragments::SkillResourceAccess;
use crate::preparation::prepare_catalogs;
use crate::provider::HostSkillProvider;
use crate::provider::SkillListQuery;
use crate::provider::SkillReadContext;
use crate::provider::SkillReadRequest;
use crate::render::AvailableSkillsRender;
use crate::render::MAX_SKILL_NAME_BYTES;
use crate::render::MAX_SKILL_PATH_BYTES;
use crate::render::SkillCatalogRenderPolicy;
use crate::render::SkillMetadataBudget;
use crate::render::SkillRenderReport;
use crate::render::render_available_skills;
use crate::render::skill_metadata_budget;
use crate::render::truncate_main_prompt_contents;
use crate::render::truncate_utf8_to_bytes;
use crate::render_observability::CatalogSurface;
use crate::render_observability::record_catalog_render;
use crate::selection::collect_explicit_skill_mentions;
use crate::shadow_selection_experiment::ShadowSelectionExperiment;
use crate::sources::SkillProviders;
use crate::state::HostSkillsCatalogInWorldState;
use crate::state::SkillsSessionState;
use crate::state::SkillsThreadState;
use crate::step_state::SkillsStepState;
use crate::telemetry::SkillTelemetry;
use crate::tools::SkillAnalytics;
use crate::tools::SkillToolAuthority;
use crate::tools::skill_tools;
use crate::warnings::bounded_warnings;
use crate::world_state_catalogs::render_world_state_sections;

#[path = "cloud_skill.rs"]
mod cloud_skill;

struct SkillsExtension<C> {
    providers: SkillProviders,
    event_sink: Arc<dyn ExtensionEventSink>,
    config_from_host: Arc<dyn Fn(&C) -> SkillsExtensionConfig + Send + Sync>,
    shadow_selection: Arc<ShadowSelectionExperiment>,
}

#[derive(Default)]
struct RenderedCatalog {
    fragment: Option<AvailableSkillsInstructions>,
    warning_message: Option<String>,
}

fn render_catalog(
    extension_metrics: Option<&dyn ExtensionMetrics>,
    catalog_surface: CatalogSurface,
    catalog: &SkillCatalog,
    include_skills_usage_instructions: bool,
    policy: SkillCatalogRenderPolicy,
    budget: SkillMetadataBudget,
) -> RenderedCatalog {
    render_prepared_catalog(
        extension_metrics,
        catalog_surface,
        include_skills_usage_instructions,
        budget,
        render_available_skills(
            catalog,
            policy,
            budget,
            include_skills_usage_instructions,
            /*alias_plan*/ None,
        ),
    )
}

fn render_prepared_catalog(
    extension_metrics: Option<&dyn ExtensionMetrics>,
    catalog_surface: CatalogSurface,
    include_skills_usage_instructions: bool,
    budget: SkillMetadataBudget,
    rendered: Option<AvailableSkillsRender>,
) -> RenderedCatalog {
    let Some(rendered) = rendered else {
        record_catalog_render(
            extension_metrics,
            catalog_surface,
            budget,
            &SkillRenderReport::default(),
        );
        return RenderedCatalog::default();
    };
    record_catalog_render(extension_metrics, catalog_surface, budget, &rendered.report);
    let warning_message = rendered
        .report
        .warning_message(/*catalog_deltas_enabled*/ false);
    let fragment = rendered.into_fragment(include_skills_usage_instructions);
    RenderedCatalog {
        fragment,
        warning_message,
    }
}

#[cfg(test)]
#[path = "extension_tests.rs"]
mod tests;

impl<C> ThreadLifecycleContributor<C> for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_thread_start<'a>(&'a self, input: ThreadStartInput<'a, C>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input.session_store.insert(SkillsSessionState {
                mcp_resources: input.mcp_resource_client.clone(),
                extension_metrics: input.extension_metrics.clone(),
            });
            let cloud_skills_available = self.providers.has_cloud_provider()
                && !input
                    .environments
                    .iter()
                    .any(|environment| environment.environment_id == LOCAL_ENVIRONMENT_ID);
            input.thread_store.insert(SkillsThreadState::new(
                (self.config_from_host)(input.config),
                cloud_skills_available,
                input.extension_metrics,
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
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &C,
        new_config: &C,
    ) {
        let next_config = (self.config_from_host)(new_config);
        if let Some(state) = thread_store.get::<SkillsThreadState>() {
            state.set_config(next_config);
        } else {
            let cloud_skills_available = self.providers.has_cloud_provider();
            thread_store.insert(SkillsThreadState::new(
                next_config,
                cloud_skills_available,
                session_store
                    .get::<SkillsSessionState>()
                    .and_then(|state| state.extension_metrics.clone()),
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
                .providers
                .list_for_turn(SkillListQuery {
                    turn_id: thread_store.level_id().to_string(),
                    executor_roots: Vec::new(),
                    resolved_executor_roots: Vec::new(),
                    host_snapshot: None,
                    include_host_skills: false,
                    include_bundled_skills: config.bundled_skills_enabled,
                    include_cloud_skills: false,
                    mcp_resources: session_store
                        .get::<SkillsSessionState>()
                        .and_then(|state| state.mcp_resources.clone()),
                    executor_capability_discovery: None,
                    executor_capability_discovery_v2: None,
                })
                .await;
            for warning in bounded_warnings(&catalog.warnings) {
                self.emit_warning(thread_store.level_id(), /*turn_id*/ None, warning);
            }
            let include_usage = thread_store
                .get::<ModelInfo>()
                .is_some_and(|model_info| model_info.include_skills_usage_instructions);
            let extension_metrics = session_store
                .get::<SkillsSessionState>()
                .and_then(|state| state.extension_metrics.clone());
            let rendered = render_catalog(
                extension_metrics.as_deref(),
                CatalogSurface::ThreadContext,
                &catalog,
                include_usage,
                SkillCatalogRenderPolicy::ExtensionCompatible,
                skill_metadata_budget(/*context_window*/ None, config.max_context_tokens),
            );
            if let Some(message) = rendered.warning_message {
                self.emit_warning(thread_store.level_id(), /*turn_id*/ None, message);
            }
            rendered
                .fragment
                .map(|fragment| {
                    PromptFragment::from_fragment(PromptSlot::DeveloperCapabilities, &fragment)
                })
                .into_iter()
                .collect()
        })
    }

    fn contribute_world_state<'a>(
        &'a self,
        input: WorldStateContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
        Box::pin(async move { render_world_state_sections(Arc::clone(&self.event_sink), input) })
    }
}

impl<C> ToolContributor for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn prepare_for_step<'a>(
        &'a self,
        input: codex_extension_api::StepPreparationInput<'a>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if input.step_store.get::<SkillsStepState>().is_some() {
                return;
            }
            if let Some(thread_state) = input.thread_store.get::<SkillsThreadState>() {
                let state = prepare_catalogs(&thread_state, &self.providers, &input).await;
                input.step_store.insert(state);
            }
        })
    }

    fn tools_for_step(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
        step_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        skill_tools(
            self.providers.clone(),
            session_store,
            thread_store,
            step_store,
        )
    }
}

impl<C> SkillInvocationContributor for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn requires_host_skill_discovery(&self) -> bool {
        self.providers.has_host_provider()
    }

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
                        state.record_invocation(input.skill_resource);
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
        input: TurnInputContext<'a>,
        extension_metrics: Option<Arc<dyn ExtensionMetrics>>,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
        turn_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<Box<dyn ContextualUserFragment + Send>>> {
        Box::pin(async move {
            let Some(thread_state) = thread_store.get::<SkillsThreadState>() else {
                return Vec::new();
            };

            let mcp_resources = session_store
                .get::<SkillsSessionState>()
                .and_then(|state| state.mcp_resources.clone());
            let host_snapshot = turn_store.get::<HostSkillsSnapshot>();
            let host_catalog_in_world_state =
                turn_store.get::<HostSkillsCatalogInWorldState>().is_some();
            let Some(step_state) = input.step_store.get::<SkillsStepState>() else {
                return Vec::new();
            };
            let config = &step_state.config;
            let host_skills = step_state.host_catalog.as_ref();
            let query = SkillListQuery {
                turn_id: input.turn_id.clone(),
                executor_roots: Vec::new(),
                resolved_executor_roots: Vec::new(),
                host_snapshot: host_snapshot.clone(),
                include_host_skills: host_skills.is_none() && !host_catalog_in_world_state,
                include_bundled_skills: config.bundled_skills_enabled,
                // Cloud and executor catalogs come from the prepared step snapshot.
                include_cloud_skills: false,
                mcp_resources: mcp_resources.clone(),
                executor_capability_discovery: None,
                executor_capability_discovery_v2: None,
            };
            let mut catalog = step_state.catalog.merged.clone();
            if !host_catalog_in_world_state && let Some(host_skills) = host_skills {
                catalog.extend(host_skills.clone());
            }
            catalog.extend(self.providers.list_for_turn(query).await);
            for warning in bounded_warnings(&catalog.warnings) {
                self.emit_warning(thread_store.level_id(), Some(&input.turn_id), warning);
            }

            let selected_entries = collect_explicit_skill_mentions(&input.user_input, &catalog);
            let shadow_selection_turn = if config.shadow_selection_enabled {
                let mut shadow_catalog = catalog.clone();
                if let Some(host_skills) = host_skills {
                    shadow_catalog.extend(host_skills.clone());
                }
                let shadow_selected_entries =
                    collect_explicit_skill_mentions(&input.user_input, &shadow_catalog);
                Some(self.shadow_selection.start(
                    &input,
                    shadow_catalog,
                    &shadow_selected_entries,
                    host_snapshot.clone(),
                    Arc::clone(&thread_state.recent_skill_invocations),
                    Arc::clone(&thread_state.shadow_task_context),
                ))
            } else {
                None
            };
            thread_state
                .replace_shadow_selection_turn(input.turn_id.clone(), shadow_selection_turn);
            let mut fragments: Vec<Box<dyn ContextualUserFragment + Send>> = Vec::new();
            if config.include_instructions && !host_catalog_in_world_state {
                let mut turn_catalog = catalog.clone();
                turn_catalog.entries.retain(|entry| {
                    entry.authority.kind != SkillSourceKind::Executor
                        && entry.authority.kind != SkillSourceKind::Cloud
                });
                let model_info = thread_store.get::<ModelInfo>();
                let include_usage = model_info
                    .as_deref()
                    .is_some_and(|model_info| model_info.include_skills_usage_instructions);
                let context_window = model_info
                    .as_deref()
                    .and_then(ModelInfo::resolved_context_window);
                let metadata_budget =
                    skill_metadata_budget(context_window, config.max_context_tokens);
                let rendered = render_catalog(
                    extension_metrics.as_deref(),
                    CatalogSurface::TurnInput,
                    &turn_catalog,
                    include_usage,
                    SkillCatalogRenderPolicy::ExtensionCompatible,
                    metadata_budget,
                );
                if let Some(message) = rendered.warning_message {
                    self.emit_warning(thread_store.level_id(), Some(&input.turn_id), message);
                }
                if let Some(fragment) = rendered.fragment {
                    fragments.push(Box::new(fragment));
                }
            }

            let mut injected_host_skill_prompts = InjectedHostSkillPrompts::default();
            let analytics = SkillAnalytics::from_stores(session_store, thread_store);
            for entry in &selected_entries {
                match self
                    .read_main_prompt(
                        entry,
                        host_snapshot.clone(),
                        mcp_resources.clone(),
                        &thread_state,
                        &input,
                    )
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
                            self.emit_warning(
                                thread_store.level_id(),
                                Some(&input.turn_id),
                                warning.clone(),
                            );
                        }
                        let fragment = SkillInstructions {
                            name: truncate_utf8_to_bytes(&entry.name, MAX_SKILL_NAME_BYTES).0,
                            path: truncate_utf8_to_bytes(
                                entry.rendered_path(),
                                MAX_SKILL_PATH_BYTES,
                            )
                            .0,
                            contents,
                            resource_access: (!entry.prompt_visible)
                                .then_some(&entry.authority)
                                .and_then(SkillToolAuthority::from_authority)
                                .map(|authority| SkillResourceAccess {
                                    authority,
                                    package: entry.id.0.clone(),
                                    main_resource: entry.main_prompt.as_str().to_string(),
                                }),
                        };
                        fragments.push(Box::new(fragment));
                        if entry.authority.kind == SkillSourceKind::Host {
                            injected_host_skill_prompts.insert_path(entry.main_prompt.as_str());
                        } else if let Some(analytics) = analytics.as_ref()
                            && let Some(model_info) = thread_store.get::<ModelInfo>()
                        {
                            analytics.track_skill_invocation(
                                entry,
                                &entry.primary_source(),
                                model_info.slug.clone(),
                                input.turn_id.clone(),
                                InvocationType::Explicit,
                            );
                        }
                    }
                    Err(message) => {
                        let warning = format!("Failed to load skill `{}`: {message}", entry.name);
                        self.emit_warning(
                            thread_store.level_id(),
                            Some(&input.turn_id),
                            warning.clone(),
                        );
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
                        injected_host_skill_prompts.insert_superseded_path(
                            host_skill.path_to_skills_md.inferred_native_path_string(),
                        );
                    }
                }
            }

            if !injected_host_skill_prompts.is_empty() {
                turn_store.insert(injected_host_skill_prompts);
            }

            fragments
        })
    }
}

impl<C> SkillsExtension<C> {
    #[tracing::instrument(level = "trace", skip_all, fields(skill = %entry.name))]
    async fn read_main_prompt(
        &self,
        entry: &SkillCatalogEntry,
        host_snapshot: Option<Arc<HostSkillsSnapshot>>,
        mcp_resources: Option<Arc<McpResourceClient>>,
        thread_state: &SkillsThreadState,
        input: &TurnInputContext<'_>,
    ) -> Result<SkillReadResult, String> {
        let context = match entry.authority.kind {
            SkillSourceKind::Cloud => SkillReadContext::Cloud { mcp_resources },
            SkillSourceKind::Custom(_) => SkillReadContext::Custom { mcp_resources },
            SkillSourceKind::Host => SkillReadContext::Host { host_snapshot },
            SkillSourceKind::Executor => {
                let fs = entry
                    .main_prompt
                    .environment_path()
                    .and_then(|(id, _)| {
                        input
                            .environments
                            .iter()
                            .find(|env| env.environment_id == id)
                            .map(|env| env.fs)
                    })
                    .ok_or_else(|| {
                        "skill environment is not available for this callback".to_string()
                    })?;
                SkillReadContext::Executor { fs }
            }
        };
        thread_state
            .read_skill(
                &self.providers,
                SkillReadRequest {
                    authority: entry.authority.clone(),
                    package: entry.id.clone(),
                    resource: entry.main_prompt.clone(),
                    context,
                },
            )
            .await
            .map_err(|err| err.message)
    }

    fn emit_warning(&self, thread_id: &str, turn_id: Option<&str>, message: String) {
        self.event_sink.emit_warning(ExtensionWarning {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.map(str::to_string),
            message,
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
    registry.turn_lifecycle_contributor(Arc::new(SkillTelemetry));
    registry.turn_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.turn_input_contributor(extension.clone());
    registry.skill_invocation_contributor(extension.clone());
    registry.tool_contributor(extension);
}
