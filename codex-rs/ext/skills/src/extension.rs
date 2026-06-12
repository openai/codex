use std::sync::Arc;

use codex_core_skills::HostLoadedSkills;
use codex_core_skills::injection::InjectedHostSkillPrompts;
use codex_extension_api::ConfigContributor;
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
use codex_extension_api::ToolExecutor;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputContributor;
use codex_mcp::McpResourceClient;
use codex_protocol::capabilities::SelectedCapabilityRoot;
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
use crate::provider::SkillReadRequest;
use crate::render::MAX_SKILL_NAME_BYTES;
use crate::render::MAX_SKILL_PATH_BYTES;
use crate::render::available_skills_fragment;
use crate::render::truncate_main_prompt_contents;
use crate::render::truncate_utf8_to_bytes;
use crate::selection::collect_explicit_skill_mentions;
use crate::sources::SkillProviders;
use crate::state::SkillsThreadState;
use crate::state::SkillsTurnState;
use crate::tools::skill_tools;

struct SkillsExtension<C> {
    providers: SkillProviders,
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
            input.thread_store.insert(SkillsThreadState::new(
                (self.config_from_host)(input.config),
                selected_roots,
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
            thread_store.insert(SkillsThreadState::new(next_config, Vec::new()));
        }
    }
}

impl<C> ContextContributor for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn contribute<'a>(
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
                        executor_roots: thread_state.selected_roots().to_vec(),
                        host: None,
                        include_host_skills: false,
                        include_bundled_skills: config.bundled_skills_enabled,
                        include_orchestrator_skills: true,
                        mcp_resources: session_store.get::<McpResourceClient>(),
                    },
                    &thread_state,
                )
                .await;
            for warning in &catalog.warnings {
                self.emit_warning(thread_store.level_id(), warning.clone());
            }
            available_skills_fragment(&catalog)
                .map(|fragment| PromptFragment::developer_capability(fragment.render()))
                .into_iter()
                .collect()
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
        _thread_store: &ExtensionData,
        _turn_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        if !self.providers.has_orchestrator_provider() {
            return Vec::new();
        }

        skill_tools(
            self.providers.clone(),
            session_store.get::<McpResourceClient>(),
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
            let host_loaded_skills = turn_store.get::<HostLoadedSkills>();
            let query = SkillListQuery {
                turn_id: input.turn_id.clone(),
                executor_roots: thread_state.selected_roots().to_vec(),
                host: host_loaded_skills.clone(),
                include_host_skills: true,
                include_bundled_skills: config.bundled_skills_enabled,
                include_orchestrator_skills: true,
                mcp_resources: session_store.get::<McpResourceClient>(),
            };
            let catalog = self.list_skills(query, &thread_state).await;
            for warning in &catalog.warnings {
                self.emit_warning(&input.turn_id, warning.clone());
            }

            let selected_entries = collect_explicit_skill_mentions(&input.user_input, &catalog);
            let mut fragments: Vec<Box<dyn ContextualUserFragment + Send>> = Vec::new();
            if config.include_instructions {
                let mut turn_catalog = catalog.clone();
                turn_catalog.entries.retain(|entry| {
                    entry.authority.kind != SkillSourceKind::Executor
                        && entry.authority.kind != SkillSourceKind::Orchestrator
                });
                if let Some(fragment) = available_skills_fragment(&turn_catalog) {
                    fragments.push(Box::new(fragment));
                }
            }

            let mut warnings = catalog.warnings.clone();
            let mut main_prompts_injected = false;
            let mut injected_host_skill_prompts = InjectedHostSkillPrompts::default();
            for entry in &selected_entries {
                match self
                    .read_main_prompt(entry, host_loaded_skills.clone(), session_store)
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

            if let Some(host_loaded_skills) = &host_loaded_skills {
                for entry in selected_entries
                    .iter()
                    .filter(|entry| entry.authority.kind != SkillSourceKind::Host)
                {
                    for host_skill in host_loaded_skills
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
    async fn list_skills(
        &self,
        mut query: SkillListQuery,
        thread_state: &SkillsThreadState,
    ) -> SkillCatalog {
        let include_orchestrator_skills = query.include_orchestrator_skills;
        let orchestrator_query = query.clone();
        query.include_orchestrator_skills = false;

        let mut catalog = self.providers.list_for_turn(query).await;
        if include_orchestrator_skills {
            let orchestrator_catalog = thread_state
                .orchestrator_catalog_snapshot(
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
        host_loaded_skills: Option<Arc<HostLoadedSkills>>,
        session_store: &ExtensionData,
    ) -> Result<SkillReadResult, String> {
        self.providers
            .read(SkillReadRequest {
                authority: entry.authority.clone(),
                package: entry.id.clone(),
                resource: entry.main_prompt.clone(),
                host: host_loaded_skills,
                mcp_resources: session_store.get::<McpResourceClient>(),
            })
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
    let extension = Arc::new(SkillsExtension {
        providers,
        event_sink: registry.event_sink(),
        config_from_host: Arc::new(config_from_host),
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.turn_input_contributor(extension.clone());
    registry.tool_contributor(extension);
}
