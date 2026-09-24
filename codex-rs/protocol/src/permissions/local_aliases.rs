//! Prepare local policy paths in the sandbox's trusted system-alias namespace.
//! Normalize grants and restrictions once per operation without resolving mutable symlinks.
//! Borrow unchanged policies and return normalization failures to fallible callers.

use super::FileSystemAccessMode;
use super::FileSystemPath;
use super::FileSystemSandboxEntry;
use super::FileSystemSandboxPolicy;
use super::FileSystemSandboxPolicyContext;
use super::FileSystemSpecialPath;
use super::local_temporary_directories;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use std::borrow::Cow;
use std::io;
use std::path::Path;

/// Owns native environment bindings so one matcher can serve a whole root calculation.
pub(super) struct LocalPolicyContext {
    cwd: PathUri,
    user_home_dir: Option<PathUri>,
    temporary_directories: Vec<PathUri>,
}

impl LocalPolicyContext {
    pub(super) fn new(cwd: &Path) -> Option<Self> {
        Some(Self {
            cwd: PathUri::from(AbsolutePathBuf::from_absolute_path(cwd).ok()?),
            user_home_dir: PathUri::from_host_native_path("~").ok(),
            temporary_directories: local_temporary_directories(),
        })
    }

    pub(super) fn as_context(&self) -> FileSystemSandboxPolicyContext<'_> {
        FileSystemSandboxPolicyContext {
            cwd: &self.cwd,
            workspace_roots: std::slice::from_ref(&self.cwd),
            user_home_dir: self.user_home_dir.as_ref(),
            temporary_directories: Some(&self.temporary_directories),
        }
    }
}

/// An operation-scoped view of a configured policy with local roots resolved.
/// Reuse it for each target in the operation; prepare again when policy or
/// environment bindings change. Only target paths are normalized during matching.
pub struct LocalFileSystemPolicyMatcher<'a> {
    policy: Cow<'a, FileSystemSandboxPolicy>,
    context: FileSystemSandboxPolicyContext<'a>,
}

impl FileSystemSandboxPolicy {
    /// Prepare local permission matching, resolving trusted aliases on macOS.
    /// Borrows the configured policy on platforms that do not need normalization.
    /// Remote paths must use `can_write_path` instead: the controller's
    /// filesystem cannot establish aliases on another host.
    pub fn prepare_local_matching<'a>(
        &'a self,
        context: &FileSystemSandboxPolicyContext<'a>,
    ) -> io::Result<LocalFileSystemPolicyMatcher<'a>> {
        let mut policy = Cow::Borrowed(self);
        if cfg!(target_os = "macos") {
            let policy = policy.to_mut();
            // Preserve non-path entries, including globs that constrain full-disk
            // grants. Materialize symbolic roots using the selected environment.
            policy.entries.retain(|entry| {
                matches!(
                    entry.path,
                    FileSystemPath::GlobPattern { .. }
                        | FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root
                                | FileSystemSpecialPath::Minimal
                                | FileSystemSpecialPath::Unknown { .. }
                        }
                )
            });
            for (root, access) in self.resolved_entries(context) {
                if root.is_opaque() {
                    continue;
                }
                let normalized =
                    root.to_abs_path()?
                        .normalize_system_aliases()
                        .map_err(|error| {
                            io::Error::new(
                                error.kind(),
                                format!("failed to normalize {root}: {error}"),
                            )
                        })?;
                policy
                    .entries
                    .push(FileSystemSandboxEntry::new(normalized.into(), access));
            }
        }
        Ok(LocalFileSystemPolicyMatcher {
            policy,
            context: *context,
        })
    }
}

impl LocalFileSystemPolicyMatcher<'_> {
    /// Check write access, preserving any failure to normalize the target path.
    pub fn can_write_path(&self, path: &PathUri) -> io::Result<bool> {
        self.with_path(path, |path| self.policy.can_write_path(path, &self.context))
    }

    pub(super) fn resolve_access(&self, path: &PathUri) -> FileSystemAccessMode {
        self.with_path(path, |path| self.policy.resolve_access(path, &self.context))
            .unwrap_or(FileSystemAccessMode::Deny)
    }

    fn with_path<T>(&self, path: &PathUri, evaluate: impl FnOnce(&PathUri) -> T) -> io::Result<T> {
        let path = if cfg!(target_os = "macos") {
            Cow::Owned(PathUri::from(
                path.to_abs_path()?
                    .normalize_system_aliases()
                    .map_err(|error| {
                        io::Error::new(error.kind(), format!("failed to normalize {path}: {error}"))
                    })?,
            ))
        } else {
            Cow::Borrowed(path)
        };
        Ok(evaluate(&path))
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "local_aliases_tests.rs"]
mod tests;
