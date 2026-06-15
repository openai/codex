use codex_protocol::capabilities::SelectedCapabilityRoot;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;
use tokio::sync::OnceCell;

use crate::SkillsExtensionConfig;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillSourceKind;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillAuthority;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillResourceId;
use crate::provider::SkillReadRequest;
use crate::sources::SkillProviders;

const MAX_CACHED_ORCHESTRATOR_RESOURCES: usize = 100;
const MAX_CACHED_ORCHESTRATOR_CONTENT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct SkillsThreadState {
    config: Mutex<SkillsExtensionConfig>,
    selected_roots: Vec<SelectedCapabilityRoot>,
    orchestrator_skills_enabled: bool,
    orchestrator_catalog: OnceCell<SkillCatalog>,
    orchestrator_resources: Mutex<OrchestratorResourceCache>,
}

impl SkillsThreadState {
    pub(crate) fn new(
        config: SkillsExtensionConfig,
        selected_roots: Vec<SelectedCapabilityRoot>,
        orchestrator_skills_enabled: bool,
    ) -> Self {
        Self {
            config: Mutex::new(config),
            selected_roots,
            orchestrator_skills_enabled,
            orchestrator_catalog: OnceCell::new(),
            orchestrator_resources: Mutex::new(OrchestratorResourceCache::default()),
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

    pub(crate) fn selected_roots(&self) -> &[SelectedCapabilityRoot] {
        &self.selected_roots
    }

    pub(crate) fn orchestrator_skills_enabled(&self) -> bool {
        self.orchestrator_skills_enabled
    }

    pub(crate) async fn orchestrator_catalog_snapshot(
        &self,
        initialize: impl Future<Output = Result<SkillCatalog, SkillProviderError>> + Send,
    ) -> SkillCatalog {
        self.orchestrator_catalog
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

        let cache_key = SkillReadCacheKey::from(&request);
        if let Some(result) = self
            .orchestrator_resources
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

        Ok(self
            .orchestrator_resources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(cache_key, result))
    }
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

#[derive(Debug, Default)]
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
