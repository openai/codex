use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::marketplace::MarketplaceListOutcome;
use crate::remote::RemoteDiscoverablePlugin;

static NEXT_PLUGIN_CATALOG_REVISION: AtomicU64 = AtomicU64::new(1);

/// Opaque process-local generation for one complete plugin catalog source.
///
/// `PluginsManager`, as the source owner, issues a new generation only after it
/// has loaded a complete curated or remote-curated snapshot. The value is not
/// a content hash and is intentionally not persisted: callers only compare it
/// for equality while reusing immutable source fragments in this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PluginCatalogRevision(u64);

impl PluginCatalogRevision {
    pub(crate) fn next() -> Self {
        Self(NEXT_PLUGIN_CATALOG_REVISION.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the opaque generation for use by runtime fragment caches.
    pub fn generation(self) -> u64 {
        self.0
    }
}

/// Immutable local OpenAI-curated marketplace snapshot published by its owner.
///
/// Mutable home and configured marketplace roots are deliberately excluded;
/// they continue to be read on every projection that combines them with this
/// stable source fragment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CuratedPluginCatalogSnapshot {
    revision: PluginCatalogRevision,
    marketplaces: Arc<MarketplaceListOutcome>,
}

impl CuratedPluginCatalogSnapshot {
    pub(crate) fn new(marketplaces: MarketplaceListOutcome) -> Self {
        Self {
            revision: PluginCatalogRevision::next(),
            marketplaces: Arc::new(marketplaces),
        }
    }

    /// Returns the generation covering the entire snapshot.
    pub fn revision(&self) -> PluginCatalogRevision {
        self.revision
    }

    /// Returns the complete marketplace fragment covered by `revision`.
    pub fn marketplaces(&self) -> &MarketplaceListOutcome {
        &self.marketplaces
    }

    /// Clones the shared marketplace fragment without copying its contents.
    pub fn shared_marketplaces(&self) -> Arc<MarketplaceListOutcome> {
        Arc::clone(&self.marketplaces)
    }
}

/// Immutable OpenAI-curated-remote discovery snapshot published by its owner.
///
/// Installed state, enablement, auth policy, and request-specific eligibility
/// remain live overlays and are not captured by this reusable source fragment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteCuratedPluginCatalogSnapshot {
    revision: PluginCatalogRevision,
    plugins: Arc<Vec<RemoteDiscoverablePlugin>>,
}

impl RemoteCuratedPluginCatalogSnapshot {
    pub(crate) fn new(plugins: Vec<RemoteDiscoverablePlugin>) -> Self {
        Self {
            revision: PluginCatalogRevision::next(),
            plugins: Arc::new(plugins),
        }
    }

    /// Returns the generation covering the entire snapshot.
    pub fn revision(&self) -> PluginCatalogRevision {
        self.revision
    }

    /// Returns the complete remote-curated discovery fragment for `revision`.
    pub fn plugins(&self) -> &[RemoteDiscoverablePlugin] {
        self.plugins.as_slice()
    }

    /// Clones the shared remote fragment without copying its contents.
    pub fn shared_plugins(&self) -> Arc<Vec<RemoteDiscoverablePlugin>> {
        Arc::clone(&self.plugins)
    }
}

/// The two plugin source snapshots eligible for runtime-fragment reuse.
///
/// Most configurations select exactly one source. Keeping both fields explicit
/// makes source addition, removal, and independent revision changes visible to
/// the runtime catalog without introducing a source registry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RevisionedPluginCatalogSnapshots {
    pub curated: Option<CuratedPluginCatalogSnapshot>,
    pub remote_curated: Option<RemoteCuratedPluginCatalogSnapshot>,
}
