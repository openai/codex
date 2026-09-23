//! Retains installed-cache and standalone-skill candidates and assembles their locations.

use super::CapabilityLocation;
use super::CapabilityLocations;
use super::MAX_LOCATIONS;
use super::discover_cached_plugins;
use crate::LocalFileSystem;
use codex_file_system::ExecutorFileSystem;
use codex_file_watcher::WatchPath;
use codex_utils_path_uri::PathUri;
use std::io;

const MAX_LOCATION_WARNINGS: usize = 64;

/// Existing candidates plus watch paths for existing and not-yet-created roots.
pub(super) struct CandidateCapabilityLocations {
    candidates: Vec<CandidateCapabilityLocation>,
    watches: Vec<WatchPath>,
}

/// An installed cache or standalone skill root whose locations can be resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
enum CandidateCapabilityLocation {
    Cache(PathUri),
    Standalone(PathUri),
}

impl CandidateCapabilityLocations {
    /// Resolves existing cache and skill roots into one bounded location inventory.
    pub(super) async fn resolve(
        fs: &LocalFileSystem,
        cache_root: Option<PathUri>,
        skill_roots: Vec<PathUri>,
    ) -> io::Result<CapabilityLocations> {
        let mut candidates = cache_root
            .into_iter()
            .map(CandidateCapabilityLocation::Cache)
            .collect::<Vec<_>>();
        let mut standalone = skill_roots;
        let mut seen = std::collections::HashSet::new();
        standalone.retain(|root| seen.insert(root.clone()));
        candidates.extend(
            standalone
                .into_iter()
                .map(CandidateCapabilityLocation::Standalone),
        );

        let mut watches = Vec::new();
        let mut retained_candidates = Vec::new();
        for candidate in candidates {
            let root = match &candidate {
                CandidateCapabilityLocation::Cache(root)
                | CandidateCapabilityLocation::Standalone(root) => root,
            };
            let exists = match fs
                .get_metadata(root, Default::default(), /*sandbox*/ None)
                .await
            {
                Ok(metadata) => metadata.is_directory,
                Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                Err(error) => return Err(error),
            };
            // Missing roots use the watcher's ancestor fallback until they are created.
            watches.push(WatchPath {
                path: root.to_abs_path()?.into_path_buf(),
                recursive: true,
            });
            if exists {
                retained_candidates.push(candidate);
            }
        }
        let candidates = Self {
            candidates: retained_candidates,
            watches,
        };
        let mut resolved = Vec::new();
        for candidate in &candidates.candidates {
            resolved.push(candidates.resolve_candidates(fs, candidate).await?);
        }
        Ok(candidates.assemble(resolved))
    }

    async fn resolve_candidates(
        &self,
        fs: &LocalFileSystem,
        candidate: &CandidateCapabilityLocation,
    ) -> io::Result<CapabilityLocations> {
        match candidate {
            CandidateCapabilityLocation::Cache(root) => discover_cached_plugins(fs, root).await,
            // Standalone roots need no manifest or version selection.
            CandidateCapabilityLocation::Standalone(root) => Ok(CapabilityLocations {
                locations: vec![CapabilityLocation {
                    root: root.clone(),
                    plugin: None,
                }],
                ..Default::default()
            }),
        }
    }

    /// Combines and bounds inventory; Core applies plugin enablement.
    fn assemble(&self, resolved_candidates: Vec<CapabilityLocations>) -> CapabilityLocations {
        let mut result = CapabilityLocations {
            watches: self.watches.clone(),
            ..Default::default()
        };
        // Combine installed plugin and standalone skill locations in discovery order.
        for candidate in resolved_candidates {
            result.locations.extend(candidate.locations);
            result.warnings.extend(candidate.warnings);
        }

        if result.locations.len() > MAX_LOCATIONS {
            result.warnings.truncate(MAX_LOCATION_WARNINGS - 1);
            result.warnings.push(format!(
                "capability discovery reached its {MAX_LOCATIONS}-location limit"
            ));
            result.locations.truncate(MAX_LOCATIONS);
        }
        result.warnings.truncate(MAX_LOCATION_WARNINGS);
        result
    }
}
