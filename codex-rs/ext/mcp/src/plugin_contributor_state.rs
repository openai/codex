//! Thread-owned catalogs, reused within their auth and resource scope, and selected-root metadata.
//! A short-lived mutex guards publication; failed refreshes preserve same-auth fallback data.

use crate::catalog_observability::record_catalog;
use codex_core_plugins::PluginCatalog;
use codex_core_plugins::PluginRootOwnership;
use codex_core_plugins::catalog_from_snapshots;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_exec_server::FileSystemSandboxContext;
use codex_extension_api::CatalogRefreshBackoff;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::SelectedPluginContribution;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpResourceClient;
use codex_mcp::McpResourceClientAuthKey;
use codex_mcp::McpResourceServerCacheKey;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

/// Plugin configuration and caches owned by one thread; mutable state stays extension-private.
#[derive(Default)]
pub struct PluginsThreadState {
    state: Mutex<PluginContributorState>,
}

impl PluginsThreadState {
    /// Returns an owned cloud snapshot only while its auth generation is current.
    pub fn cloud_catalog(&self) -> Option<PluginCatalog> {
        self.contributor_state().cloud_catalog().cloned()
    }

    pub(crate) fn contributor_state(&self) -> MutexGuard<'_, PluginContributorState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// `None` is unpublished; an empty catalog is a successful discovery result.
#[derive(Default)]
pub(crate) struct PluginContributorState {
    pub(crate) extension_metrics: Option<Arc<dyn ExtensionMetrics>>,
    pub(crate) plugins_enabled: bool,
    pub(crate) cloud_generation: Option<CloudPluginGeneration>,
    pub(crate) cloud_retry: Option<CatalogRefreshBackoff>,
    pub(crate) executor_catalog: Option<PluginCatalog>,
    executor_snapshots: Vec<ExecutorCapabilityDiscoveryV2Snapshot>,
    pub(crate) executor_cache: Vec<CachedSelectedRoot>,
}

/// Keeps cloud plugin metadata within one published auth scope and Apps availability state.
pub(crate) struct CloudPluginGeneration {
    pub(crate) auth_cache_key: Option<McpResourceClientAuthKey>,
    pub(crate) mcp_resources: Option<McpResourceClient>,
    pub(crate) catalog: Option<PluginCatalog>,
    pub(crate) resource_cache_key: Option<McpResourceServerCacheKey>,
}

impl CloudPluginGeneration {
    pub(crate) fn current_resource_cache_key(&self) -> Option<McpResourceServerCacheKey> {
        self.mcp_resources
            .as_ref()
            .and_then(|client| client.server_cache_key(CODEX_APPS_MCP_SERVER_NAME))
    }

    pub(crate) fn is_auth_current(&self) -> bool {
        self.auth_cache_key
            == self
                .mcp_resources
                .as_ref()
                .map(|client| client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME))
    }
}

impl PluginContributorState {
    /// Reuses unchanged discovery and preserves surviving installations' order on refresh.
    pub(crate) fn update_executor_catalog(
        &mut self,
        snapshots: &[ExecutorCapabilityDiscoveryV2Snapshot],
    ) {
        if self.executor_catalog.is_some() && self.executor_snapshots == snapshots {
            return;
        }
        let mut catalog = catalog_from_snapshots(snapshots);
        if let Some(previous) = &self.executor_catalog {
            // Preserve installation order to keep MCPs on the same executor while it remains available.
            let mut previous_ranks = HashMap::with_capacity(previous.entries.len());
            for (rank, entry) in previous.entries.iter().enumerate() {
                previous_ranks
                    .entry((&entry.id, entry.locations.as_slice()))
                    .or_insert(rank);
            }
            catalog.entries.sort_by_key(|entry| {
                previous_ranks
                    .get(&(&entry.id, entry.locations.as_slice()))
                    .copied()
                    .unwrap_or(usize::MAX)
            });
        }
        record_catalog(self.extension_metrics.as_deref(), "executor", &catalog);
        self.executor_catalog = Some(catalog);
        self.executor_snapshots = snapshots.to_vec();
    }

    pub(crate) fn clear_executor_catalog(&mut self) {
        self.executor_catalog = None;
        self.executor_snapshots.clear();
    }

    /// Returns the cloud catalog only while it belongs to the live auth generation.
    pub(crate) fn cloud_catalog(&self) -> Option<&PluginCatalog> {
        self.cloud_generation
            .as_ref()
            .filter(|generation| generation.is_auth_current())
            .and_then(|generation| generation.catalog.as_ref())
    }

    /// Establishes the auth scope and returns the key used by that generation.
    pub(crate) fn prepare_cloud_generation(
        &mut self,
        mcp_resources: Option<McpResourceClient>,
    ) -> Option<McpResourceClientAuthKey> {
        let auth_cache_key = mcp_resources
            .as_ref()
            .map(|client| client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME));
        if self
            .cloud_generation
            .as_ref()
            .is_none_or(|generation| generation.auth_cache_key != auth_cache_key)
        {
            self.cloud_generation = Some(CloudPluginGeneration {
                auth_cache_key: auth_cache_key.clone(),
                mcp_resources,
                catalog: None,
                resource_cache_key: None,
            });
        }
        auth_cache_key
    }

    /// Replaces the catalog with each accepted complete or partial result.
    /// Returns false when auth or resource identity changed during discovery.
    pub(crate) fn publish_cloud_catalog(
        &mut self,
        auth_cache_key: Option<&McpResourceClientAuthKey>,
        resource_cache_key: Option<McpResourceServerCacheKey>,
        catalog: PluginCatalog,
    ) -> bool {
        let Some(generation) = self
            .cloud_generation
            .as_mut()
            .filter(|generation| generation.auth_cache_key.as_ref() == auth_cache_key)
            .filter(|generation| generation.is_auth_current())
            .filter(|generation| generation.current_resource_cache_key() == resource_cache_key)
        else {
            return false;
        };
        record_catalog(self.extension_metrics.as_deref(), "cloud", &catalog);
        generation.catalog = Some(catalog);
        generation.resource_cache_key = resource_cache_key;
        true
    }
}

pub(crate) struct CachedSelectedRoot {
    pub(crate) root: SelectedCapabilityRoot,
    pub(crate) ownership: Option<PluginRootOwnership>,
    pub(crate) ownership_sandbox: Option<Box<FileSystemSandboxContext>>,
    pub(crate) metadata: CachedPluginMetadata,
}

/// Denied roots retain ownership without loading their capabilities.
pub(crate) enum CachedPluginMetadata {
    Unloaded,
    Loaded(Option<SelectedPluginContribution>),
}

/// Bounded declarations projected from merged plugin metadata.
#[derive(Clone, Default)]
pub(crate) struct SelectedPluginMetadata {
    pub(crate) plugin_display_name: String,
    pub(crate) servers: Vec<(String, codex_config::McpServerConfig)>,
    pub(crate) connector_ids: Vec<String>,
}

#[cfg(test)]
#[path = "plugin_contributor_state_tests.rs"]
mod tests;
