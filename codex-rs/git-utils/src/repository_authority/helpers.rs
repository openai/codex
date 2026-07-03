use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::errors::GitReadError;
use crate::git_config::path_is_within;

use super::directories_refer_to_same_location;
use super::path_has_untrusted_root_identity_ancestor;

#[derive(Clone, Debug)]
pub(super) struct RegisteredWorktree {
    pub(super) root: PathBuf,
    pub(super) admin_dir: PathBuf,
    pub(super) marker_route: PathBuf,
}

#[derive(Debug)]
pub(super) struct RegisteredWorktreeReadError {
    pub(super) path: PathBuf,
    pub(super) source: io::Error,
}

pub(super) fn registry_error(path: &Path, source: io::Error) -> RegisteredWorktreeReadError {
    RegisteredWorktreeReadError {
        path: path.to_path_buf(),
        source,
    }
}

pub(super) fn common_dir_is_within_untrusted_root(common_dir: &Path, roots: &[PathBuf]) -> bool {
    for root in roots {
        if directories_refer_to_same_location(common_dir, root).unwrap_or(false) {
            continue;
        }
        if path_is_within(common_dir, root)
            || path_has_untrusted_root_identity_ancestor(common_dir, std::slice::from_ref(root))
        {
            return true;
        }
    }
    false
}

pub(super) fn canonical_existing_ancestors(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut canonical = Vec::new();
    for ancestor in path.ancestors() {
        match std::fs::canonicalize(ancestor) {
            Ok(path) => canonical.push(path),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(canonical)
}

pub(super) fn validated_logical_process_cwd(canonical_cwd: &Path) -> Option<PathBuf> {
    let process_cwd = std::fs::canonicalize(std::env::current_dir().ok()?).ok()?;
    if !paths_equal(&process_cwd, canonical_cwd) {
        return None;
    }
    let logical_cwd = PathBuf::from(std::env::var_os("PWD")?);
    if !logical_cwd.is_absolute() {
        return None;
    }
    let canonical_logical_cwd = std::fs::canonicalize(&logical_cwd).ok()?;
    paths_equal(&canonical_logical_cwd, canonical_cwd).then_some(logical_cwd)
}

pub(super) fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| paths_equal(existing, &path)) {
        paths.push(path);
    }
}

pub(super) fn paths_equal(left: &Path, right: &Path) -> bool {
    path_is_within(left, right) && path_is_within(right, left)
}

pub(super) fn invalid_metadata(path: &Path, error: io::Error) -> GitReadError {
    GitReadError::InvalidRepositoryMetadata {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}
