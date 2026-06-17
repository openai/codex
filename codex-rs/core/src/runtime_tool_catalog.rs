//! Session-scoped reuse of revision-proven runtime tool fragments.
//!
//! This module sits between the source owners and `built_tools`. Source owners
//! remain responsible for refreshing data and issuing revisions that cover a
//! complete fragment. The runtime catalog only retains immutable fragments for
//! revisions that can be proven unchanged, then publishes them as one
//! consistent snapshot.
//!
//! Request-specific policy, unrevisioned sources, dynamic tools, and the final
//! `ToolRouter` stay on their existing per-request paths.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::RwLock;

use codex_app_server_protocol::AppInfo;
use codex_core_plugins::PluginCatalogRevision;
use codex_core_plugins::marketplace::MarketplaceListOutcome;
use codex_mcp::HostedConnectorRuntimeRevision;
use codex_mcp::ToolInfo;

/// Cache key for the complete host-owned Codex Apps runtime fragment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HostedConnectorsRevision(pub(crate) u64);

impl From<HostedConnectorRuntimeRevision> for HostedConnectorsRevision {
    fn from(revision: HostedConnectorRuntimeRevision) -> Self {
        Self(revision.generation())
    }
}

/// Cache key for a complete plugin catalog whose source supplied a revision.
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

/// Complete host-owned connector tools and compact metadata for one revision.
pub(crate) struct HostedConnectorRuntimeFragment {
    pub(crate) revision: HostedConnectorsRevision,
    pub(crate) tools: Vec<ToolInfo>,
    pub(crate) connectors: Vec<AppInfo>,
}

/// Complete marketplace data covered by one proven plugin source revision.
pub(crate) struct RevisionedPluginCatalogFragment {
    pub(crate) revision: RevisionedPluginsRevision,
    pub(crate) marketplaces: MarketplaceListOutcome,
}

/// Immutable aggregate of the reusable runtime tool source fragments.
///
/// Each slot contains a complete fragment from one source owner. A missing
/// slot means the source is absent or cannot prove a reusable revision; it does
/// not represent a partially built fragment. Unrevisioned inputs are composed
/// separately by `built_tools` and are never stored here.
#[derive(Default)]
pub(crate) struct RuntimeToolCatalogSnapshot {
    /// Host-owned Codex Apps tools and connector metadata.
    pub(crate) hosted_connectors: Option<Arc<HostedConnectorRuntimeFragment>>,
    /// Plugin marketplace metadata backed by a proven source revision.
    pub(crate) revisioned_plugins: Option<Arc<RevisionedPluginCatalogFragment>>,
}

impl RuntimeToolCatalogSnapshot {
    /// Returns the hosted fragment only when its complete-source revision matches.
    pub(crate) fn hosted_connectors_for(
        &self,
        revision: HostedConnectorsRevision,
    ) -> Option<Arc<HostedConnectorRuntimeFragment>> {
        self.hosted_connectors
            .as_ref()
            .filter(|fragment| fragment.revision == revision)
            .map(Arc::clone)
    }

    /// Returns the plugin fragment only when its proven source revision matches.
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

/// Session-scoped coordinator for reusable runtime tool catalog fragments.
///
/// The manager is deliberately not a source owner or a generic cache
/// framework. `McpConnectionManager` and `PluginsManager` continue to own
/// refreshes, materialization, and revision semantics. `built_tools` reads
/// those source inputs, reuses matching fragments from the current snapshot,
/// rebuilds changed fragments through their owners, and asks this manager to
/// publish the resulting complete aggregate.
///
/// Publication is compare-and-swap-like: a rebuild may replace only the
/// snapshot it started from. A concurrent publisher wins over a stale rebuild,
/// and a failed rebuild publishes nothing. The manager never caches the final
/// `ToolRouter` or request- and turn-scoped overlays.
#[derive(Default)]
pub(crate) struct RuntimeToolCatalogManager {
    snapshot: RwLock<Arc<RuntimeToolCatalogSnapshot>>,
}

impl RuntimeToolCatalogManager {
    /// Returns the current immutable aggregate without cloning its fragments.
    pub(crate) fn snapshot(&self) -> Arc<RuntimeToolCatalogSnapshot> {
        self.snapshot
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Publishes a successful complete rebuild if `base` is still current.
    ///
    /// If another rebuild has already published, its snapshot is returned and
    /// `next` is discarded. If `next` is an error, the current snapshot remains
    /// untouched and the error is returned to the caller.
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
