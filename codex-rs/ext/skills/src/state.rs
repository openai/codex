use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_core_skills::runtime::SkillAuthority;
use codex_core_skills::runtime::SkillCatalog;
use codex_core_skills::runtime::SkillPackageId;
use codex_core_skills::runtime::SkillReadRequest;
use codex_core_skills::runtime::SkillReadResult;
use codex_core_skills::runtime::SkillResourceId;
use codex_core_skills::runtime::SkillSourceError;
use codex_core_skills::runtime::SkillSourceResult;
use codex_mcp::McpResourceClient;
use codex_mcp::McpResourceClientCacheKey;
use tokio::sync::OnceCell;

const MAX_CACHED_ORCHESTRATOR_RESOURCES: usize = 100;
const MAX_CACHED_ORCHESTRATOR_CONTENT_BYTES: usize = 8 * 1024 * 1024;

pub(crate) struct SkillsThreadState {
    orchestrator_skills_enabled: AtomicBool,
    orchestrator_skills_available: bool,
    orchestrator_cache: Mutex<Option<Arc<OrchestratorGenerationCache>>>,
}

impl SkillsThreadState {
    pub(crate) fn new(
        orchestrator_skills_enabled: bool,
        orchestrator_skills_available: bool,
    ) -> Self {
        Self {
            orchestrator_skills_enabled: AtomicBool::new(orchestrator_skills_enabled),
            orchestrator_skills_available,
            orchestrator_cache: Mutex::new(None),
        }
    }

    pub(crate) fn set_orchestrator_skills_enabled(&self, enabled: bool) {
        self.orchestrator_skills_enabled
            .store(enabled, Ordering::Relaxed);
    }

    pub(crate) fn orchestrator_skills_enabled(&self) -> bool {
        self.orchestrator_skills_available
            && self.orchestrator_skills_enabled.load(Ordering::Relaxed)
    }

    pub(crate) async fn orchestrator_catalog_snapshot(
        &self,
        mcp_resources: Option<&McpResourceClient>,
        initialize: impl Future<Output = Result<SkillCatalog, SkillSourceError>> + Send,
    ) -> SkillCatalog {
        let cache = self.orchestrator_cache(mcp_resources);
        if let Some(catalog) = cache.catalog.get() {
            return catalog.clone();
        }
        let catalog = initialize.await.unwrap_or_else(|err| SkillCatalog {
            warnings: vec![err.message],
            ..Default::default()
        });
        if !catalog.entries.is_empty() && catalog.warnings.is_empty() {
            let _ = cache.catalog.set(catalog.clone());
        }
        catalog
    }

    pub(crate) async fn read_orchestrator_resource(
        &self,
        request: &SkillReadRequest,
        mcp_resources: Option<&McpResourceClient>,
        read: impl Future<Output = SkillSourceResult<SkillReadResult>> + Send,
    ) -> SkillSourceResult<SkillReadResult> {
        let cache_key = SkillReadCacheKey::from(request);
        let cache = self.orchestrator_cache(mcp_resources);
        if let Some(result) = cache
            .resources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&cache_key)
        {
            return Ok(result);
        }

        let result = read.await?;
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
