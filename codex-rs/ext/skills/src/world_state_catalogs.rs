use std::sync::Arc;

use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::SelectedPluginSnapshot;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::openai_models::ModelInfo;

use crate::HostSkillsSnapshot;
use crate::SkillsExtensionConfig;
use crate::catalog::SkillCatalog;
use crate::provider::SkillListQuery;
use crate::provider::attribute_executor_plugins;
use crate::render::AvailableSkillsRender;
use crate::render::RenderedSkillCatalogs;
use crate::render::SkillMetadataBudget;
use crate::render::render_combined_available_skills;
use crate::render::skill_metadata_budget;
use crate::render_observability::CatalogSurface;
use crate::render_observability::record_catalog_render;
use crate::sources::SkillProviders;
use crate::state::EmittedCatalogBudgetWarnings;
use crate::state::ExecutorSkillsStepState;
use crate::state::HostSkillsCatalogInWorldState;
use crate::state::HostSkillsStepState;
use crate::state::SkillsSessionState;
use crate::state::SkillsThreadState;
use crate::world_state::CatalogRenderCallback;
use crate::world_state::executor_skills_world_state_section;
use crate::world_state::host_skills_world_state_section;
use crate::world_state::orchestrator_skills_world_state_section;

type CatalogWarningEmitter = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Clone, Copy, Eq, PartialEq)]
enum CatalogKind {
    Executor,
    Orchestrator,
    Host,
}

impl CatalogKind {
    fn metrics_surface(self) -> CatalogSurface {
        match self {
            Self::Executor => CatalogSurface::ExecutorWorldState,
            Self::Orchestrator => CatalogSurface::OrchestratorWorldState,
            Self::Host => CatalogSurface::HostWorldState,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum CatalogStatus {
    Unavailable,
    Disabled,
    Enabled,
}

struct CatalogContribution {
    catalog: SkillCatalog,
    status: CatalogStatus,
}

impl CatalogContribution {
    fn unavailable() -> Self {
        Self {
            catalog: SkillCatalog::default(),
            status: CatalogStatus::Unavailable,
        }
    }
}

pub(crate) struct CatalogContributions {
    executor: CatalogContribution,
    orchestrator: CatalogContribution,
    host: CatalogContribution,
}

pub(crate) struct RenderedCatalogContribution {
    kind: CatalogKind,
    pub(crate) status: CatalogStatus,
    rendered: Option<AvailableSkillsRender>,
}

pub(crate) struct CatalogContext<'a> {
    providers: &'a SkillProviders,
    input: WorldStateContributionInput<'a>,
    thread_state: Arc<SkillsThreadState>,
    config: SkillsExtensionConfig,
    metadata_budget: SkillMetadataBudget,
    include_usage: bool,
    warning_emitter: CatalogWarningEmitter,
}

impl<'a> CatalogContext<'a> {
    pub(crate) fn new(
        providers: &'a SkillProviders,
        event_sink: Arc<dyn ExtensionEventSink>,
        input: WorldStateContributionInput<'a>,
    ) -> Option<Self> {
        let thread_state = input.thread_store.get::<SkillsThreadState>()?;
        let config = thread_state.config();
        let model_info = input.thread_store.get::<ModelInfo>();
        let include_usage = model_info
            .as_deref()
            .is_some_and(|model_info| model_info.include_skills_usage_instructions);
        let context_window = model_info
            .as_deref()
            .and_then(ModelInfo::resolved_context_window);
        let metadata_budget = skill_metadata_budget(context_window, config.max_context_tokens);
        let emitted_warnings = input
            .turn_store
            .get_or_init(EmittedCatalogBudgetWarnings::default);
        let thread_id = input.thread_store.level_id().to_string();
        let turn_id = input.turn_id.to_string();
        let warning_emitter: CatalogWarningEmitter = Arc::new(move |message| {
            if emitted_warnings.insert(&message) {
                event_sink.emit_warning(ExtensionWarning {
                    thread_id: thread_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    message,
                });
            }
        });

        Some(Self {
            providers,
            input,
            thread_state,
            config,
            metadata_budget,
            include_usage,
            warning_emitter,
        })
    }

    pub(crate) async fn discover_catalogs(&self) -> CatalogContributions {
        let orchestrator_enabled = self.thread_state.orchestrator_skills_enabled()
            && self.providers.has_orchestrator_provider();
        let query = SkillListQuery {
            turn_id: self.input.turn_id.to_string(),
            executor_roots: self.input.ready_selected_capability_roots.to_vec(),
            resolved_executor_roots: Vec::new(),
            host_snapshot: None,
            include_host_skills: false,
            include_bundled_skills: self.config.bundled_skills_enabled,
            include_orchestrator_skills: orchestrator_enabled,
            mcp_resources: self
                .input
                .session_store
                .get::<SkillsSessionState>()
                .and_then(|state| state.mcp_resources.clone()),
            executor_capability_discovery: self.input.executor_capability_discovery.cloned(),
        };

        let (executor, orchestrator, host) = futures::join!(
            self.discover_executor_catalog(query.clone()),
            self.discover_orchestrator_catalog(query),
            self.discover_host_catalog(),
        );

        CatalogContributions {
            executor,
            orchestrator,
            host,
        }
    }

    async fn discover_executor_catalog(&self, query: SkillListQuery) -> CatalogContribution {
        let mut catalog = self
            .thread_state
            .executor_catalog_snapshot(self.providers, query)
            .await;
        if let Some(selected_plugins) = self.input.turn_store.get::<SelectedPluginSnapshot>() {
            attribute_executor_plugins(&mut catalog, &selected_plugins);
        }
        self.input
            .turn_store
            .insert(ExecutorSkillsStepState(catalog.clone()));

        CatalogContribution {
            catalog,
            status: CatalogStatus::Enabled,
        }
    }

    async fn discover_orchestrator_catalog(&self, query: SkillListQuery) -> CatalogContribution {
        if !self.providers.has_orchestrator_provider() {
            return CatalogContribution::unavailable();
        }

        if !query.include_orchestrator_skills {
            return CatalogContribution {
                catalog: SkillCatalog::default(),
                status: CatalogStatus::Disabled,
            };
        }

        CatalogContribution {
            catalog: self
                .thread_state
                .orchestrator_catalog_snapshot(self.providers, query)
                .await,
            status: CatalogStatus::Enabled,
        }
    }

    async fn discover_host_catalog(&self) -> CatalogContribution {
        let Some(host_snapshot) = self
            .input
            .turn_store
            .get::<HostSkillsSnapshot>()
            .filter(|_| self.providers.has_host_provider())
        else {
            return CatalogContribution::unavailable();
        };

        let needs_catalog =
            self.config.include_instructions || self.config.shadow_selection_enabled;
        let catalog = if needs_catalog {
            let catalog = self
                .providers
                .list_host_for_turn(SkillListQuery {
                    turn_id: self.input.turn_id.to_string(),
                    executor_roots: Vec::new(),
                    resolved_executor_roots: Vec::new(),
                    host_snapshot: Some(host_snapshot),
                    include_host_skills: true,
                    include_bundled_skills: false,
                    include_orchestrator_skills: false,
                    mcp_resources: None,
                    executor_capability_discovery: None,
                })
                .await;
            self.input
                .turn_store
                .insert(HostSkillsStepState(catalog.clone()));
            catalog
        } else {
            SkillCatalog::default()
        };

        CatalogContribution {
            catalog,
            status: CatalogStatus::Enabled,
        }
    }

    pub(crate) fn render_catalogs(
        &self,
        catalogs: CatalogContributions,
    ) -> [RenderedCatalogContribution; 3] {
        let rendered = if self.config.include_instructions {
            render_combined_available_skills(
                &catalogs.executor.catalog,
                &catalogs.orchestrator.catalog,
                &catalogs.host.catalog,
                self.metadata_budget,
                self.include_usage,
            )
        } else {
            RenderedSkillCatalogs::default()
        };

        [
            (CatalogKind::Executor, catalogs.executor, rendered.executor),
            (
                CatalogKind::Orchestrator,
                catalogs.orchestrator,
                rendered.orchestrator,
            ),
            (CatalogKind::Host, catalogs.host, rendered.host),
        ]
        .map(|(kind, catalog, rendered)| RenderedCatalogContribution {
            kind,
            status: catalog.status,
            rendered,
        })
    }

    pub(crate) fn build_world_state_section(
        &self,
        catalog: RenderedCatalogContribution,
    ) -> WorldStateSectionContribution {
        let RenderedCatalogContribution {
            kind,
            status,
            rendered,
        } = catalog;
        let report = rendered
            .as_ref()
            .map(|rendered| rendered.report.clone())
            .unwrap_or_default();
        let body = rendered
            .and_then(|rendered| rendered.into_fragment(self.include_usage))
            .map(|fragment| fragment.body());
        let include_instructions = self.config.include_instructions;
        let metrics = self.input.extension_metrics.clone();
        let warning_emitter = Arc::clone(&self.warning_emitter);
        let metadata_budget = self.metadata_budget;
        let render_report = report.clone();
        let on_render: CatalogRenderCallback = Box::new(move || {
            if !include_instructions || status != CatalogStatus::Enabled {
                return;
            }

            record_catalog_render(
                metrics.as_deref(),
                kind.metrics_surface(),
                metadata_budget,
                &render_report,
            );
            if let Some(message) = render_report.warning_message() {
                warning_emitter(message);
            }
        });

        match kind {
            CatalogKind::Executor => {
                executor_skills_world_state_section(body, include_instructions, on_render)
            }
            CatalogKind::Orchestrator => orchestrator_skills_world_state_section(
                body,
                include_instructions,
                status == CatalogStatus::Enabled,
                on_render,
            ),
            CatalogKind::Host => {
                self.input.turn_store.insert(HostSkillsCatalogInWorldState);
                host_skills_world_state_section(body, include_instructions, &report, on_render)
            }
        }
    }
}
