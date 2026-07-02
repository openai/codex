use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::errors::GitReadError;
use crate::git_config::path_is_within;
use crate::path_authority::RepositoryRouteBoundaries;
use crate::path_authority::repository_route_boundaries;

use super::CommonConfigAuthority;
use super::RegisteredWorktree;
use super::ResolvedRepositoryMetadata;
use super::authority_refusal;
use super::directories_refer_to_same_location;
use super::helpers::common_dir_is_within_untrusted_root;
use super::helpers::invalid_metadata;
use super::helpers::paths_equal;
use super::helpers::push_unique;
use super::helpers::validated_logical_process_cwd;
use super::inspect_plain_common_config_authority;
use super::linked_common_dir_for_root;
use super::primary_authority_is_proven;
use super::registered_worktrees;
use super::resolve_repository_metadata;

mod policy;

/// Repository and filesystem authority retained for the lifetime of one
/// trusted Git runner.
#[derive(Debug)]
pub(crate) struct RepositoryAuthority {
    active_worktree_root: PathBuf,
    roots: Vec<PathBuf>,
    worktree_roots: Vec<PathBuf>,
    common_dirs: Vec<PathBuf>,
    unproven_primary_common_dir: Option<PathBuf>,
    metadata_snapshots: Vec<ResolvedRepositoryMetadata>,
    registered_worktrees: Vec<RegisteredWorktree>,
    active_metadata: Option<ResolvedRepositoryMetadata>,
    route_boundaries: RepositoryRouteBoundaries,
}

impl RepositoryAuthority {
    pub(crate) fn discover(cwd: &Path) -> Result<Self, GitReadError> {
        let lexical_cwd = if cwd.is_absolute() {
            cwd.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|_| GitReadError::NotRepository {
                    path: cwd.to_path_buf(),
                })?
                .join(cwd)
        };
        let canonical_cwd =
            std::fs::canonicalize(cwd).map_err(|_| GitReadError::NotRepository {
                path: cwd.to_path_buf(),
            })?;
        let worktree_root = crate::get_git_repo_root(&canonical_cwd)
            .and_then(|root| std::fs::canonicalize(root).ok())
            .unwrap_or_else(|| canonical_cwd.clone());
        let mut authority = Self {
            active_worktree_root: worktree_root.clone(),
            roots: Vec::new(),
            worktree_roots: Vec::new(),
            common_dirs: Vec::new(),
            unproven_primary_common_dir: None,
            metadata_snapshots: Vec::new(),
            registered_worktrees: Vec::new(),
            active_metadata: None,
            route_boundaries: RepositoryRouteBoundaries::default(),
        };
        authority.record_repository_ancestry(&worktree_root)?;

        // Canonicalization can erase a repository-controlled symlink prefix.
        // Retain the requested spelling so lexical enclosing repositories stay
        // in the authority boundary.
        let lexical_base = if lexical_cwd.is_dir() {
            lexical_cwd
        } else {
            lexical_cwd
                .parent()
                .ok_or_else(|| GitReadError::NotRepository {
                    path: cwd.to_path_buf(),
                })?
                .to_path_buf()
        };
        authority.record_repository_ancestry(&lexical_base)?;
        if let Some(logical_cwd) = validated_logical_process_cwd(&canonical_cwd) {
            authority.record_repository_ancestry(&logical_cwd)?;
        }
        authority.record_preselection_primary_authorities()?;
        authority.active_metadata = authority
            .metadata_snapshots
            .iter()
            .find(|snapshot| snapshot.marker == worktree_root.join(".git"))
            .cloned();
        authority.route_boundaries =
            repository_route_boundaries(&authority.worktree_roots, &authority.common_dirs)
                .map_err(|error| invalid_metadata(&worktree_root, error))?;
        authority.validate_registered_worktree_routes()?;
        authority.validate_metadata_routes()?;
        Ok(authority)
    }

    pub(crate) fn ensure_primary_authority(&self) -> Result<(), GitReadError> {
        if let Some(common_dir) = &self.unproven_primary_common_dir {
            return Err(GitReadError::UnprovenPrimaryAuthority {
                common_dir: common_dir.display().to_string(),
            });
        }
        Ok(())
    }

    pub(crate) fn active_worktree_root(&self) -> Option<&Path> {
        self.active_metadata
            .as_ref()
            .map(|_| self.active_worktree_root.as_path())
    }

    pub(crate) fn path_is_untrusted_for_executable(&self, path: &Path) -> bool {
        self.path_is_untrusted_for_executable_result(path)
            .unwrap_or(true)
    }

    pub(crate) fn revalidate_active_repository_metadata(&self) -> io::Result<()> {
        let Some(expected) = &self.active_metadata else {
            return Ok(());
        };
        let actual = resolve_repository_metadata(&expected.marker).map_err(|error| {
            authority_refusal(format!(
                "repository metadata changed during Git operation at {}: {error}",
                expected.marker.display()
            ))
        })?;
        if &actual != expected {
            return Err(authority_refusal(format!(
                "repository metadata changed during Git operation at {}",
                expected.marker.display()
            )));
        }
        self.validate_snapshot_routes(&actual).map_err(|error| {
            authority_refusal(format!(
                "repository metadata changed during Git operation at {}: {error}",
                expected.marker.display()
            ))
        })
    }

    pub(crate) fn ensure_active_worktree_root(&self, root: &Path) -> io::Result<()> {
        let canonical_root = std::fs::canonicalize(root)?;
        let same_root = canonical_root == self.active_worktree_root
            || same_file::is_same_file(&canonical_root, &self.active_worktree_root)?;
        if !same_root {
            return Err(authority_refusal(format!(
                "guarded Git repository root {} does not match runner repository {}",
                canonical_root.display(),
                self.active_worktree_root.display()
            )));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn from_test_locations(
        roots: Vec<PathBuf>,
        worktree_roots: Vec<PathBuf>,
        common_dirs: Vec<PathBuf>,
    ) -> Result<Self, GitReadError> {
        let route_boundaries = repository_route_boundaries(&worktree_roots, &common_dirs)
            .map_err(|error| invalid_metadata(Path::new("<test>"), error))?;
        Ok(Self {
            active_worktree_root: worktree_roots
                .first()
                .or_else(|| roots.first())
                .cloned()
                .unwrap_or_default(),
            roots,
            worktree_roots,
            common_dirs,
            unproven_primary_common_dir: None,
            metadata_snapshots: Vec::new(),
            registered_worktrees: Vec::new(),
            active_metadata: None,
            route_boundaries,
        })
    }

    #[cfg(all(test, unix))]
    pub(crate) fn active_git_dir(&self) -> Option<&Path> {
        self.active_metadata
            .as_ref()
            .map(|metadata| metadata.git_dir.as_path())
    }

    #[cfg(test)]
    pub(crate) fn contains_root(&self, root: &Path) -> bool {
        self.roots
            .iter()
            .any(|candidate| paths_equal(candidate, root))
    }

    #[cfg(test)]
    pub(crate) fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    fn record_preselection_primary_authorities(&mut self) -> Result<(), GitReadError> {
        let mut inspected_common_dirs: Vec<PathBuf> = Vec::new();
        loop {
            let roots = self.roots.clone();
            let mut next = None;
            for root in roots {
                let Some(common_dir) = linked_common_dir_for_root(&root)
                    .map_err(|error| invalid_metadata(&root.join(".git"), error))?
                else {
                    continue;
                };
                let already_inspected = inspected_common_dirs.iter().try_fold(
                    false,
                    |found, inspected| -> Result<bool, GitReadError> {
                        Ok(found
                            || directories_refer_to_same_location(inspected, &common_dir)
                                .map_err(|error| invalid_metadata(&common_dir, error))?)
                    },
                )?;
                if !already_inspected {
                    next = Some((root, common_dir));
                    break;
                }
            }
            let Some((linked_root, common_dir)) = next else {
                return Ok(());
            };
            inspected_common_dirs.push(common_dir.clone());
            if primary_authority_is_proven(&linked_root, &common_dir, &self.roots)
                .map_err(|error| invalid_metadata(&common_dir, error))?
            {
                continue;
            }
            if common_dir_is_within_untrusted_root(&common_dir, &self.roots) {
                self.unproven_primary_common_dir.get_or_insert(common_dir);
                continue;
            }
            match inspect_plain_common_config_authority(&common_dir)
                .map_err(|error| invalid_metadata(&common_dir, error))?
            {
                CommonConfigAuthority::Bare => {}
                CommonConfigAuthority::Worktree(root) => {
                    self.record_repository_ancestry(&root)?;
                }
                CommonConfigAuthority::Unproven => {
                    self.unproven_primary_common_dir.get_or_insert(common_dir);
                }
            }
        }
    }

    fn record_repository_ancestry(&mut self, start: &Path) -> Result<(), GitReadError> {
        push_unique(&mut self.roots, start.to_path_buf());
        push_unique(&mut self.worktree_roots, start.to_path_buf());
        self.record_repository_marker(start)?;
        for ancestor in start.parent().into_iter().flat_map(Path::ancestors) {
            let marker = ancestor.join(".git");
            match std::fs::symlink_metadata(&marker) {
                Ok(_) => {
                    push_unique(&mut self.roots, ancestor.to_path_buf());
                    push_unique(&mut self.worktree_roots, ancestor.to_path_buf());
                    let canonical_root = std::fs::canonicalize(ancestor)
                        .map_err(|error| invalid_metadata(&marker, error))?;
                    push_unique(&mut self.roots, canonical_root.clone());
                    push_unique(&mut self.worktree_roots, canonical_root);
                    self.record_repository_marker(ancestor)?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(invalid_metadata(&marker, error)),
            }
        }
        Ok(())
    }

    fn record_repository_marker(&mut self, worktree_root: &Path) -> Result<(), GitReadError> {
        let marker = worktree_root.join(".git");
        let snapshot = match std::fs::symlink_metadata(&marker) {
            Ok(_) => resolve_repository_metadata(&marker)
                .map_err(|error| invalid_metadata(&marker, error))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(invalid_metadata(&marker, error)),
        };
        let common_dir = snapshot.common_dir.clone();
        if !self
            .metadata_snapshots
            .iter()
            .any(|known| known.marker == snapshot.marker)
        {
            self.metadata_snapshots.push(snapshot);
        }
        if !path_is_within(&common_dir, worktree_root) {
            push_unique(&mut self.roots, common_dir.clone());
        }
        push_unique(&mut self.common_dirs, common_dir.clone());
        self.record_registered_worktrees(&common_dir)?;
        self.record_common_dir_ancestry(common_dir)
    }

    fn record_registered_worktrees(&mut self, common_dir: &Path) -> Result<(), GitReadError> {
        let registrations = registered_worktrees(common_dir)
            .map_err(|error| invalid_metadata(&error.path, error.source))?;
        for registration in registrations {
            if !self
                .registered_worktrees
                .iter()
                .any(|known| known.admin_dir == registration.admin_dir)
            {
                self.registered_worktrees.push(registration.clone());
            }
            let already_known = self
                .roots
                .iter()
                .any(|known| paths_equal(known, &registration.root));
            push_unique(&mut self.roots, registration.root.clone());
            push_unique(&mut self.worktree_roots, registration.root.clone());
            if let Ok(canonical_root) = std::fs::canonicalize(&registration.root) {
                push_unique(&mut self.roots, canonical_root.clone());
                push_unique(&mut self.worktree_roots, canonical_root);
            }
            self.validate_registered_worktree_backlink(&registration)?;
            if !already_known {
                self.record_repository_ancestry(&registration.root)?;
            }
        }
        Ok(())
    }

    fn validate_registered_worktree_backlink(
        &self,
        registration: &RegisteredWorktree,
    ) -> Result<(), GitReadError> {
        match std::fs::metadata(&registration.root) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(GitReadError::InvalidRepositoryMetadata {
                    path: registration.root.clone(),
                    reason: "registered Git worktree is not a directory".to_string(),
                });
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                return Ok(());
            }
            Err(error) => return Err(invalid_metadata(&registration.root, error)),
        }
        let marker = registration.root.join(".git");
        match std::fs::symlink_metadata(&marker) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(GitReadError::InvalidRepositoryMetadata {
                    path: marker,
                    reason: "unsupported registered Git worktree marker".to_string(),
                });
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                return Ok(());
            }
            Err(error) => return Err(invalid_metadata(&marker, error)),
        }
        let resolved = resolve_repository_metadata(&marker)
            .map_err(|error| invalid_metadata(&marker, error))?;
        if !directories_refer_to_same_location(&resolved.git_dir, &registration.admin_dir)
            .map_err(|error| invalid_metadata(&marker, error))?
        {
            return Err(GitReadError::UnsafeRepositoryMetadata {
                path: marker,
                reason: "Git worktree registry backlink mismatch".to_string(),
            });
        }
        Ok(())
    }

    fn record_common_dir_ancestry(&mut self, common_dir: PathBuf) -> Result<(), GitReadError> {
        for ancestor in common_dir.parent().into_iter().flat_map(Path::ancestors) {
            let marker = ancestor.join(".git");
            match std::fs::symlink_metadata(&marker) {
                Ok(_) => {
                    let canonical_root = std::fs::canonicalize(ancestor)
                        .map_err(|error| invalid_metadata(&marker, error))?;
                    let already_known = self.roots.iter().any(|root| {
                        paths_equal(root, ancestor) || paths_equal(root, &canonical_root)
                    });
                    push_unique(&mut self.roots, ancestor.to_path_buf());
                    push_unique(&mut self.worktree_roots, ancestor.to_path_buf());
                    push_unique(&mut self.roots, canonical_root.clone());
                    push_unique(&mut self.worktree_roots, canonical_root);
                    if !already_known {
                        self.record_repository_marker(ancestor)?;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(invalid_metadata(&marker, error)),
            }
        }
        Ok(())
    }
}
