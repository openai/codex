//! Stores published skill catalogs, executor discovery projections, and cached cloud resources.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
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

/// Stores published cloud and executor catalogs for a thread.
#[derive(Clone, Debug, Default)]
pub(crate) struct SkillsExtensionState {
    pub(crate) executor_catalog_selection: ExecutorCatalogSelection,
    pub(crate) executor_cache: Vec<CachedExecutorCatalog>,
    pub(crate) executor_discovery_cache: Option<CachedExecutorDiscoveryCatalog>,
    pub(crate) cloud_cache: Option<Arc<CloudSkillGeneration>>,
}

/// Selects the existing cache entries published by the most recent refresh.
#[derive(Clone, Debug, Default)]
pub(crate) enum ExecutorCatalogSelection {
    #[default]
    Discovery,
    Roots(Vec<SelectedCapabilityRoot>),
}

/// Keeps cloud metadata and contents within one published auth scope and Apps availability state.
/// Warning-free discovery is reused until the Apps resource generation changes.
/// A successful refresh replaces the Arc so late reads cannot populate its successor.
pub(crate) struct CloudSkillGeneration {
    pub(crate) auth_cache_key: Option<McpResourceClientAuthKey>,
    pub(crate) resource_cache_key: Option<McpResourceServerCacheKey>,
    pub(crate) mcp_resources: Option<McpResourceClient>,
    pub(crate) catalog: Option<SkillCatalog>,
    pub(crate) resources: Mutex<CloudResourceCache>,
}

impl CloudSkillGeneration {
    pub(crate) fn current_resource_cache_key(&self) -> Option<McpResourceServerCacheKey> {
        self.mcp_resources
            .as_ref()
            .and_then(|client| client.server_cache_key(codex_mcp::CODEX_APPS_MCP_SERVER_NAME))
    }

    pub(crate) fn is_current(&self) -> bool {
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

/// Retains the legacy per-root catalog until the thread is dropped.
#[derive(Clone, Debug)]
pub(crate) struct CachedExecutorCatalog {
    pub(crate) root: SelectedCapabilityRoot,
    pub(crate) catalog: SkillCatalog,
}

/// Retains the latest projection for snapshots; refresh only reuses successful unchanged inputs.
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
