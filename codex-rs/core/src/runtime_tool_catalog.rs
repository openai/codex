//! Session-scoped reuse and atomic composition of stable runtime tool fragments.
//!
//! `RuntimeToolCatalogManager` is a small consistency boundary between source
//! owners and `built_tools`. It has exactly two top-level reusable slots:
//! host-owned Codex Apps runtime data and owner-revisioned plugin catalog data.
//! Source owners remain responsible for loading, refreshing, and issuing a
//! revision that covers their complete fragment. This module compares those
//! revisions, retains unchanged immutable fragments, and publishes the chosen
//! fragments together as one session snapshot.
//!
//! This is not the connector or plugin directory, and it is not a general cache
//! registry. It performs no network fetches, marketplace discovery, plugin
//! materialization, or policy evaluation. Request-specific policy,
//! unrevisioned sources, dynamic tools, and the final `ToolRouter` stay on their
//! existing per-request paths.

use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::RwLock;

use codex_app_server_protocol::AppInfo;
use codex_core_plugins::PluginCatalogRevision;
use codex_core_plugins::marketplace::MarketplaceListOutcome;
use codex_core_plugins::remote::RemoteDiscoverablePlugin;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RevisionedPluginsRevision(pub(crate) u64);

impl From<PluginCatalogRevision> for RevisionedPluginsRevision {
    fn from(revision: PluginCatalogRevision) -> Self {
        Self(revision.generation())
    }
}

/// Complete host-owned connector tools and compact metadata for one revision.
pub(crate) struct HostedConnectorRuntimeFragment {
    pub(crate) revision: HostedConnectorsRevision,
    pub(crate) tools: Vec<ToolInfo>,
    pub(crate) connectors: Vec<AppInfo>,
}

/// Complete local curated marketplace data covered by one owner revision.
pub(crate) struct CuratedPluginCatalogFragment {
    pub(crate) revision: RevisionedPluginsRevision,
    pub(crate) marketplaces: Arc<MarketplaceListOutcome>,
}

/// Complete remote-curated discovery data covered by one owner revision.
pub(crate) struct RemoteCuratedPluginCatalogFragment {
    pub(crate) revision: RevisionedPluginsRevision,
    pub(crate) plugins: Arc<Vec<RemoteDiscoverablePlugin>>,
}

/// Reusable plugin fragments, separated by their independent source owners.
///
/// This is one top-level runtime-catalog slot, but its two fields deliberately
/// retain distinct revisions. A curated refresh therefore cannot evict or
/// rebuild an unchanged remote-curated fragment, and vice versa.
pub(crate) struct RevisionedPluginCatalogFragment {
    pub(crate) curated: Option<Arc<CuratedPluginCatalogFragment>>,
    pub(crate) remote_curated: Option<Arc<RemoteCuratedPluginCatalogFragment>>,
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
    /// Local curated and remote-curated plugin fragments with independent revisions.
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

    /// Returns local curated data only when its owner generation still matches.
    pub(crate) fn curated_plugins_for(
        &self,
        revision: RevisionedPluginsRevision,
    ) -> Option<Arc<CuratedPluginCatalogFragment>> {
        self.revisioned_plugins
            .as_ref()
            .and_then(|fragment| fragment.curated.as_ref())
            .filter(|fragment| fragment.revision == revision)
            .map(Arc::clone)
    }

    /// Returns remote-curated data only when its owner generation still matches.
    pub(crate) fn remote_curated_plugins_for(
        &self,
        revision: RevisionedPluginsRevision,
    ) -> Option<Arc<RemoteCuratedPluginCatalogFragment>> {
        self.revisioned_plugins
            .as_ref()
            .and_then(|fragment| fragment.remote_curated.as_ref())
            .filter(|fragment| fragment.revision == revision)
            .map(Arc::clone)
    }
}

/// Session-scoped coordinator for reusable runtime tool catalog fragments.
///
/// The manager answers one narrow question: which complete, immutable source
/// fragments can this session safely reuse on the next `built_tools` call?
/// `McpConnectionManager` and `PluginsManager` continue to own refresh and
/// revision semantics. `built_tools` reads their current snapshots, reuses
/// matching fragments from this manager, composes live overlays, and still
/// constructs a new final router.
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
