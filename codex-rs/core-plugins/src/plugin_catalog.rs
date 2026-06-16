use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::marketplace::MarketplaceError;
use crate::marketplace::MarketplaceListOutcome;
use crate::marketplace::list_marketplaces_with_home;
use crate::plugin_catalog_revision::PluginCatalogRevision;

const MAX_REUSED_PLUGIN_CATALOG_ENTRIES: usize = 1024;
const MAX_REUSED_PLUGIN_CATALOG_SOURCES: usize = 64;

/// A marketplace membership view assembled from ordered plugin catalog sources.
#[derive(Default)]
pub(crate) struct PluginCatalogSnapshot {
    outcome: MarketplaceListOutcome,
}

impl PluginCatalogSnapshot {
    pub(crate) fn marketplace_outcome(&self) -> MarketplaceListOutcome {
        self.outcome.clone()
    }
}

/// Declares whether a source can safely reuse a previously loaded fragment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PluginCatalogLoadMode {
    AlwaysRebuild,
    ReuseIfRevisionMatches(PluginCatalogRevision),
}

impl PluginCatalogLoadMode {
    pub(crate) fn for_revision(revision: Option<PluginCatalogRevision>) -> Self {
        revision.map_or(Self::AlwaysRebuild, Self::ReuseIfRevisionMatches)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum PluginCatalogSourceLocation {
    Home(AbsolutePathBuf),
    FilesystemRoot(AbsolutePathBuf),
}

/// Ordered input describing one independently refreshable plugin catalog source.
pub(crate) struct PluginCatalogSource {
    location: PluginCatalogSourceLocation,
    load_mode: PluginCatalogLoadMode,
}

impl PluginCatalogSource {
    pub(crate) fn home(home: AbsolutePathBuf) -> Self {
        Self {
            location: PluginCatalogSourceLocation::Home(home),
            load_mode: PluginCatalogLoadMode::AlwaysRebuild,
        }
    }

    pub(crate) fn filesystem_root(root: AbsolutePathBuf, load_mode: PluginCatalogLoadMode) -> Self {
        Self {
            location: PluginCatalogSourceLocation::FilesystemRoot(root),
            load_mode,
        }
    }
}

/// Builds snapshots while reusing only sources with a proven revision contract.
#[derive(Default)]
pub(crate) struct PluginCatalog {
    state: Mutex<PluginCatalogState>,
}

#[derive(Default)]
struct PluginCatalogState {
    reused_fragments: HashMap<PluginCatalogSourceLocation, CachedPluginCatalogFragment>,
}

struct CachedPluginCatalogFragment {
    revision: PluginCatalogRevision,
    fragment: Arc<PluginCatalogFragment>,
}

struct PluginCatalogFragment {
    outcome: MarketplaceListOutcome,
}

impl PluginCatalog {
    pub(crate) fn snapshot(
        &self,
        sources: &[PluginCatalogSource],
    ) -> Result<PluginCatalogSnapshot, MarketplaceError> {
        let mut outcome = MarketplaceListOutcome::default();
        let mut seen_marketplace_paths = HashSet::new();
        let mut seen_error_paths = HashSet::new();
        for source in sources {
            let fragment = self.fragment_for(source)?;
            for marketplace in &fragment.outcome.marketplaces {
                if seen_marketplace_paths.insert(marketplace.path.clone()) {
                    outcome.marketplaces.push(marketplace.clone());
                }
            }
            for error in &fragment.outcome.errors {
                if seen_error_paths.insert(error.path.clone()) {
                    outcome.errors.push(error.clone());
                }
            }
        }
        Ok(PluginCatalogSnapshot { outcome })
    }

    fn fragment_for(
        &self,
        source: &PluginCatalogSource,
    ) -> Result<Arc<PluginCatalogFragment>, MarketplaceError> {
        let PluginCatalogLoadMode::ReuseIfRevisionMatches(revision) = &source.load_mode else {
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .reused_fragments
                .remove(&source.location);
            return self.load_fragment(source).map(Arc::new);
        };

        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(cached) = state.reused_fragments.get(&source.location)
            && cached.revision == *revision
        {
            return Ok(Arc::clone(&cached.fragment));
        }

        let fragment = Arc::new(self.load_fragment(source)?);
        if !revision.is_current()
            || !fragment.outcome.errors.is_empty()
            || fragment.plugin_count() > MAX_REUSED_PLUGIN_CATALOG_ENTRIES
        {
            state.reused_fragments.remove(&source.location);
            return Ok(fragment);
        }

        let retained_entry_count = state
            .reused_fragments
            .iter()
            .filter(|(location, _cached)| *location != &source.location)
            .map(|(_location, cached)| cached.fragment.plugin_count())
            .sum::<usize>();
        if retained_entry_count.saturating_add(fragment.plugin_count())
            > MAX_REUSED_PLUGIN_CATALOG_ENTRIES
            || (state.reused_fragments.len() >= MAX_REUSED_PLUGIN_CATALOG_SOURCES
                && !state.reused_fragments.contains_key(&source.location))
        {
            state.reused_fragments.clear();
        }
        state.reused_fragments.insert(
            source.location.clone(),
            CachedPluginCatalogFragment {
                revision: revision.clone(),
                fragment: Arc::clone(&fragment),
            },
        );
        Ok(fragment)
    }

    fn load_fragment(
        &self,
        source: &PluginCatalogSource,
    ) -> Result<PluginCatalogFragment, MarketplaceError> {
        let outcome = match &source.location {
            PluginCatalogSourceLocation::Home(home) => {
                list_marketplaces_with_home(&[], Some(home.as_path()))?
            }
            PluginCatalogSourceLocation::FilesystemRoot(root) => {
                list_marketplaces_with_home(std::slice::from_ref(root), /*home_dir*/ None)?
            }
        };
        Ok(PluginCatalogFragment { outcome })
    }
}

impl PluginCatalogFragment {
    fn plugin_count(&self) -> usize {
        self.outcome
            .marketplaces
            .iter()
            .map(|marketplace| marketplace.plugins.len())
            .sum()
    }
}

#[cfg(test)]
#[path = "plugin_catalog_tests.rs"]
mod tests;
