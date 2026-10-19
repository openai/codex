use std::sync::Arc;

use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::SelectedPluginSnapshot;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use serde::Deserialize;
use serde::Serialize;

use crate::HostSkillsSnapshot;
use crate::SkillsExtensionConfig;
use crate::catalog::SkillCatalog;
use crate::catalog::apply_plugin_denials;
use crate::provider::SkillListQuery;
use crate::provider::attribute_executor_plugins;
use crate::render::AvailableSkillsRender;
use crate::render::PreparedSkillCatalog;
use crate::render::RenderedSkillCatalogs;
use crate::render::SkillCatalogRenderPolicy;
use crate::render::SkillMetadataBudget;
use crate::render::render_available_skills;
use crate::render::render_prepared_skill_catalogs;
use crate::render::skill_metadata_budget;
use crate::render_observability::CatalogSurface;
use crate::render_observability::record_catalog_render;
use crate::skills_extension_state::SkillToolAuthoritySelector;
use crate::skills_extension_state::SkillsStepState;
use crate::sources::SkillProviders;
use crate::state::EmittedCatalogBudgetWarnings;
use crate::state::HostSkillsCatalogInWorldState;
use crate::state::HostSkillsStepState;
use crate::state::SkillsSessionState;
use crate::state::SkillsThreadState;
use crate::world_state::CatalogRenderCallback;
use crate::world_state::cloud_skills_world_state_section;
use crate::world_state::executor_skills_world_state_section;
use crate::world_state::host_skills_world_state_section;

// Start with one quarter reserved for filesystem skills, before their inventory is known.
const FILESYSTEM_BUDGET_DIVISOR: usize = 4;

/// A display allocation retained across executor readiness changes and thread resume.
/// A changed cloud catalog or total budget starts a new allocation. Within that
/// allocation, filesystem pressure can only shrink the cloud cap.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogBudgetAllocation {
    total_budget: SkillMetadataBudget,
    cloud_limit: usize,
    cloud_catalog_fingerprint: [u8; 32],
}

type CatalogWarningEmitter = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Clone, Copy, Eq, PartialEq)]
enum CatalogKind {
    Executor,
    Cloud,
    Host,
}

impl CatalogKind {
    fn metrics_surface(self) -> CatalogSurface {
        match self {
            Self::Executor => CatalogSurface::ExecutorWorldState,
            Self::Cloud => CatalogSurface::CloudWorldState,
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
    cloud: CatalogContribution,
    host: CatalogContribution,
}

pub(crate) struct RenderedCatalogContribution {
    kind: CatalogKind,
    pub(crate) status: CatalogStatus,
    budget: SkillMetadataBudget,
    allocation: CatalogBudgetAllocation,
    rendered: Option<AvailableSkillsRender>,
}

pub(crate) struct CatalogContext<'a> {
    providers: &'a SkillProviders,
    input: WorldStateContributionInput<'a>,
    thread_state: Arc<SkillsThreadState>,
    config: SkillsExtensionConfig,
    metadata_budget: SkillMetadataBudget,
    render_policy: SkillCatalogRenderPolicy,
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
        let include_usage = input.model_info.include_skills_usage_instructions;
        let context_window = input.model_info.resolved_context_window();
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

        let render_policy = if input.executor_capability_discovery_v2.is_some() {
            SkillCatalogRenderPolicy::DiscoveryV2
        } else {
            SkillCatalogRenderPolicy::ExtensionCompatible
        };
        Some(Self {
            providers,
            input,
            thread_state,
            config,
            metadata_budget,
            render_policy,
            include_usage,
            warning_emitter,
        })
    }

    pub(crate) async fn discover_catalogs(&self) -> CatalogContributions {
        let cloud_enabled = self.thread_state.cloud_skill_enabled();
        // Core always supplies this vector, including when no captured roots are ready.
        // Descriptor-only callers retain their unbound discovery path.
        let captured_roots = self
            .input
            .step_store
            .get::<Vec<ResolvedSelectedCapabilityRoot>>();
        let executor_roots = captured_roots
            .as_ref()
            .map(|roots| {
                roots
                    .iter()
                    .map(|root| root.selected_root().clone())
                    .collect()
            })
            .unwrap_or_else(|| self.input.ready_selected_capability_roots.to_vec());
        let query = SkillListQuery {
            turn_id: self.input.turn_id.to_string(),
            executor_roots,
            resolved_executor_roots: captured_roots
                .map(|roots| roots.as_ref().clone())
                .unwrap_or_default(),
            host_snapshot: None,
            include_host_skills: false,
            include_bundled_skills: self.config.bundled_skills_enabled,
            include_cloud_skills: cloud_enabled,
            mcp_resources: self
                .input
                .session_store
                .get::<SkillsSessionState>()
                .and_then(|state| state.mcp_resources.clone()),
            executor_capability_discovery: self.input.executor_capability_discovery.cloned(),
            executor_capability_discovery_v2: self
                .input
                .executor_capability_discovery_v2
                .map(Vec::from),
        };

        let selected_plugins = self.input.step_store.get::<SelectedPluginSnapshot>();
        let step_state = self.input.step_store.get_or_init(SkillsStepState::new);
        let (step_catalog, host) = futures::join!(
            step_state.get_or_init(|| async {
                let discovery_v2_enabled = query.executor_capability_discovery_v2.is_some();
                let mut executor = self
                    .thread_state
                    .refresh_executor_catalog(self.providers, query)
                    .await;
                let mut cloud = self.thread_state.available_cloud_catalog();
                if let Some(selected_plugins) = selected_plugins.as_deref() {
                    if discovery_v2_enabled {
                        apply_plugin_denials(&mut executor, selected_plugins);
                        if let Some(cloud) = &mut cloud {
                            apply_plugin_denials(cloud, selected_plugins);
                        }
                    } else {
                        attribute_executor_plugins(&mut executor, selected_plugins);
                    }
                }
                Arc::new(self.thread_state.prepare_step_catalog(
                    cloud,
                    executor,
                    discovery_v2_enabled,
                ))
            }),
            self.discover_host_catalog(),
        );
        let cloud_status = if !self.providers.has_cloud_provider() {
            CatalogStatus::Unavailable
        } else if cloud_enabled || step_catalog.cloud_warnings.is_some() {
            CatalogStatus::Enabled
        } else {
            CatalogStatus::Disabled
        };
        CatalogContributions {
            executor: CatalogContribution {
                catalog: step_catalog.select_from_catalog(&[SkillToolAuthoritySelector::Executor]),
                status: CatalogStatus::Enabled,
            },
            cloud: CatalogContribution {
                catalog: step_catalog.select_from_catalog(&[SkillToolAuthoritySelector::Cloud]),
                status: cloud_status,
            },
            host,
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
                    include_cloud_skills: false,
                    mcp_resources: None,
                    executor_capability_discovery: None,
                    executor_capability_discovery_v2: None,
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
        let total_limit = self.metadata_budget.limit();
        let default_cloud_limit = total_limit - total_limit / FILESYSTEM_BUDGET_DIVISOR;
        let step_state = self.input.step_store.get::<SkillsStepState>();
        let executor_aliases = step_state
            .as_ref()
            .and_then(|state| state.get())
            .map(|catalog| {
                catalog
                    .aliases
                    .for_authority(SkillToolAuthoritySelector::Executor)
            });
        let mut executor = PreparedSkillCatalog::with_alias_plan(
            &catalogs.executor.catalog,
            self.render_policy,
            executor_aliases,
        );
        executor.prefer_cloud_skills(&catalogs.cloud.catalog);
        // Identify the full visible cloud inventory before budgeting, including entries
        // a previous cap omitted. Executor readiness and provider warnings are not inputs.
        let cloud_metadata = catalogs
            .cloud
            .catalog
            .entries
            .iter()
            .filter(|entry| entry.is_model_visible())
            .map(|entry| {
                (
                    &entry.id.0,
                    entry.authority.kind.to_string(),
                    &entry.name,
                    entry
                        .short_description
                        .as_ref()
                        .unwrap_or(&entry.description),
                    entry.rendered_path(),
                    entry.alias_root(),
                    entry.alias_root_order(),
                )
            })
            .collect::<Vec<_>>();
        let cloud_catalog_fingerprint =
            *blake3::hash(serde_json::json!(cloud_metadata).to_string().as_bytes()).as_bytes();
        let mut allocation = self
            .input
            .previous_world_state
            .and_then(|state| state.get(crate::world_state::CLOUD_SKILLS_WORLD_STATE_ID))
            .and_then(|snapshot| snapshot.get("allocation"))
            .and_then(|value| serde_json::from_value::<CatalogBudgetAllocation>(value.clone()).ok())
            .filter(|previous| {
                previous.total_budget == self.metadata_budget
                    && previous.cloud_catalog_fingerprint == cloud_catalog_fingerprint
            })
            .unwrap_or(CatalogBudgetAllocation {
                total_budget: self.metadata_budget,
                cloud_limit: default_cloud_limit,
                cloud_catalog_fingerprint,
            });
        allocation.cloud_limit = allocation.cloud_limit.min(default_cloud_limit);
        let (mut rendered, mut filesystem_budget) =
            self.render_with_cloud_limit(&catalogs, &executor, allocation.cloud_limit);

        if let Some(cloud_limit) = self.rebalance_to_retain_all_skills(
            &catalogs,
            &executor,
            &rendered,
            allocation.cloud_limit,
        ) {
            allocation.cloud_limit = cloud_limit;
            (rendered, filesystem_budget) =
                self.render_with_cloud_limit(&catalogs, &executor, allocation.cloud_limit);
        }

        [
            (CatalogKind::Executor, catalogs.executor, rendered.executor),
            (CatalogKind::Cloud, catalogs.cloud, rendered.cloud),
            (CatalogKind::Host, catalogs.host, rendered.host),
        ]
        .map(|(kind, catalog, rendered)| RenderedCatalogContribution {
            kind,
            status: catalog.status,
            budget: match kind {
                CatalogKind::Cloud => self.metadata_budget.with_limit(allocation.cloud_limit),
                CatalogKind::Executor | CatalogKind::Host => filesystem_budget,
            },
            allocation,
            rendered,
        })
    }

    /// Propose a smaller cloud cap only when omitted filesystem entries can fit without
    /// losing any cloud entries. Description truncation alone never triggers rebalancing.
    /// Within one cloud catalog and total budget, the cap can only shrink, so VM
    /// disappearance does not expand the catalog.
    fn rebalance_to_retain_all_skills(
        &self,
        catalogs: &CatalogContributions,
        executor: &PreparedSkillCatalog<'_>,
        rendered: &RenderedSkillCatalogs,
        current_cloud_limit: usize,
    ) -> Option<usize> {
        if ![&rendered.executor, &rendered.host]
            .into_iter()
            .flatten()
            .any(|catalog| catalog.report.omitted_count > 0)
        {
            return None;
        }

        let step_state = self.input.step_store.get::<SkillsStepState>();
        let cloud_aliases = step_state
            .as_ref()
            .and_then(|state| state.get())
            .map(|catalog| {
                catalog
                    .aliases
                    .for_authority(SkillToolAuthoritySelector::Cloud)
            });
        let shared = render_prepared_skill_catalogs(
            executor,
            &PreparedSkillCatalog::with_alias_plan(
                &catalogs.cloud.catalog,
                self.render_policy,
                cloud_aliases,
            ),
            &PreparedSkillCatalog::new(
                &catalogs.host.catalog,
                SkillCatalogRenderPolicy::CoreCompatible,
            ),
            self.metadata_budget,
            self.include_usage,
        );
        if [&shared.executor, &shared.cloud, &shared.host]
            .into_iter()
            .flatten()
            .any(|catalog| catalog.report.omitted_count > 0)
        {
            return None;
        }

        let cloud_limit = shared.cloud.as_ref().map_or(0, |cloud| {
            cloud.metadata_cost(self.metadata_budget, self.include_usage)
        });
        (cloud_limit < current_cloud_limit).then_some(cloud_limit)
    }

    /// Render the same buckets both before and after adjusting the allocation, so
    /// the accepted cloud text is reproduced exactly on the next sampling step.
    fn render_with_cloud_limit(
        &self,
        catalogs: &CatalogContributions,
        executor: &PreparedSkillCatalog<'_>,
        cloud_limit: usize,
    ) -> (RenderedSkillCatalogs, SkillMetadataBudget) {
        if !self.config.include_instructions {
            return (RenderedSkillCatalogs::default(), self.metadata_budget);
        }

        let step_state = self.input.step_store.get::<SkillsStepState>();
        let cloud_aliases = step_state
            .as_ref()
            .and_then(|state| state.get())
            .map(|catalog| {
                catalog
                    .aliases
                    .for_authority(SkillToolAuthoritySelector::Cloud)
            });
        let cloud_budget = self.metadata_budget.with_limit(cloud_limit);
        let cloud = render_available_skills(
            &catalogs.cloud.catalog,
            self.render_policy,
            cloud_budget,
            self.include_usage,
            cloud_aliases,
        );
        // Empty or disabled cloud catalogs leave the full allowance to filesystem skills.
        let cloud_cost = cloud.as_ref().map_or(0, |rendered| {
            rendered.metadata_cost(cloud_budget, self.include_usage)
        });
        let filesystem_budget = self
            .metadata_budget
            .with_limit(self.metadata_budget.limit().saturating_sub(cloud_cost));
        let mut rendered = render_prepared_skill_catalogs(
            executor,
            &PreparedSkillCatalog::new(&SkillCatalog::default(), self.render_policy),
            &PreparedSkillCatalog::new(
                &catalogs.host.catalog,
                SkillCatalogRenderPolicy::CoreCompatible,
            ),
            filesystem_budget,
            self.include_usage,
        );
        rendered.cloud = cloud;

        (rendered, filesystem_budget)
    }

    pub(crate) fn build_world_state_section(
        &self,
        catalog: RenderedCatalogContribution,
    ) -> WorldStateSectionContribution {
        let RenderedCatalogContribution {
            kind,
            status,
            budget,
            allocation,
            rendered,
        } = catalog;
        let report = rendered
            .as_ref()
            .map(|rendered| rendered.report.clone())
            .unwrap_or_default();
        let body = rendered
            .and_then(|rendered| rendered.into_fragment(self.include_usage))
            .map(|fragment| fragment.body().into_text().unwrap_or_default());
        let include_instructions = self.config.include_instructions;
        let metrics = self.input.extension_metrics.clone();
        let warning_emitter = Arc::clone(&self.warning_emitter);
        let render_report = report.clone();
        let on_render: CatalogRenderCallback = Box::new(move || {
            if !include_instructions || status != CatalogStatus::Enabled {
                return;
            }

            record_catalog_render(
                metrics.as_deref(),
                kind.metrics_surface(),
                budget,
                &render_report,
            );
            if let Some(message) = render_report.warning_message() {
                warning_emitter(message);
            }
        });

        match kind {
            CatalogKind::Executor => executor_skills_world_state_section(
                body,
                include_instructions,
                self.input
                    .previous_world_state
                    .and_then(|state| state.get(crate::world_state::SKILLS_WORLD_STATE_ID)),
                on_render,
            ),
            CatalogKind::Cloud => cloud_skills_world_state_section(
                body,
                include_instructions,
                status == CatalogStatus::Enabled,
                allocation,
                on_render,
            ),
            CatalogKind::Host => {
                self.input.turn_store.insert(HostSkillsCatalogInWorldState);
                host_skills_world_state_section(body, include_instructions, &report, on_render)
            }
        }
    }
}
