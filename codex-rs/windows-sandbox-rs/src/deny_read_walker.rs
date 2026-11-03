//! Matcher-backed snapshot traversal for Windows deny-read ACL targets.
//!
//! The accessible-directory behavior is the same as the original native
//! resolver: disappearing and inaccessible branches cannot be enumerated, but
//! a matching directory is retained before reading its children. This does not
//! grant access to a skipped branch. Unexpected I/O errors abort the snapshot.
//! Canonical parents are reused only within this walk; links are resolved anew.

use super::push_absolute_path;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashSet;
use std::fs::FileType;
use std::io;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy)]
pub(super) enum DirectoryScanMode {
    DirectoriesOnly,
    IncludeAllFiles,
}

pub(super) fn collect_existing_glob_directory_matches(
    root: &Path,
    matcher: &ReadDenyMatcher,
    paths: &mut Vec<AbsolutePathBuf>,
    seen_paths: &mut HashSet<PathBuf>,
    max_depth: Option<usize>,
    mode: DirectoryScanMode,
) -> Result<(), String> {
    let Some(metadata) = accessible_entry(root.metadata(), root)? else {
        return Ok(());
    };
    let canonical = accessible_entry(root.canonicalize(), root)?;
    let mut walker = GlobWalker {
        matcher,
        paths,
        seen_paths,
        seen_directories: HashSet::new(),
        max_depth,
    };
    walker.visit(
        root,
        canonical.as_deref(),
        metadata.file_type(),
        /*depth*/ 0,
        mode,
    )
}

struct GlobWalker<'a> {
    matcher: &'a ReadDenyMatcher,
    paths: &'a mut Vec<AbsolutePathBuf>,
    seen_paths: &'a mut HashSet<PathBuf>,
    seen_directories: HashSet<PathBuf>,
    max_depth: Option<usize>,
}

impl GlobWalker<'_> {
    fn visit(
        &mut self,
        path: &Path,
        canonical: Option<&Path>,
        file_type: FileType,
        depth: usize,
        mode: DirectoryScanMode,
    ) -> Result<(), String> {
        let denied = match canonical {
            Some(canonical) => self
                .matcher
                .is_local_path_read_denied_with_canonical_path(path, canonical),
            None => self.matcher.is_local_path_read_denied(path),
        };
        if denied {
            push_absolute_path(self.paths, self.seen_paths, path)?;
        }
        if !file_type.is_dir()
            || self.max_depth.is_some_and(|limit| depth >= limit)
            || !self
                .seen_directories
                .insert(canonical.unwrap_or(path).to_path_buf())
        {
            return Ok(());
        }
        let Some(entries) = accessible_entry(std::fs::read_dir(path), path)? else {
            return Ok(());
        };
        for entry in entries {
            let Some(entry) = accessible_entry(entry, path)? else {
                continue;
            };
            let entry_path = entry.path();
            let Some(file_type) = accessible_entry(entry.file_type(), &entry_path)? else {
                continue;
            };
            if file_type.is_symlink() {
                // Name-surrogate reparse points (including Windows junctions)
                // cannot inherit the parent's canonical location. Resolve the
                // target, then retain lexical matches and detect target cycles.
                let target = accessible_entry(entry_path.canonicalize(), &entry_path)?;
                let Some(metadata) = accessible_entry(entry_path.metadata(), &entry_path)? else {
                    continue;
                };
                self.visit(
                    &entry_path,
                    target.as_deref(),
                    metadata.file_type(),
                    depth + 1,
                    DirectoryScanMode::IncludeAllFiles,
                )?;
            } else if file_type.is_dir() || matches!(mode, DirectoryScanMode::IncludeAllFiles) {
                // Directory entries already carry their type on Windows. A
                // normal child has the resolved parent's location plus its
                // enumerated name; do not stat/canonicalize every ordinary file.
                self.visit(
                    &entry_path,
                    canonical
                        .map(|parent| parent.join(entry.file_name()))
                        .as_deref(),
                    file_type,
                    depth + 1,
                    mode,
                )?;
            }
        }
        Ok(())
    }
}

fn accessible_entry<T>(result: io::Result<T>, path: &Path) -> Result<Option<T>, String> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            Ok(None)
        }
        Err(err) => Err(format!(
            "failed to enumerate unreadable glob paths under {}: {err}",
            path.display()
        )),
    }
}

#[cfg(test)]
#[path = "deny_read_walker_tests.rs"]
mod tests;
