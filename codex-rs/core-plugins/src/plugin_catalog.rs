use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_plugin::PluginCapabilitySummary;
use codex_plugin::PluginId;
use codex_plugin::prompt_safe_plugin_description;
use codex_protocol::protocol::Product;
use codex_utils_absolute_path::AbsolutePathBuf;
use thiserror::Error;
use tokio::sync::OnceCell;

use crate::loader::PluginCapabilityFacts;
use crate::loader::PluginCapabilitySkillMode;
use crate::loader::load_plugin_capability_facts;
use crate::manager::remote_plugin_install_required_description;
use crate::marketplace::MarketplaceError;
use crate::marketplace::MarketplaceListOutcome;
use crate::marketplace::MarketplacePluginSource;
use crate::marketplace::list_marketplaces_with_home;
use crate::plugin_catalog_revision::PluginCatalogRevision;

const MAX_REUSED_PLUGIN_CATALOG_ENTRIES: usize = 1024;

/// Source-derived plugin facts that are safe to share across runtime projections.
struct PluginCatalogEntry {
    plugin_id: PluginId,
    source: MarketplacePluginSource,
    restriction_product: Option<Product>,
    revision: Option<Arc<PluginCatalogRevision>>,
    capability_facts: OnceCell<PluginCapabilityFacts>,
}

impl PluginCatalogEntry {
    async fn capability_facts(
        &self,
    ) -> Result<PluginCapabilityFacts, PluginCatalogCapabilityError> {
        self.capability_facts
            .get_or_try_init(|| self.load_capability_facts())
            .await
            .cloned()
    }

    async fn load_capability_facts(
        &self,
    ) -> Result<PluginCapabilityFacts, PluginCatalogCapabilityError> {
        self.ensure_revision_is_current()?;
        let MarketplacePluginSource::Local { path: plugin_root } = &self.source else {
            return Ok(PluginCapabilityFacts {
                summary: PluginCapabilitySummary {
                    config_name: self.plugin_id.as_key(),
                    display_name: self.plugin_id.plugin_name.clone(),
                    description: prompt_safe_plugin_description(Some(
                        &remote_plugin_install_required_description(&self.source),
                    )),
                    ..PluginCapabilitySummary::default()
                },
                app_declarations: Vec::new(),
                had_errors: false,
            });
        };
        if !plugin_root.as_path().is_dir() {
            return Err(PluginCatalogCapabilityError::InvalidPlugin(
                "path does not exist or is not a directory",
            ));
        }
        let facts = load_plugin_capability_facts(
            &self.plugin_id,
            plugin_root,
            PluginCapabilitySkillMode::ValidForProduct(self.restriction_product),
        )
        .await
        .ok_or(PluginCatalogCapabilityError::InvalidPlugin(
            "missing or invalid plugin.json",
        ))?;
        self.ensure_revision_is_current()?;
        if facts.had_errors {
            return Err(PluginCatalogCapabilityError::InvalidPlugin(
                "failed to load one or more plugin capability files",
            ));
        }
        Ok(facts)
    }

    fn ensure_revision_is_current(&self) -> Result<(), PluginCatalogCapabilityError> {
        if self
            .revision
            .as_deref()
            .is_some_and(|revision| !revision.is_current())
        {
            return Err(PluginCatalogCapabilityError::SourceChanged);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub(crate) enum PluginCatalogCapabilityError {
    #[error("plugin catalog source changed while capability metadata was loading")]
    SourceChanged,
    #[error("{0}")]
    InvalidPlugin(&'static str),
}

/// Ordered catalog view with revision-safe lazy capabilities.
#[derive(Default)]
pub(crate) struct PluginCatalogSnapshot {
    outcome: MarketplaceListOutcome,
    entries_by_plugin_id: HashMap<String, Arc<PluginCatalogEntry>>,
}

impl PluginCatalogSnapshot {
    pub(crate) fn marketplace_outcome(&self) -> MarketplaceListOutcome {
        self.outcome.clone()
    }

    pub(crate) async fn capability_facts(
        &self,
        plugin_id: &str,
    ) -> Result<Option<PluginCapabilityFacts>, PluginCatalogCapabilityError> {
        let Some(entry) = self.entries_by_plugin_id.get(plugin_id) else {
            return Ok(None);
        };
        entry.capability_facts().await.map(Some)
    }
}

/// Declares whether a source can safely reuse a previously loaded fragment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PluginCatalogLoadMode {
    /// The source has no revision contract and must be reconstructed for each snapshot.
    AlwaysRebuild,
    /// Reuses the source when the revision covers its manifest and all reachable local artifacts.
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

/// Builds plugin catalog snapshots and reuses only sources with a proven revision contract.
pub(crate) struct PluginCatalog {
    restriction_product: Option<Product>,
    reused_fragments: Mutex<HashMap<PluginCatalogSourceLocation, CachedPluginCatalogFragment>>,
}

struct CachedPluginCatalogFragment {
    revision: PluginCatalogRevision,
    fragment: Arc<PluginCatalogFragment>,
}

struct PluginCatalogFragment {
    outcome: MarketplaceListOutcome,
    entries_by_plugin_id: HashMap<String, Arc<PluginCatalogEntry>>,
}

impl PluginCatalog {
    pub(crate) fn new(restriction_product: Option<Product>) -> Self {
        Self {
            restriction_product,
            reused_fragments: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn snapshot(
        &self,
        sources: &[PluginCatalogSource],
    ) -> Result<PluginCatalogSnapshot, MarketplaceError> {
        let mut outcome = MarketplaceListOutcome::default();
        let mut entries_by_plugin_id = HashMap::new();
        let mut seen_marketplace_paths = HashSet::new();
        let mut seen_error_paths = HashSet::new();
        for source in sources {
            let fragment = self.fragment_for(source)?;
            for marketplace in &fragment.outcome.marketplaces {
                if seen_marketplace_paths.insert(marketplace.path.clone()) {
                    outcome.marketplaces.push(marketplace.clone());
                    for plugin in &marketplace.plugins {
                        let plugin_id = format!("{}@{}", plugin.name, marketplace.name);
                        if let Some(entry) = fragment.entries_by_plugin_id.get(&plugin_id) {
                            entries_by_plugin_id
                                .entry(plugin_id)
                                .or_insert_with(|| Arc::clone(entry));
                        }
                    }
                }
            }
            for error in &fragment.outcome.errors {
                if seen_error_paths.insert(error.path.clone()) {
                    outcome.errors.push(error.clone());
                }
            }
        }
        Ok(PluginCatalogSnapshot {
            outcome,
            entries_by_plugin_id,
        })
    }

    fn fragment_for(
        &self,
        source: &PluginCatalogSource,
    ) -> Result<Arc<PluginCatalogFragment>, MarketplaceError> {
        let PluginCatalogLoadMode::ReuseIfRevisionMatches(revision) = &source.load_mode else {
            self.reused_fragments
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&source.location);
            return self.load_fragment(source).map(Arc::new);
        };

        // Loading while holding the mutex makes a revision miss single-flight. Fragment loads are
        // synchronous and only revisioned sources enter this critical section.
        let mut reused_fragments = self
            .reused_fragments
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(cached) = reused_fragments.get(&source.location)
            && cached.revision == *revision
        {
            return Ok(Arc::clone(&cached.fragment));
        }

        let fragment = Arc::new(self.load_fragment(source)?);
        let plugin_count = fragment.entries_by_plugin_id.len();
        if !revision.is_current()
            || !fragment.outcome.errors.is_empty()
            || plugin_count == 0
            || plugin_count > MAX_REUSED_PLUGIN_CATALOG_ENTRIES
        {
            reused_fragments.remove(&source.location);
            return Ok(fragment);
        }

        let retained_entry_count = reused_fragments
            .iter()
            .filter(|(location, _cached)| *location != &source.location)
            .map(|(_location, cached)| cached.fragment.entries_by_plugin_id.len())
            .sum::<usize>();
        if retained_entry_count.saturating_add(plugin_count) > MAX_REUSED_PLUGIN_CATALOG_ENTRIES {
            reused_fragments.clear();
        }
        reused_fragments.insert(
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
        let revision =
            if let PluginCatalogLoadMode::ReuseIfRevisionMatches(revision) = &source.load_mode {
                Some(Arc::new(revision.clone()))
            } else {
                None
            };
        let outcome = match &source.location {
            PluginCatalogSourceLocation::Home(home) => {
                list_marketplaces_with_home(&[], Some(home.as_path()))?
            }
            PluginCatalogSourceLocation::FilesystemRoot(root) => {
                list_marketplaces_with_home(std::slice::from_ref(root), /*home_dir*/ None)?
            }
        };
        let mut entries_by_plugin_id = HashMap::new();
        for marketplace in &outcome.marketplaces {
            for plugin in &marketplace.plugins {
                let Ok(plugin_id) = PluginId::new(plugin.name.clone(), marketplace.name.clone())
                else {
                    continue;
                };
                entries_by_plugin_id
                    .entry(plugin_id.as_key())
                    .or_insert_with(|| {
                        Arc::new(PluginCatalogEntry {
                            plugin_id,
                            source: plugin.source.clone(),
                            restriction_product: self.restriction_product,
                            revision: revision.clone(),
                            capability_facts: OnceCell::new(),
                        })
                    });
            }
        }
        Ok(PluginCatalogFragment {
            outcome,
            entries_by_plugin_id,
        })
    }
}

#[cfg(test)]
#[path = "plugin_catalog_tests.rs"]
mod tests;
