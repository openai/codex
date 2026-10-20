//! Reusable provider catalog caches and Cloud resource contents.
//! Provider catalogs remain unfiltered until step preparation applies policy.

use std::collections::HashMap;
use std::sync::Arc;

use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::McpResourceClient;
use codex_mcp::McpResourceClientAuthKey;
use codex_mcp::McpResourceServerCacheKey;
use codex_protocol::capabilities::SelectedCapabilityRoot;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::provider::SkillReadRequest;

const MAX_CACHED_CLOUD_RESOURCES: usize = 100;
const MAX_CACHED_CLOUD_CONTENT_BYTES: usize = 8 * 1024 * 1024;

/// Reusable provider catalogs, before step-specific plugin policy and merging.
#[derive(Clone, Debug, Default)]
pub(crate) struct SkillCatalogState {
    // Roots-only discovery, reused within a turn.
    pub(crate) executor_cache: Option<CachedExecutorCatalog>,
    // DiscoveryV1.
    pub(crate) executor_discovery_cache: Option<CachedExecutorDiscoveryCatalog>,
    // DiscoveryV2.
    pub(crate) executor_v2_cache: Option<CachedExecutorV2Catalog>,
    // Shared by both discovery modes.
    pub(crate) cloud_generation: Option<Arc<CloudSkillGeneration>>,
    pub(crate) cloud_resources: CloudResourceCache,
}

impl SkillCatalogState {
    /// Reuses the auth scope or replaces it, clearing contents from the previous scope.
    pub(crate) fn prepare_cloud_generation(
        &mut self,
        mcp_resources: Option<&McpResourceClient>,
    ) -> Arc<CloudSkillGeneration> {
        let auth_cache_key = mcp_resources
            .map(|client| client.auth_cache_key_for_server(codex_mcp::CODEX_APPS_MCP_SERVER_NAME));
        if let Some(generation) = self
            .cloud_generation
            .as_ref()
            .filter(|generation| generation.auth_cache_key == auth_cache_key)
        {
            return Arc::clone(generation);
        }

        let generation = Arc::new(CloudSkillGeneration {
            auth_cache_key,
            resource_cache_key: None,
            mcp_resources: mcp_resources.cloned(),
            catalog: None,
        });
        self.cloud_generation = Some(Arc::clone(&generation));
        self.cloud_resources = CloudResourceCache::default();
        generation
    }
}

/// Keeps cloud metadata within one published auth scope and Apps availability state.
/// Warning-free discovery is reused until the Apps resource generation changes.
/// A published refresh replaces the Arc so late reads cannot populate its successor.
pub(crate) struct CloudSkillGeneration {
    pub(crate) auth_cache_key: Option<McpResourceClientAuthKey>,
    pub(crate) resource_cache_key: Option<McpResourceServerCacheKey>,
    pub(crate) mcp_resources: Option<McpResourceClient>,
    pub(crate) catalog: Option<SkillCatalog>,
}

impl CloudSkillGeneration {
    pub(crate) fn current_resource_cache_key(&self) -> Option<McpResourceServerCacheKey> {
        self.mcp_resources
            .as_ref()
            .and_then(|client| client.server_cache_key(codex_mcp::CODEX_APPS_MCP_SERVER_NAME))
    }

    pub(crate) fn is_auth_current(&self) -> bool {
        self.auth_cache_key
            == self.mcp_resources.as_ref().map(|client| {
                client.auth_cache_key_for_server(codex_mcp::CODEX_APPS_MCP_SERVER_NAME)
            })
    }
}

impl std::fmt::Debug for CloudSkillGeneration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CloudSkillGeneration")
            .field("catalog", &self.catalog)
            .finish_non_exhaustive()
    }
}

/// Reuses the projection of unchanged successful V2 inventories, before plugin policy.
#[derive(Clone, Debug)]
pub(crate) struct CachedExecutorV2Catalog {
    pub(crate) snapshots: Vec<codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot>,
    pub(crate) catalog: SkillCatalog,
}

impl CachedExecutorV2Catalog {
    pub(crate) fn matches(
        &self,
        snapshots: &[codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot],
    ) -> bool {
        self.snapshots.len() == snapshots.len()
            && self
                .snapshots
                .iter()
                .zip(snapshots)
                .all(|(previous, current)| {
                    current.result.is_ok() && previous.same_discovery(current)
                })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ExecutorCatalogCacheKey {
    pub(crate) turn_id: String,
    pub(crate) resolved_roots: Vec<ResolvedSelectedCapabilityRoot>,
    pub(crate) include_bundled_skills: bool,
}

impl ExecutorCatalogCacheKey {
    pub(crate) fn matches(&self, query: &crate::provider::SkillListQuery) -> bool {
        self.turn_id == query.turn_id
            && self
                .resolved_roots
                .iter()
                .map(ResolvedSelectedCapabilityRoot::selected_root)
                .eq(query.executor_roots.iter())
            && self.include_bundled_skills == query.include_bundled_skills
            // Unbound callers cannot establish identity for a cached filesystem catalog.
            && query.resolved_executor_roots.len() == query.executor_roots.len()
            && self.resolved_roots.iter().zip(&query.resolved_executor_roots).all(|(previous, current)| {
                previous.selected_root() == current.selected_root()
                    && Arc::ptr_eq(previous.environment(), current.environment())
            })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CachedExecutorCatalog {
    pub(crate) key: ExecutorCatalogCacheKey,
    pub(crate) catalog: SkillCatalog,
}

/// Keeps the latest result; only successful results for unchanged inputs can be reused.
#[derive(Clone, Debug)]
pub(crate) struct CachedExecutorDiscoveryCatalog {
    pub(crate) roots: Vec<SelectedCapabilityRoot>,
    pub(crate) discovery: ExecutorCapabilityDiscoverySnapshot,
    pub(crate) include_bundled_skills: bool,
    pub(crate) catalog: SkillCatalog,
}

impl CachedExecutorDiscoveryCatalog {
    pub(crate) fn matches(
        &self,
        roots: &[SelectedCapabilityRoot],
        discovery: Option<&ExecutorCapabilityDiscoverySnapshot>,
        include_bundled_skills: bool,
    ) -> bool {
        discovery.is_some_and(|discovery| {
            self.roots == roots
                && self.include_bundled_skills == include_bundled_skills
                && self.discovery.sandbox_contexts() == discovery.sandbox_contexts()
                && self.discovery.roots().len() == discovery.roots().len()
                && self.discovery.roots().iter().zip(discovery.roots()).all(
                    |(previous, current)| {
                        previous.selected_root == current.selected_root
                            && match (&previous.result, &current.result) {
                                (Ok(previous), Ok(current)) => Arc::ptr_eq(previous, current),
                                _ => false,
                            }
                    },
                )
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SkillReadCacheKey {
    authority: SkillAuthority,
    package: SkillPackageId,
    pub(crate) resource: SkillResourceId,
}

impl From<&SkillReadRequest<'_>> for SkillReadCacheKey {
    fn from(request: &SkillReadRequest<'_>) -> Self {
        Self {
            authority: request.authority.clone(),
            package: request.package.clone(),
            resource: request.resource.clone(),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct CloudResourceCache {
    entries: HashMap<SkillReadCacheKey, SkillReadResult>,
    contents_bytes: usize,
}

impl CloudResourceCache {
    pub(crate) fn get(&self, key: &SkillReadCacheKey) -> Option<SkillReadResult> {
        self.entries.get(key).cloned()
    }

    pub(crate) fn insert(
        &mut self,
        key: SkillReadCacheKey,
        result: SkillReadResult,
    ) -> SkillReadResult {
        if let Some(cached) = self.entries.get(&key) {
            return cached.clone();
        }

        let contents_bytes = result.contents.len();
        let Some(next_contents_bytes) = self.contents_bytes.checked_add(contents_bytes) else {
            return result;
        };
        if self.entries.len() >= MAX_CACHED_CLOUD_RESOURCES
            || next_contents_bytes > MAX_CACHED_CLOUD_CONTENT_BYTES
        {
            return result;
        }

        self.contents_bytes = next_contents_bytes;
        self.entries.insert(key, result.clone());
        result
    }
}

impl std::fmt::Debug for CloudResourceCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CloudResourceCache")
            .field("entry_count", &self.entries.len())
            .field("contents_bytes", &self.contents_bytes)
            .finish_non_exhaustive()
    }
}
