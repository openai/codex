//! Renders prepared step catalogs without performing discovery or changing tool readiness.

use std::sync::Arc;

use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use serde::Deserialize;
use serde::Serialize;

use crate::catalog::SkillCatalog;
use crate::catalog_delta::CatalogDelta;
use crate::catalog_delta::CatalogUsage;
use crate::catalog_delta::skills_delta_section;
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
use crate::state::EmittedCatalogBudgetWarnings;
use crate::state::HostSkillsCatalogInWorldState;
use crate::step_state::CatalogStatus;
use crate::step_state::SkillToolAuthoritySelector;
use crate::step_state::SkillsStepState;
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

struct CatalogContributions {
    executor: CatalogContribution,
    cloud: CatalogContribution,
    host: CatalogContribution,
}

struct RenderedCatalogContribution {
    kind: CatalogKind,
    status: CatalogStatus,
    budget: SkillMetadataBudget,
    allocation: CatalogBudgetAllocation,
    rendered: Option<AvailableSkillsRender>,
}

struct CatalogRenderingContext<'a> {
    input: WorldStateContributionInput<'a>,
    step_state: Arc<SkillsStepState>,
    metadata_budget: SkillMetadataBudget,
    render_policy: SkillCatalogRenderPolicy,
    include_usage: bool,
    warning_emitter: CatalogWarningEmitter,
}

impl<'a> CatalogRenderingContext<'a> {
    fn new(
        event_sink: Arc<dyn ExtensionEventSink>,
        input: WorldStateContributionInput<'a>,
    ) -> Option<Self> {
        let step_state = input.step_store.get::<SkillsStepState>()?;
        let include_usage = input.model_info.include_skills_usage_instructions;
        let context_window = input.model_info.resolved_context_window();
        let metadata_budget =
            skill_metadata_budget(context_window, step_state.config.max_context_tokens);
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

        let render_policy = if step_state.discovery_v2 {
            SkillCatalogRenderPolicy::DiscoveryV2
        } else {
            SkillCatalogRenderPolicy::ExtensionCompatible
        };
        Some(Self {
            input,
            step_state,
            metadata_budget,
            render_policy,
            include_usage,
            warning_emitter,
        })
    }

    fn render_world_state_catalogs(&self) -> [RenderedCatalogContribution; 3] {
        let catalogs = CatalogContributions {
            executor: CatalogContribution {
                catalog: self
                    .step_state
                    .catalog
                    .select_from_catalog(SkillToolAuthoritySelector::Executor),
                status: CatalogStatus::Enabled,
            },
            cloud: CatalogContribution {
                catalog: self
                    .step_state
                    .catalog
                    .select_from_catalog(SkillToolAuthoritySelector::Cloud),
                status: self.step_state.cloud_status,
            },
            host: self.step_state.host_catalog.as_ref().map_or_else(
                CatalogContribution::unavailable,
                |catalog| CatalogContribution {
                    catalog: catalog.clone(),
                    status: CatalogStatus::Enabled,
                },
            ),
        };
        let total_limit = self.metadata_budget.limit();
        let default_cloud_limit = total_limit - total_limit / FILESYSTEM_BUDGET_DIVISOR;
        let executor_aliases = Some(
            self.step_state
                .catalog
                .aliases
                .for_authority(SkillToolAuthoritySelector::Executor),
        );
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

        if self.step_state.config.catalog_deltas_enabled {
            let cloud = PreparedSkillCatalog::with_alias_plan(
                &catalogs.cloud.catalog,
                self.render_policy,
                Some(
                    self.step_state
                        .catalog
                        .aliases
                        .for_authority(SkillToolAuthoritySelector::Cloud),
                ),
            );
            let host = PreparedSkillCatalog::new(
                &catalogs.host.catalog,
                SkillCatalogRenderPolicy::CoreCompatible,
            );
            for (catalog, rendered) in [
                (&executor, &mut rendered.executor),
                (&cloud, &mut rendered.cloud),
                (&host, &mut rendered.host),
            ] {
                if let Some(rendered) = rendered {
                    rendered.entries = catalog.entries_for_render(rendered);
                }
            }
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

        let cloud_aliases = Some(
            self.step_state
                .catalog
                .aliases
                .for_authority(SkillToolAuthoritySelector::Cloud),
        );
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
        if !self.step_state.config.include_instructions {
            return (RenderedSkillCatalogs::default(), self.metadata_budget);
        }

        let cloud_aliases = Some(
            self.step_state
                .catalog
                .aliases
                .for_authority(SkillToolAuthoritySelector::Cloud),
        );
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

    fn build_world_state_section(
        &self,
        catalog: RenderedCatalogContribution,
        remaining: &mut SkillMetadataBudget,
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
        let include_instructions = self.step_state.config.include_instructions;
        let catalog_deltas_enabled = self.step_state.config.catalog_deltas_enabled;
        let metrics = self.input.extension_metrics.clone();
        let warning_emitter = Arc::clone(&self.warning_emitter);
        let on_render = move |render_report: &crate::render::SkillRenderReport| {
            if !include_instructions || status != CatalogStatus::Enabled {
                return;
            }

            record_catalog_render(
                metrics.as_deref(),
                kind.metrics_surface(),
                budget,
                render_report,
            );
            if let Some(message) = render_report.warning_message(catalog_deltas_enabled) {
                warning_emitter(message);
            }
        };

        if catalog_deltas_enabled {
            let (id, source) = match kind {
                CatalogKind::Executor => (
                    crate::world_state::SKILLS_WORLD_STATE_ID,
                    "Selected-environment",
                ),
                CatalogKind::Cloud => (crate::world_state::CLOUD_SKILLS_WORLD_STATE_ID, "Cloud"),
                CatalogKind::Host => {
                    if status != CatalogStatus::Unavailable {
                        self.input.turn_store.insert(HostSkillsCatalogInWorldState);
                    }
                    (crate::world_state::HOST_SKILLS_WORLD_STATE_ID, "Host")
                }
            };
            return skills_delta_section(
                id,
                source,
                CatalogDelta {
                    rendered,
                    include_usage: self.include_usage,
                    allocation: (kind == CatalogKind::Cloud).then_some(allocation),
                },
                self.input
                    .previous_world_state
                    .and_then(|state| state.get(id)),
                remaining,
                on_render,
            );
        }

        let body = rendered
            .and_then(|rendered| rendered.into_fragment(self.include_usage))
            .map(|fragment| fragment.body().into_text().unwrap_or_default());
        let render_report = report.clone();
        let on_render: CatalogRenderCallback = Box::new(move || on_render(&render_report));
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

/// Renders catalog sections, retaining empty delta snapshots for shared budget accounting.
pub(crate) fn render_world_state_sections(
    event_sink: Arc<dyn ExtensionEventSink>,
    input: WorldStateContributionInput<'_>,
) -> Vec<WorldStateSectionContribution> {
    let Some(context) = CatalogRenderingContext::new(event_sink, input) else {
        return Vec::new();
    };
    let used = context
        .input
        .previous_world_state
        .into_iter()
        .flat_map(|state| {
            [
                crate::world_state::SKILLS_WORLD_STATE_ID,
                crate::world_state::CLOUD_SKILLS_WORLD_STATE_ID,
                crate::world_state::HOST_SKILLS_WORLD_STATE_ID,
            ]
            .into_iter()
            .filter_map(|id| state.get(id))
        })
        .filter_map(|snapshot| snapshot.get("usage"))
        .filter_map(|usage| CatalogUsage::deserialize(usage).ok())
        .map(|usage| usage.cost(context.metadata_budget))
        .sum::<usize>();
    let mut remaining = context
        .metadata_budget
        .with_limit(context.metadata_budget.limit().saturating_sub(used));
    context
        .render_world_state_catalogs()
        .into_iter()
        // Keep empty snapshots too: a source that becomes available later must use
        // the same remaining budget instead of publishing another initial catalog.
        .filter(|catalog| {
            context.step_state.config.catalog_deltas_enabled
                || catalog.status != CatalogStatus::Unavailable
        })
        .map(|catalog| context.build_world_state_section(catalog, &mut remaining))
        .collect()
}
