//! Tracks the source of model metadata without changing the catalog wire or cache formats.

use std::borrow::Cow;
use std::collections::HashSet;

use codex_protocol::openai_models::ModelInfo;

use crate::cache::ModelsCacheEntry;

/// Where an effective catalog entry was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelOrigin {
    Bundled,
    Provider,
    ExplicitConfig,
}

/// Model metadata and its source captured from the same catalog snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogModel {
    pub model: ModelInfo,
    pub origin: ModelOrigin,
}

/// Catalog authority is independent of whether its model metadata is borrowed or owned.
pub(crate) enum SelectedCatalog<'a> {
    CurrentSnapshot(Cow<'a, [ModelInfo]>),
    Fallback(Cow<'a, [ModelInfo]>),
}

impl<'a> SelectedCatalog<'a> {
    pub fn into_models(self) -> Cow<'a, [ModelInfo]> {
        match self {
            Self::CurrentSnapshot(models) | Self::Fallback(models) => models,
        }
    }
}

/// The effective catalog and the IDs in the unmerged provider response share one lock.
#[derive(Debug)]
pub(crate) struct CatalogState {
    pub entry: ModelsCacheEntry,
    pub provider_model_ids: HashSet<String>,
}

impl CatalogState {
    pub fn bundled(entry: ModelsCacheEntry) -> Self {
        Self {
            entry,
            provider_model_ids: HashSet::new(),
        }
    }

    pub fn provider(entry: ModelsCacheEntry) -> Self {
        // Capture raw response IDs before merging, including when loading from disk cache.
        let provider_model_ids = entry
            .models
            .iter()
            .map(|model| model.slug.clone())
            .collect();
        Self {
            entry,
            provider_model_ids,
        }
    }

    pub fn model_catalog(&self, selected: SelectedCatalog<'_>) -> Vec<CatalogModel> {
        let uses_snapshot_ids = matches!(selected, SelectedCatalog::CurrentSnapshot(_));
        selected
            .into_models()
            .into_owned()
            .into_iter()
            .map(|model| {
                let origin = if uses_snapshot_ids && self.provider_model_ids.contains(&model.slug) {
                    ModelOrigin::Provider
                } else {
                    ModelOrigin::Bundled
                };
                CatalogModel { model, origin }
            })
            .collect()
    }
}
