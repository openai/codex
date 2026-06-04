use std::sync::Arc;

use codex_core::config::Config;
use codex_core_skills::HostLoadedSkills;
use codex_core_skills::SkillInstructions;
use codex_core_skills::injection::InjectedHostSkillPrompts;
use codex_core_skills::injection::SkillInjection;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContextContributionInput;
use codex_extension_api::ContextContributor;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputContributor;
use codex_extension_api::TurnInputEnvironment;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WarningEvent;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillSourceKind;
use crate::provider::HostSkillProvider;
use crate::provider::SkillListQuery;
use crate::provider::SkillReadRequest;
use crate::render::available_skills_fragment;
use crate::render::truncate_main_prompt_contents;
use crate::selection::collect_explicit_skill_mentions;
use crate::sources::SkillProviders;
use crate::state::SkillsExtensionConfig;
use crate::state::SkillsThreadState;
use crate::state::SkillsTurnState;

#[derive(Clone)]
struct SkillsExtension {
    providers: SkillProviders,
    event_sink: Arc<dyn ExtensionEventSink>,
}

#[async_trait::async_trait]
impl ThreadLifecycleContributor<Config> for SkillsExtension {
    async fn on_thread_start(&self, input: ThreadStartInput<'_, Config>) {
        input
            .thread_store
            .insert(SkillsThreadState::new(SkillsExtensionConfig::from_config(
                input.config,
            )));
    }
}

impl ConfigContributor<Config> for SkillsExtension {
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &Config,
        new_config: &Config,
    ) {
        let next_config = SkillsExtensionConfig::from_config(new_config);
        if let Some(state) = thread_store.get::<SkillsThreadState>() {
            state.set_config(next_config);
        } else {
            thread_store.insert(SkillsThreadState::new(next_config));
        }
    }
}

impl ContextContributor for SkillsExtension {
    fn contribute<'a>(
        &'a self,
        input: ContextContributionInput,
        _session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
        turn_store: &'a ExtensionData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<PromptFragment>> + Send + 'a>> {
        Box::pin(async move {
            let Some(thread_state) = thread_store.get::<SkillsThreadState>() else {
                return Vec::new();
            };

            let config = thread_state.config();
            if !config.include_instructions {
                return Vec::new();
            }

            let host_loaded_skills = turn_store.get::<HostLoadedSkills>();
            let catalog = self
                .list_catalog(
                    input.turn_id.clone(),
                    &input.environments,
                    config.bundled_skills_enabled,
                    host_loaded_skills,
                )
                .await;

            let fragment = available_skills_fragment(&catalog);
            let catalog_body = fragment.body();
            if !thread_state.mark_catalog_emitted_if_changed(&catalog_body) {
                return Vec::new();
            }

            vec![PromptFragment::developer_capability(fragment.render())]
        })
    }
}

#[async_trait::async_trait]
impl TurnInputContributor for SkillsExtension {
    async fn contribute(
        &self,
        input: TurnInputContext,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        turn_store: &ExtensionData,
    ) -> Vec<Box<dyn ContextualUserFragment + Send>> {
        let Some(thread_state) = thread_store.get::<SkillsThreadState>() else {
            return Vec::new();
        };

        let config = thread_state.config();
        let host_loaded_skills = turn_store.get::<HostLoadedSkills>();
        let catalog = self
            .list_catalog(
                input.turn_id.clone(),
                &input.environments,
                config.bundled_skills_enabled,
                host_loaded_skills.clone(),
            )
            .await;
        for warning in &catalog.warnings {
            self.emit_warning(&input.turn_id, warning.clone());
        }

        let selected_entries = collect_explicit_skill_mentions(&input.user_input, &catalog);
        let mut fragments: Vec<Box<dyn ContextualUserFragment + Send>> = Vec::new();
        if config.include_instructions {
            let fragment = available_skills_fragment(&catalog);
            let catalog_body = fragment.body();
            if thread_state.mark_catalog_emitted_if_changed(&catalog_body) {
                fragments.push(Box::new(fragment));
            }
        }

        let mut warnings = catalog.warnings.clone();
        let mut main_prompts_injected = false;
        let mut injected_host_skill_prompts = InjectedHostSkillPrompts::default();
        for entry in &selected_entries {
            match self
                .read_main_prompt(entry, host_loaded_skills.clone())
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
                    let injection = SkillInjection {
                        name: entry.name.clone(),
                        path: entry.rendered_path().to_string(),
                        contents,
                    };
                    fragments.push(Box::new(SkillInstructions::from(&injection)));
                    main_prompts_injected = true;
                    if entry.authority.kind == SkillSourceKind::Host {
                        injected_host_skill_prompts.insert_path(entry.main_prompt.0.clone());
                    }
                }
                Err(message) => {
                    let warning = format!("Failed to load skill `{}`: {message}", entry.name);
                    self.emit_warning(&input.turn_id, warning.clone());
                    warnings.push(warning);
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
    }
}

impl SkillsExtension {
    async fn list_catalog(
        &self,
        turn_id: String,
        environments: &[TurnInputEnvironment],
        bundled_skills_enabled: bool,
        host_loaded_skills: Option<Arc<HostLoadedSkills>>,
    ) -> crate::catalog::SkillCatalog {
        let query = SkillListQuery {
            turn_id,
            executor_authorities: environments
                .iter()
                .map(|environment| {
                    SkillAuthority::new(
                        SkillSourceKind::Executor,
                        environment.environment_id.clone(),
                    )
                })
                .collect(),
            include_host_skills: true,
            include_bundled_skills: bundled_skills_enabled,
            include_remote_skills: true,
        };
        self.providers
            .list_for_turn(query, host_loaded_skills)
            .await
    }

    async fn read_main_prompt(
        &self,
        entry: &SkillCatalogEntry,
        host_loaded_skills: Option<Arc<HostLoadedSkills>>,
    ) -> Result<SkillReadResult, String> {
        let request = SkillReadRequest {
            authority: entry.authority.clone(),
            package: entry.id.clone(),
            resource: entry.main_prompt.clone(),
        };
        let result = if entry.authority.kind == SkillSourceKind::Host {
            self.providers.read_host(request, host_loaded_skills).await
        } else {
            self.providers.read(request).await
        };

        result.map_err(|err| err.message)
    }

    fn emit_warning(&self, turn_id: &str, message: String) {
        self.event_sink.emit(Event {
            id: turn_id.to_string(),
            msg: EventMsg::Warning(WarningEvent { message }),
        });
    }
}

pub fn install(registry: &mut ExtensionRegistryBuilder<Config>) {
    install_with_providers(
        registry,
        SkillProviders::new().with_host_provider(Arc::new(HostSkillProvider::new())),
    );
}

pub fn install_with_providers(
    registry: &mut ExtensionRegistryBuilder<Config>,
    providers: SkillProviders,
) {
    let extension = Arc::new(SkillsExtension {
        providers,
        event_sink: registry.event_sink(),
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.turn_input_contributor(extension);
}
