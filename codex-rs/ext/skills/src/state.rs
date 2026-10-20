//! Shared skill lifecycle state, including session resources and thread-owned caches.
//! Keeps discovery caches and live authorization checks separate from step preparation.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use codex_exec_server::EnvironmentAccessKey;
use codex_extension_api::ExtensionMetrics;
use codex_mcp::McpResourceClient;

use crate::SkillsExtensionConfig;
use crate::aliases::SkillAliasState;
use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillSourceKind;
use crate::catalog_state::CachedExecutorCatalog;
use crate::catalog_state::CachedExecutorDiscoveryCatalog;
use crate::catalog_state::CachedExecutorV2Catalog;
use crate::catalog_state::CloudResourceCache;
use crate::catalog_state::CloudSkillGeneration;
use crate::catalog_state::ExecutorCatalogCacheKey;
use crate::catalog_state::SkillCatalogState;
use crate::catalog_state::SkillReadCacheKey;
use crate::provider::SkillListQuery;
use crate::provider::SkillReadContext;
use crate::provider::SkillReadRequest;
use crate::shadow_selection_experiment::RecentSkillInvocations;
use crate::shadow_selection_experiment::ShadowSelectionTurnState;
use crate::shadow_selection_experiment::ShadowTaskContext;
use crate::sources::SkillProviders;
use crate::step_state::SkillStepCatalog;

#[cfg(test)]
#[path = "cloud_cache_tests.rs"]
mod cloud_cache_tests;

pub(crate) struct SkillsSessionState {
    pub(crate) mcp_resources: Option<Arc<McpResourceClient>>,
    pub(crate) extension_metrics: Option<Arc<dyn ExtensionMetrics>>,
}

/// Thread-owned skill configuration and caches; consumers can only read catalog snapshots.
pub struct SkillsThreadState {
    config: Mutex<SkillsExtensionConfig>,
    cloud_skills_available: bool,
    catalog_state: Mutex<SkillCatalogState>,
    aliases: SkillAliasState,
    shadow_selection_turn: Mutex<Option<ShadowSelectionTurn>>,
    pub(crate) executor_read_snapshot: Mutex<Option<ExecutorReadSnapshot>>,
    pub(crate) recent_skill_invocations: Arc<RecentSkillInvocations>,
    pub(crate) shadow_task_context: Arc<ShadowTaskContext>,
}

impl SkillsThreadState {
    pub(crate) fn new(config: SkillsExtensionConfig, cloud_skills_available: bool) -> Self {
        Self {
            config: Mutex::new(config),
            cloud_skills_available,
            catalog_state: Mutex::new(SkillCatalogState::default()),
            aliases: SkillAliasState::default(),
            shadow_selection_turn: Mutex::new(None),
            executor_read_snapshot: Mutex::new(None),
            recent_skill_invocations: Arc::new(RecentSkillInvocations::default()),
            shadow_task_context: Arc::new(ShadowTaskContext::default()),
        }
    }

    pub(crate) fn config(&self) -> SkillsExtensionConfig {
        self.config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn set_config(&self, config: SkillsExtensionConfig) {
        let mut previous = self
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if previous.cloud_skill_enabled != config.cloud_skill_enabled {
            let mut catalogs = self
                .catalog_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            catalogs.cloud_generation = None;
            catalogs.cloud_resources = CloudResourceCache::default();
        }
        *previous = config;
    }

    pub(crate) fn cloud_skills_available(&self) -> bool {
        self.cloud_skills_available
    }

    pub(crate) fn cloud_skill_enabled(&self) -> bool {
        self.cloud_skills_available && self.config().cloud_skill_enabled
    }

    pub(crate) fn prepare_step_catalog(
        &self,
        cloud: Option<SkillCatalog>,
        executor: SkillCatalog,
        discovery_v2: bool,
    ) -> SkillStepCatalog {
        let mut step = SkillStepCatalog::new(cloud, executor, discovery_v2, Default::default());
        step.aliases = self.aliases.capture(
            step.merged
                .entries
                .iter()
                .flat_map(super::catalog::SkillCatalogEntry::source_locations)
                .filter(|source| source.enabled && source.prompt_visible)
                .filter_map(|source| source.alias_root.map(|root| (source.authority.kind, root))),
        );
        step
    }

    /// Returns the executor catalog, reusing matching discovery caches.
    #[tracing::instrument(
        name = "skills.executor.refresh_executor_catalog",
        level = "info",
        skip_all,
        fields(root_count = query.executor_roots.len())
    )]
    pub(crate) async fn refresh_executor_catalog(
        &self,
        providers: &SkillProviders,
        query: SkillListQuery,
    ) -> SkillCatalog {
        if let Some(snapshots) = &query.executor_capability_discovery_v2 {
            let snapshots = snapshots.clone();
            let cached = self
                .catalog_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .executor_v2_cache
                .as_ref()
                .filter(|cached| cached.matches(&snapshots))
                .map(|cached| cached.catalog.clone());
            match cached {
                Some(catalog) => catalog,
                None => {
                    let catalog = providers.list_executor_for_turn(query).await;
                    self.catalog_state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .executor_v2_cache = Some(CachedExecutorV2Catalog {
                        snapshots,
                        catalog: catalog.clone(),
                    });
                    catalog
                }
            }
        } else if query.executor_capability_discovery.is_some() {
            self.executor_discovery_catalog_snapshot(providers, query)
                .await
        } else {
            if let Some(cached) = self
                .catalog_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .executor_cache
                .as_ref()
                .filter(|cached| cached.key.matches(&query))
            {
                return cached.catalog.clone();
            }

            let key = ExecutorCatalogCacheKey {
                turn_id: query.turn_id.clone(),
                resolved_roots: query.resolved_executor_roots.clone(),
                include_bundled_skills: query.include_bundled_skills,
            };
            let catalog = if query.executor_roots.is_empty() {
                SkillCatalog::default()
            } else {
                providers.list_executor_for_turn(query).await
            };
            self.catalog_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .executor_cache = Some(CachedExecutorCatalog {
                key,
                catalog: catalog.clone(),
            });
            catalog
        }
    }

    /// Reuses matching successful discovery; otherwise returns the result of this request.
    async fn executor_discovery_catalog_snapshot(
        &self,
        providers: &SkillProviders,
        query: SkillListQuery,
    ) -> SkillCatalog {
        let discovery = query.executor_capability_discovery.clone();
        let discovery_failed = discovery.as_ref().is_some_and(|discovery| {
            discovery.roots().iter().any(|root| {
                root.result
                    .as_ref()
                    .ok()
                    .is_none_or(|discovered| discovered.error.is_some())
            })
        });
        if !discovery_failed
            && let Some(cached) = self
                .catalog_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .executor_discovery_cache
                .as_ref()
                .filter(|cached| {
                    cached.matches(
                        &query.executor_roots,
                        discovery.as_ref(),
                        query.include_bundled_skills,
                    )
                })
        {
            return cached.catalog.clone();
        }

        let roots = query.executor_roots.clone();
        let include_bundled_skills = query.include_bundled_skills;
        let discovered = providers.list_executor_for_turn(query).await;
        let mut state = self
            .catalog_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Failed discovery is returned to this caller but is never reused by another request.
        state.executor_discovery_cache =
            discovery.map(|discovery| CachedExecutorDiscoveryCatalog {
                roots,
                discovery,
                include_bundled_skills,
                catalog: discovered.clone(),
            });
        discovered
    }

    /// Returns the currently authorized cached Cloud catalog without refreshing it.
    /// Preparation decides whether Cloud is enabled using its captured config.
    pub(crate) fn available_cloud_catalog(&self) -> Option<SkillCatalog> {
        self.catalog_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .cloud_generation
            .as_ref()
            .filter(|cache| cache.is_auth_current())
            .and_then(|cache| cache.catalog.clone())
    }

    /// The serialized turn-start lifecycle is the only discovery writer.
    /// Reuse warning-free discovery until invalidated; retry failures and partial catalogs next turn.
    #[tracing::instrument(name = "skills.cloud.refresh_cloud_catalog", level = "info", skip_all)]
    pub(crate) async fn refresh_cloud_catalog(
        &self,
        providers: &SkillProviders,
        query: SkillListQuery,
    ) -> SkillProviderResult<()> {
        let (cache, resource_cache_key) = {
            let config = self
                .config
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !config.cloud_skill_enabled || !self.cloud_skills_available {
                return Ok(());
            }
            let mut catalogs = self
                .catalog_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Switch auth scopes before discovery so failures retain only authorized contents.
            let cache = catalogs.prepare_cloud_generation(
                providers.cloud_mcp_resources(query.mcp_resources.as_deref()),
            );
            let resource_cache_key = cache.current_resource_cache_key();
            if cache.is_auth_current()
                && cache
                    .catalog
                    .as_ref()
                    .is_some_and(|catalog| !catalog.is_partial && catalog.warnings.is_empty())
                && cache.resource_cache_key == resource_cache_key
            {
                return Ok(());
            }
            (cache, resource_cache_key)
        };
        let mut catalog = providers.list_cloud_for_turn(query).await?;
        catalog.entries.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        let mut catalogs = self
            .catalog_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !cache.is_auth_current()
            || cache.current_resource_cache_key() != resource_cache_key
            || !catalogs
                .cloud_generation
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &cache))
        {
            return Err(SkillProviderError::new(
                "cloud skill cache changed during discovery",
            ));
        }
        if catalog.is_partial
            && let Some(previous) = cache
                .catalog
                .as_ref()
                .filter(|previous| !previous.is_partial)
        {
            catalog = previous.clone();
            catalog.is_partial = true;
        } else {
            catalogs.cloud_resources = CloudResourceCache::default();
        }
        catalogs.cloud_generation = Some(Arc::new(CloudSkillGeneration {
            auth_cache_key: cache.auth_cache_key.clone(),
            resource_cache_key,
            mcp_resources: cache.mcp_resources.clone(),
            catalog: Some(catalog),
        }));
        Ok(())
    }

    pub(crate) async fn read_skill(
        &self,
        providers: &SkillProviders,
        request: SkillReadRequest<'_>,
    ) -> SkillProviderResult<SkillReadResult> {
        if request.authority.kind != SkillSourceKind::Cloud {
            return providers.read(request).await;
        }

        let SkillReadContext::Cloud { mcp_resources } = &request.context else {
            return Err(SkillProviderError::new(
                "cloud skill reads require a cloud context",
            ));
        };
        let cache_key = SkillReadCacheKey::from(&request);
        let cache = {
            let config = self
                .config
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !config.cloud_skill_enabled || !self.cloud_skills_available {
                return Err(SkillProviderError::new("cloud skills are disabled"));
            }
            let mut catalogs = self
                .catalog_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let cache = catalogs
                .prepare_cloud_generation(providers.cloud_mcp_resources(mcp_resources.as_deref()));
            if !cache.is_auth_current() {
                return Err(SkillProviderError::new(
                    "cloud skill cache changed before read",
                ));
            }
            if let Some(result) = catalogs.cloud_resources.get(&cache_key) {
                return Ok(result);
            }
            cache
        };

        let result = providers.read(request).await?;
        let mut catalogs = self
            .catalog_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !cache.is_auth_current()
            || !catalogs
                .cloud_generation
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &cache))
        {
            return Err(SkillProviderError::new(
                "cloud skill cache changed during read",
            ));
        }
        if result.resource != cache_key.resource {
            return Ok(result);
        }
        Ok(catalogs.cloud_resources.insert(cache_key, result))
    }

    pub(crate) fn replace_shadow_selection_turn(
        &self,
        turn_id: String,
        state: Option<ShadowSelectionTurnState>,
    ) {
        *self
            .shadow_selection_turn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            state.map(|state| ShadowSelectionTurn {
                turn_id,
                state: Arc::new(state),
            });
    }

    pub(crate) fn shadow_selection_turn(
        &self,
        turn_id: &str,
    ) -> Option<Arc<ShadowSelectionTurnState>> {
        self.shadow_selection_turn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|turn| turn.turn_id == turn_id)
            .map(|turn| Arc::clone(&turn.state))
    }
}

/// One bounded executor resource, retained for continuations until replacement or thread drop.
/// Interleaved resources may evict it; misses reread and validate the content-bound cursor.
pub(crate) struct ExecutorReadSnapshot {
    pub(crate) authority: SkillAuthority,
    pub(crate) package: SkillPackageId,
    // The key binds cached contents to their source filesystem and callback permissions.
    pub(crate) access: EnvironmentAccessKey,
    pub(crate) result: Arc<SkillReadResult>,
}

struct ShadowSelectionTurn {
    turn_id: String,
    state: Arc<ShadowSelectionTurnState>,
}

#[derive(Default)]
pub(crate) struct EmittedCatalogBudgetWarnings(Mutex<HashSet<String>>);

impl EmittedCatalogBudgetWarnings {
    pub(crate) fn insert(&self, warning: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(warning.to_string())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HostSkillsCatalogInWorldState;
