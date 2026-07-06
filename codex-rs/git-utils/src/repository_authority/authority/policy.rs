use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::errors::GitReadError;
use crate::git_config::path_is_within;

use super::super::RepositoryMetadataRouteKind;
use super::super::ResolvedRepositoryMetadata;
use super::super::directories_refer_to_same_location;
use super::super::helpers::canonical_existing_ancestors;
use super::super::helpers::invalid_metadata;
use super::super::helpers::paths_equal;
use super::super::repository_common_dir_for_candidate_root;
use super::RepositoryAuthority;

impl RepositoryAuthority {
    pub(crate) fn canonical_command_cwd(&self, cwd: &Path) -> io::Result<PathBuf> {
        let canonical = std::fs::canonicalize(cwd)?;
        let inspection = self.route_boundaries.inspect_route(cwd)?;
        if (inspection.touches_worktree || inspection.crosses_worktree)
            && !path_is_within(&canonical, &self.active_worktree_root)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "refusing Git command cwd that no longer resolves within the selected worktree: {}",
                    cwd.display()
                ),
            ));
        }
        Ok(canonical)
    }

    pub(crate) fn ensure_config_source_is_not_worktree_controlled(
        &self,
        path: &Path,
        description: &str,
    ) -> io::Result<()> {
        if !path.is_absolute() {
            return Err(worktree_controlled_config_source(path, description));
        }
        if self
            .route_boundaries
            .route_contains_process_relative_procfs_path(path)?
        {
            return Err(process_relative_config_source(path, description));
        }
        let inspection = self.route_boundaries.inspect_route(path)?;
        if inspection.crosses_worktree {
            return Err(worktree_controlled_config_source(path, description));
        }
        for observed in inspection.observed_paths {
            if self.route_boundaries.contains_known_boundary(&observed)? {
                continue;
            }
            if self.has_related_repository_ancestor(&observed)? {
                return Err(worktree_controlled_config_source(path, description));
            }
        }
        Ok(())
    }

    pub(super) fn path_is_untrusted_for_executable_result(&self, path: &Path) -> io::Result<bool> {
        let inspection = self.route_boundaries.inspect_route(path)?;
        if inspection.touches_worktree || inspection.crosses_metadata {
            return Ok(true);
        }
        for candidate in inspection.observed_paths {
            if self.route_boundaries.contains_known_boundary(&candidate)? {
                continue;
            }
            if self.has_related_repository_ancestor(&candidate)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn has_related_repository_ancestor(&self, path: &Path) -> io::Result<bool> {
        let mut ancestors = path.ancestors().map(Path::to_path_buf).collect::<Vec<_>>();
        ancestors.extend(canonical_existing_ancestors(path)?);
        let mut seen = BTreeSet::new();
        for ancestor in ancestors {
            if !seen.insert(ancestor.clone()) {
                continue;
            }
            let Some(common_dir) = repository_common_dir_for_candidate_root(&ancestor)? else {
                continue;
            };
            for protected in &self.common_dirs {
                if directories_refer_to_same_location(protected, &common_dir)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(super) fn validate_metadata_routes(&self) -> Result<(), GitReadError> {
        for snapshot in &self.metadata_snapshots {
            self.validate_snapshot_routes(snapshot)?;
        }
        Ok(())
    }

    pub(super) fn validate_registered_worktree_routes(&self) -> Result<(), GitReadError> {
        for registration in &self.registered_worktrees {
            if self
                .route_boundaries
                .retained_route_is_untrusted(&registration.marker_route)
                .map_err(|error| invalid_metadata(&registration.admin_dir.join("gitdir"), error))?
            {
                return Err(GitReadError::UnsafeRepositoryMetadata {
                    path: registration.admin_dir.join("gitdir"),
                    reason: "Git worktree registry route crosses a repository worktree".to_string(),
                });
            }
        }
        Ok(())
    }

    pub(super) fn validate_snapshot_routes(
        &self,
        snapshot: &ResolvedRepositoryMetadata,
    ) -> Result<(), GitReadError> {
        for route in &snapshot.routes {
            if route.kind == RepositoryMetadataRouteKind::StandardDirectory {
                let root = route.spelling.parent().ok_or_else(|| {
                    GitReadError::InvalidRepositoryMetadata {
                        path: route.spelling.clone(),
                        reason: "Git metadata directory has no worktree parent".to_string(),
                    }
                })?;
                let expected = std::fs::canonicalize(root)
                    .map_err(|error| invalid_metadata(root, error))?
                    .join(".git");
                if !paths_equal(&route.target, &expected) {
                    return Err(GitReadError::UnsafeRepositoryMetadata {
                        path: snapshot.marker.clone(),
                        reason: "nonstandard Git metadata directory".to_string(),
                    });
                }
                if self
                    .route_boundaries
                    .route_cancels_worktree_descendant(&route.spelling)
                    .map_err(|error| invalid_metadata(&route.spelling, error))?
                {
                    return Err(GitReadError::UnsafeRepositoryMetadata {
                        path: snapshot.marker.clone(),
                        reason: "Git metadata route crosses a repository worktree".to_string(),
                    });
                }
                continue;
            }
            let spelling_crosses = self
                .route_boundaries
                .inspect_route(&route.spelling)
                .map_err(|error| invalid_metadata(&route.spelling, error))?
                .crosses_worktree;
            let target_crosses = self
                .route_boundaries
                .inspect_route(&route.target)
                .map_err(|error| invalid_metadata(&route.target, error))?
                .crosses_worktree;
            if spelling_crosses || target_crosses {
                return Err(GitReadError::UnsafeRepositoryMetadata {
                    path: snapshot.marker.clone(),
                    reason: "Git metadata route crosses a repository worktree".to_string(),
                });
            }
        }
        Ok(())
    }
}

fn worktree_controlled_config_source(path: &Path, description: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!(
            "refusing to use worktree-controlled {description}: {}",
            path.display()
        ),
    )
}

fn process_relative_config_source(path: &Path, description: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!(
            "refusing to use process-relative {description}: {}",
            path.display()
        ),
    )
}
