use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

use codex_exec_server::Environment;
use codex_exec_server::FileSystemSandboxContext;
use codex_extension_api::ExtensionMetrics;
use codex_mcp::McpResourceClient;
use codex_protocol::capabilities::SelectedCapabilityRoot;

use crate::SkillsExtensionConfig;
use crate::SkillsExtensionState;
use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillSourceKind;
use crate::provider::SkillListQuery;
use crate::provider::SkillReadRequest;
use crate::shadow_selection_experiment::RecentSkillInvocations;
use crate::shadow_selection_experiment::ShadowSelectionTurnState;
use crate::shadow_selection_experiment::ShadowTaskContext;
use crate::skills_extension_state::CachedExecutorCatalog;
use crate::skills_extension_state::CachedExecutorDiscoveryCatalog;
use crate::skills_extension_state::CloudResourceCache;
use crate::skills_extension_state::CloudSkillGeneration;
use crate::skills_extension_state::ExecutorCatalogSelection;
use crate::skills_extension_state::SkillReadCacheKey;
use crate::sources::SkillProviders;

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
    skills_extension_state: Mutex<SkillsExtensionState>,
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
            skills_extension_state: Mutex::new(SkillsExtensionState::default()),
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
                .skills_extension_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            catalogs.cloud_cache = None;
        }
        *previous = config;
    }

    pub(crate) fn cloud_skill_enabled(&self) -> bool {
        self.cloud_skills_available && self.config().cloud_skill_enabled
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

    /// Refreshes the current step's executor catalog in the existing caches.
    #[tracing::instrument(
        name = "skills.executor.refresh_executor_catalog",
        level = "info",
        skip_all,
        fields(root_count = query.executor_roots.len())
    )]
    pub(crate) async fn refresh_executor_catalog(
        &self,
        providers: &SkillProviders,
        mut query: SkillListQuery,
    ) {
        let selection = if query.executor_capability_discovery.is_some() {
            // High-level discovery is enabled: reuse or project its discovery snapshot.
            self.executor_discovery_catalog_snapshot(providers, query)
                .await;
            ExecutorCatalogSelection::Discovery
        } else {
            // High-level discovery is not enabled: retain the legacy per-root cache path.
            let roots = std::mem::take(&mut query.executor_roots);
            for root in &roots {
                query.executor_roots = vec![root.clone()];
                self.executor_root_catalog(providers, root.clone(), query.clone())
                    .await;
            }
            ExecutorCatalogSelection::Roots(roots)
        };
        self.skills_extension_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .executor_catalog_selection = selection;
    }

    /// Reads the most recently refreshed selection without invoking providers.
    pub fn executor_catalog_snapshot(&self) -> SkillCatalog {
        let state = self
            .skills_extension_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &state.executor_catalog_selection {
            ExecutorCatalogSelection::Discovery => state
                .executor_discovery_cache
                .as_ref()
                .map(|cached| cached.catalog.clone())
                .unwrap_or_default(),
            ExecutorCatalogSelection::Roots(roots) => {
                let mut catalog = SkillCatalog::default();
                for root in roots {
                    if let Some(cached) = state
                        .executor_cache
                        .iter()
                        .find(|cached| &cached.root == root)
                    {
                        catalog.extend(cached.catalog.clone());
                    }
                }
                catalog
            }
        }
    }

    /// Reuses matching successful discovery or retains a fresh result for snapshot reads.
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
                .skills_extension_state
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
            .skills_extension_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Retain failures for read-only snapshots; discovery_failed above prevents reuse.
        state.executor_discovery_cache =
            discovery.map(|discovery| CachedExecutorDiscoveryCatalog {
                roots,
                discovery,
                include_bundled_skills,
                catalog: discovered.clone(),
            });
        discovered
    }

    #[tracing::instrument(name = "skills.executor.catalog_root", level = "info", skip_all)]
    async fn executor_root_catalog(
        &self,
        providers: &SkillProviders,
        root: SelectedCapabilityRoot,
        query: SkillListQuery,
    ) -> SkillCatalog {
        if let Some(cached) = self
            .skills_extension_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .executor_cache
            .iter()
            .find(|cached| cached.root == root)
        {
            return cached.catalog.clone();
        }

        let discovered = providers.list_executor_for_turn(query).await;
        let mut state = self
            .skills_extension_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cache = &mut state.executor_cache;
        if let Some(cached) = cache.iter().find(|cached| cached.root == root) {
            return cached.catalog.clone();
        }
        cache.push(CachedExecutorCatalog {
            root,
            catalog: discovered.clone(),
        });
        discovered
    }

    /// Reads the last authorized turn-start catalog without discovery or retries.
    /// Same-auth failures may retain it; disabled or invalidated catalogs are empty.
    pub fn cloud_catalog_snapshot(&self) -> SkillCatalog {
        let config = self
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !config.cloud_skill_enabled || !self.cloud_skills_available {
            return SkillCatalog::default();
        }
        self.skills_extension_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .cloud_cache
            .as_ref()
            .filter(|cache| cache.is_current())
            .and_then(|cache| cache.catalog.clone())
            .unwrap_or_default()
    }

    fn cloud_cache(&self, mcp_resources: Option<&McpResourceClient>) -> Arc<CloudSkillGeneration> {
        let mut state = self
            .skills_extension_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cache_key = mcp_resources
            .map(|client| client.auth_cache_key_for_server(codex_mcp::CODEX_APPS_MCP_SERVER_NAME));
        if let Some(cache) = state
            .cloud_cache
            .as_ref()
            .filter(|cache| cache.auth_cache_key == cache_key)
        {
            return Arc::clone(cache);
        }

        let next_cache = Arc::new(CloudSkillGeneration {
            auth_cache_key: cache_key,
            resource_cache_key: None,
            mcp_resources: mcp_resources.cloned(),
            catalog: None,
            resources: Mutex::new(CloudResourceCache::default()),
        });
        state.cloud_cache = Some(Arc::clone(&next_cache));
        next_cache
    }

    /// The serialized turn-start lifecycle is the only discovery writer.
    /// Reuse warning-free discovery until invalidated; retry failures and partial catalogs next turn.
    #[tracing::instrument(name = "skills.cloud.refresh_cloud_catalog", level = "info", skip_all)]
    pub(crate) async fn refresh_cloud_catalog(
        &self,
        providers: &SkillProviders,
        query: SkillListQuery,
    ) -> SkillProviderResult<()> {
        if !self.cloud_skill_enabled() {
            return Ok(());
        }
        // Switch generations before the fallible lookup; only same-auth-scope failures
        // may retain previously authorized metadata and contents.
        let cache = self.cloud_cache(query.mcp_resources.as_deref());
        let resource_cache_key = cache.current_resource_cache_key();
        if cache.is_current()
            && cache
                .catalog
                .as_ref()
                .is_some_and(|catalog| catalog.warnings.is_empty())
            && cache.resource_cache_key == resource_cache_key
        {
            return Ok(());
        }
        let mut catalog = providers.list_cloud_for_turn(query).await?;
        catalog.entries.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        let mut catalogs = self
            .skills_extension_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !cache.is_current()
            || cache.current_resource_cache_key() != resource_cache_key
            || !catalogs
                .cloud_cache
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &cache))
        {
            return Err(SkillProviderError::new(
                "cloud skill cache changed during discovery",
            ));
        }
        catalogs.cloud_cache = Some(Arc::new(CloudSkillGeneration {
            auth_cache_key: cache.auth_cache_key.clone(),
            resource_cache_key,
            mcp_resources: cache.mcp_resources.clone(),
            catalog: Some(catalog),
            resources: Mutex::new(CloudResourceCache::default()),
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

        let cache = self.cloud_cache(request.mcp_resources.as_deref());
        let cache_key = SkillReadCacheKey::from(&request);
        {
            let config = self
                .config
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !config.cloud_skill_enabled || !self.cloud_skills_available {
                return Err(SkillProviderError::new("cloud skills are disabled"));
            }
            let catalogs = self
                .skills_extension_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !cache.is_current()
                || !catalogs
                    .cloud_cache
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &cache))
            {
                return Err(SkillProviderError::new(
                    "cloud skill cache changed before read",
                ));
            }
            if let Some(result) = cache
                .resources
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&cache_key)
            {
                return Ok(result);
            }
        }

        let result = providers.read(request).await?;
        let catalogs = self
            .skills_extension_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !cache.is_current()
            || !catalogs
                .cloud_cache
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
        Ok(cache
            .resources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(cache_key, result))
    }
}

/// One bounded executor resource, retained for continuations until replacement or thread drop.
/// Interleaved resources may evict it; misses reread and validate the content-bound cursor.
pub(crate) struct ExecutorReadSnapshot {
    pub(crate) authority: SkillAuthority,
    pub(crate) package: SkillPackageId,
    // Named environments can be replaced; do not reuse their old resource or keep them alive.
    pub(crate) environment: Weak<Environment>,
    pub(crate) sandbox: Option<FileSystemSandboxContext>,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SkillsTurnState {
    pub(crate) catalog: SkillCatalog,
    pub(crate) selected_entries: Vec<SkillCatalogEntry>,
    pub(crate) warnings: Vec<String>,
    pub(crate) main_prompts_injected: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HostSkillsCatalogInWorldState;

#[derive(Clone, Debug, Default)]
pub(crate) struct ExecutorSkillsStepState(pub(crate) SkillCatalog);

#[derive(Clone, Debug, Default)]
pub(crate) struct HostSkillsStepState(pub(crate) SkillCatalog);
