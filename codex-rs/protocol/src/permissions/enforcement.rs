//! Projects filesystem permissions for each sandbox, preserving mutable paths where the backend binds them.

use super::FileSystemSandboxPolicy;
use super::LocalPolicyContext;
use super::normalize_effective_absolute_path;
use crate::protocol::WritableRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::io;
use std::path::Path;

/// How a sandbox binds filesystem roots after access and precedence are resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSystemPolicyProjection {
    /// Resolve roots and carveouts to their effective native paths.
    Native,
    /// Normalize trusted system aliases, leaving mutable components for the sandbox to bind.
    PreserveMutableRoots,
    /// Inherit root metadata writes from the root mount and retain literal paths for read denials.
    LinuxMounts,
}

impl FileSystemPolicyProjection {
    pub(super) fn resolve_writable_path(self, path: AbsolutePathBuf) -> AbsolutePathBuf {
        match self {
            Self::Native | Self::LinuxMounts => normalize_effective_absolute_path(path),
            Self::PreserveMutableRoots => path.normalize_system_aliases().unwrap_or(path),
        }
    }
}

/// Filesystem permissions resolved on the host that will enforce them.
///
/// Roots retain policy order. Deny globs remain patterns for the sandbox to enforce or expand.
/// Resolving these paths does not pin their targets; the sandbox still binds them at launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFileSystemPermissions {
    pub has_full_disk_read_access: bool,
    pub has_full_disk_write_access: bool,
    pub include_platform_defaults: bool,
    pub readable_roots: Vec<AbsolutePathBuf>,
    pub writable_roots: Vec<WritableRoot>,
    pub unreadable_roots: Vec<AbsolutePathBuf>,
    pub unreadable_globs: Vec<String>,
    pub glob_scan_max_depth: Option<usize>,
}

impl FileSystemSandboxPolicy {
    /// Resolve the roots for one enforcement operation after binding executor-owned paths.
    pub fn resolve_for_enforcement(
        &self,
        cwd: &Path,
        projection: FileSystemPolicyProjection,
    ) -> io::Result<ResolvedFileSystemPermissions> {
        let mut resolved = ResolvedFileSystemPermissions {
            has_full_disk_read_access: self.has_full_disk_read_access(),
            has_full_disk_write_access: self.has_full_disk_write_access(),
            include_platform_defaults: self.include_platform_defaults(),
            readable_roots: Vec::new(),
            writable_roots: Vec::new(),
            unreadable_roots: Vec::new(),
            unreadable_globs: self.get_unreadable_globs_with_cwd(cwd),
            glob_scan_max_depth: self.glob_scan_max_depth,
        };
        if resolved.has_full_disk_read_access && resolved.has_full_disk_write_access {
            return Ok(resolved);
        }
        let entries = self.resolved_entries_with_cwd(cwd);
        if entries.is_empty() {
            return Ok(resolved);
        }

        let context = LocalPolicyContext::new(cwd);
        let matching = context
            .as_ref()
            .and_then(|context| self.prepare_local_matching(&context.as_context()).ok());
        if let Some(matching) = &matching {
            if !resolved.has_full_disk_read_access {
                resolved.readable_roots = Self::readable_roots(&entries, matching);
            }
            if !resolved.has_full_disk_write_access
                && entries.iter().any(|entry| entry.access.can_write())
            {
                resolved.writable_roots = Self::writable_roots(cwd, &entries, matching, projection);
            }
        }
        // If the matcher couldn't be prepared, withhold grants but keep explicit denials.
        resolved.unreadable_roots =
            Self::unreadable_roots(cwd, &entries, matching.as_ref(), projection);
        Ok(resolved)
    }
}
