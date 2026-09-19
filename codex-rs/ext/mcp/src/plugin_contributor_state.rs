//! Thread-owned cloud catalog and legacy selected-root cache, guarded by a short-lived mutex.

use codex_core_plugins::PluginCatalog;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpResourceClient;
use codex_mcp::McpResourceClientAuthKey;
use codex_protocol::capabilities::SelectedCapabilityRoot;
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
    pub(crate) plugins_enabled: bool,
    pub(crate) cloud_generation: Option<CloudPluginGeneration>,
    pub(crate) executor_cache: Vec<CachedSelectedRoot>,
}

/// Keeps cloud plugin metadata within one published auth scope and Apps availability state.
pub(crate) struct CloudPluginGeneration {
    pub(crate) auth_cache_key: Option<McpResourceClientAuthKey>,
    pub(crate) mcp_resources: Option<McpResourceClient>,
    pub(crate) catalog: Option<PluginCatalog>,
}

impl CloudPluginGeneration {
    pub(crate) fn new(mcp_resources: Option<McpResourceClient>) -> Self {
        let auth_cache_key = mcp_resources
            .as_ref()
            .map(|client| client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME));
        Self {
            auth_cache_key,
            mcp_resources,
            catalog: None,
        }
    }

    pub(crate) fn is_current(&self) -> bool {
        self.auth_cache_key
            == self
                .mcp_resources
                .as_ref()
                .map(|client| client.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME))
    }
}

impl PluginContributorState {
    /// Returns the cloud catalog only while it belongs to the live auth generation.
    fn cloud_catalog(&self) -> Option<&PluginCatalog> {
        self.cloud_generation
            .as_ref()
            .filter(|generation| generation.is_current())
            .and_then(|generation| generation.catalog.as_ref())
    }

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
            self.cloud_generation = Some(CloudPluginGeneration::new(mcp_resources));
        }
        auth_cache_key
    }

    pub(crate) fn publish_cloud_catalog(
        &mut self,
        auth_cache_key: Option<&McpResourceClientAuthKey>,
        catalog: PluginCatalog,
    ) -> bool {
        let Some(generation) = self
            .cloud_generation
            .as_mut()
            .filter(|generation| generation.auth_cache_key.as_ref() == auth_cache_key)
            .filter(|generation| generation.is_current())
        else {
            return false;
        };
        generation.catalog = Some(catalog);
        true
    }
}

pub(crate) struct CachedSelectedRoot {
    pub(crate) root: SelectedCapabilityRoot,
    pub(crate) metadata: Option<SelectedPluginMetadata>,
}

/// Frozen declarations retain the logical environment ID across executor reconnections.
#[derive(Clone)]
pub(crate) struct SelectedPluginMetadata {
    pub(crate) plugin_id: String,
    pub(crate) plugin_display_name: String,
    pub(crate) servers: Vec<(String, codex_config::McpServerConfig)>,
    pub(crate) connector_ids: Vec<String>,
}
