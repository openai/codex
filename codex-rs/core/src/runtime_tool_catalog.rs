use std::path::PathBuf;
use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::RwLock;

use codex_app_server_protocol::AppInfo;
use codex_core_plugins::PluginCatalogRevision;
use codex_core_plugins::marketplace::MarketplaceListOutcome;
use codex_mcp::HostedConnectorRuntimeRevision;
use codex_mcp::ToolInfo;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HostedConnectorsRevision(pub(crate) u64);

impl From<HostedConnectorRuntimeRevision> for HostedConnectorsRevision {
    fn from(revision: HostedConnectorRuntimeRevision) -> Self {
        Self(revision.generation())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RevisionedPluginsRevision(PathBuf, String);

impl From<&PluginCatalogRevision> for RevisionedPluginsRevision {
    fn from(revision: &PluginCatalogRevision) -> Self {
        Self(
            revision.source_root().to_path_buf(),
            revision.marker_contents().to_string(),
        )
    }
}

pub(crate) struct HostedConnectorRuntimeFragment {
    pub(crate) revision: HostedConnectorsRevision,
    pub(crate) tools: Vec<ToolInfo>,
    pub(crate) connectors: Vec<AppInfo>,
}

pub(crate) struct RevisionedPluginCatalogFragment {
    pub(crate) revision: RevisionedPluginsRevision,
    pub(crate) marketplaces: MarketplaceListOutcome,
}

#[derive(Default)]
pub(crate) struct RuntimeToolCatalogSnapshot {
    pub(crate) hosted_connectors: Option<Arc<HostedConnectorRuntimeFragment>>,
    pub(crate) revisioned_plugins: Option<Arc<RevisionedPluginCatalogFragment>>,
}

impl RuntimeToolCatalogSnapshot {
    pub(crate) fn hosted_connectors_for(
        &self,
        revision: HostedConnectorsRevision,
    ) -> Option<Arc<HostedConnectorRuntimeFragment>> {
        self.hosted_connectors
            .as_ref()
            .filter(|fragment| fragment.revision == revision)
            .map(Arc::clone)
    }

    pub(crate) fn revisioned_plugins_for(
        &self,
        revision: &RevisionedPluginsRevision,
    ) -> Option<Arc<RevisionedPluginCatalogFragment>> {
        self.revisioned_plugins
            .as_ref()
            .filter(|fragment| fragment.revision == *revision)
            .map(Arc::clone)
    }
}

#[derive(Default)]
pub(crate) struct RuntimeToolCatalogManager {
    snapshot: RwLock<Arc<RuntimeToolCatalogSnapshot>>,
}

impl RuntimeToolCatalogManager {
    pub(crate) fn snapshot(&self) -> Arc<RuntimeToolCatalogSnapshot> {
        self.snapshot
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn publish_if_current<E>(
        &self,
        base: &Arc<RuntimeToolCatalogSnapshot>,
        next: Result<RuntimeToolCatalogSnapshot, E>,
    ) -> Result<Arc<RuntimeToolCatalogSnapshot>, E> {
        let next = Arc::new(next?);
        let mut current = self
            .snapshot
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        if Arc::ptr_eq(&current, base) {
            *current = next;
        }
        Ok(Arc::clone(&current))
    }
}

#[cfg(test)]
#[path = "runtime_tool_catalog_tests.rs"]
mod tests;
