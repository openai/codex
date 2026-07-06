use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;

use codex_mcp::McpResourceClient;
use codex_mcp::McpResourceClientCacheKey;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use tokio::sync::OnceCell;

use crate::SkillsExtensionConfig;
use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSourceKind;
use crate::provider::SkillListQuery;
use crate::provider::SkillReadRequest;
use crate::sources::SkillProviders;

const MAX_CACHED_ORCHESTRATOR_RESOURCES: usize = 100;
const MAX_CACHED_ORCHESTRATOR_CONTENT_BYTES: usize = 8 * 1024 * 1024;

pub(crate) struct SkillsThreadState {
    config: Mutex<SkillsExtensionConfig>,
    orchestrator_skills_available: bool,
    executor_cache: Mutex<Vec<CachedExecutorCatalog>>,
    orchestrator_cache: Mutex<Option<Arc<OrchestratorGenerationCache>>>,
}

impl SkillsThreadState {
    pub(crate) fn new(config: SkillsExtensionConfig, orchestrator_skills_available: bool) -> Self {
        Self {
            config: Mutex::new(config),
            orchestrator_skills_available,
            executor_cache: Mutex::new(Vec::new()),
            orchestrator_cache: Mutex::new(None),
        }
    }

    pub(crate) fn config(&self) -> SkillsExtensionConfig {
        self.config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn set_config(&self, config: SkillsExtensionConfig) {
        *self
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = config;
    }

    pub(crate) fn orchestrator_skills_enabled(&self) -> bool {
        self.orchestrator_skills_available && self.config().orchestrator_skills_enabled
    }

    /// Returns catalogs for stable selected roots.
    ///
    /// The first catalog returned for a root remains cached until this thread state is dropped.
    /// Environment availability only controls whether the root is projected into the current
    /// step; it never invalidates the cache. There is intentionally no filesystem watcher or
    /// content-based invalidation because selected environment roots are treated as stable.
    #[tracing::instrument(
        name = "skills.executor.catalog_snapshot",
        level = "info",
        skip_all,
        fields(root_count = query.executor_roots.len())
    )]
    pub(crate) async fn executor_catalog_snapshot(
        &self,
        providers: &SkillProviders,
        mut query: SkillListQuery,
    ) -> SkillCatalog {
        let roots = std::mem::take(&mut query.executor_roots);
        let mut catalog = SkillCatalog::default();
        for root in roots {
            query.executor_roots = vec![root.clone()];
            catalog.extend(
                self.executor_root_catalog(providers, root, query.clone())
                    .await,
            );
        }
        catalog
    }

    pub(crate) async fn orchestrator_catalog_snapshot(
        &self,
        mcp_resources: Option<&McpResourceClient>,
        initialize: impl Future<Output = Result<SkillCatalog, SkillProviderError>> + Send,
    ) -> SkillCatalog {
        self.orchestrator_cache(mcp_resources)
            .catalog
            .get_or_init(|| async {
                initialize.await.unwrap_or_else(|err| SkillCatalog {
                    warnings: vec![err.message],
                    ..Default::default()
                })
            })
            .await
            .clone()
    }

    pub(crate) async fn read_skill(
        &self,
        providers: &SkillProviders,
        request: SkillReadRequest,
    ) -> SkillProviderResult<SkillReadResult> {
        if request.authority.kind != SkillSourceKind::Orchestrator {
            return providers.read(request).await;
        }

        let cache = self.orchestrator_cache(request.mcp_resources.as_deref());
        let cache_key = SkillReadCacheKey::from(&request);
        if let Some(result) = cache
            .resources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&cache_key)
        {
            return Ok(result);
        }

        let result = providers.read(request).await?;
        if result.resource != cache_key.resource {
            return Ok(result);
        }

        Ok(cache
            .resources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(cache_key, result))
    }

    fn orchestrator_cache(
        &self,
        mcp_resources: Option<&McpResourceClient>,
    ) -> Arc<OrchestratorGenerationCache> {
        let mut cache = self
            .orchestrator_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cache_key = mcp_resources.map(McpResourceClient::cache_key);
        if let Some(cache) = cache
            .as_ref()
            .filter(|cache| cache.mcp_cache_key == cache_key)
        {
            return Arc::clone(cache);
        }

        let next_cache = Arc::new(OrchestratorGenerationCache {
            mcp_cache_key: cache_key,
            catalog: OnceCell::new(),
            resources: Mutex::new(OrchestratorResourceCache::default()),
        });
        *cache = Some(Arc::clone(&next_cache));
        next_cache
    }

    #[tracing::instrument(name = "skills.executor.catalog_root", level = "info", skip_all)]
    async fn executor_root_catalog(
        &self,
        providers: &SkillProviders,
        root: SelectedCapabilityRoot,
        query: SkillListQuery,
    ) -> SkillCatalog {
        if let Some(cached) = self
            .executor_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|cached| cached.root == root)
        {
            return cached.catalog.clone();
        }

        let discovered = providers.list_executor_for_turn(query).await;
        let mut cache = self
            .executor_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = cache.iter().find(|cached| cached.root == root) {
            return cached.catalog.clone();
        }
        cache.push(CachedExecutorCatalog {
            root,
            catalog: discovered.clone(),
        });
        discovered
    }
}

struct CachedExecutorCatalog {
    root: SelectedCapabilityRoot,
    catalog: SkillCatalog,
}

struct OrchestratorGenerationCache {
    mcp_cache_key: Option<McpResourceClientCacheKey>,
    catalog: OnceCell<SkillCatalog>,
    resources: Mutex<OrchestratorResourceCache>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SkillReadCacheKey {
    authority: SkillAuthority,
    package: SkillPackageId,
    resource: SkillResourceId,
}

impl From<&SkillReadRequest> for SkillReadCacheKey {
    fn from(request: &SkillReadRequest) -> Self {
        Self {
            authority: request.authority.clone(),
            package: request.package.clone(),
            resource: request.resource.clone(),
        }
    }
}

#[derive(Default)]
struct OrchestratorResourceCache {
    entries: HashMap<SkillReadCacheKey, SkillReadResult>,
    contents_bytes: usize,
}

impl OrchestratorResourceCache {
    fn get(&self, key: &SkillReadCacheKey) -> Option<SkillReadResult> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: SkillReadCacheKey, result: SkillReadResult) -> SkillReadResult {
        if let Some(cached) = self.entries.get(&key) {
            return cached.clone();
        }

        let contents_bytes = result.contents.len();
        let Some(next_contents_bytes) = self.contents_bytes.checked_add(contents_bytes) else {
            return result;
        };
        if self.entries.len() >= MAX_CACHED_ORCHESTRATOR_RESOURCES
            || next_contents_bytes > MAX_CACHED_ORCHESTRATOR_CONTENT_BYTES
        {
            return result;
        }

        self.contents_bytes = next_contents_bytes;
        self.entries.insert(key, result.clone());
        result
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SkillsTurnState {
    pub(crate) catalog: SkillCatalog,
    pub(crate) selected_entries: Vec<SkillCatalogEntry>,
    pub(crate) warnings: Vec<String>,
    pub(crate) main_prompts_injected: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExecutorSkillsStepState(pub(crate) SkillCatalog);
