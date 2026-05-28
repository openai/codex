use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_protocol::permissions::project_roots_glob_pattern;
use codex_protocol::protocol::WritableRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fmt;
use std::path::Path;

/// Context needed to evaluate an already-materialized filesystem policy.
pub struct FilesystemPermissionsContext<'a> {
    /// Resolves cwd-sensitive policy mechanics such as filesystem root and
    /// relative candidate paths. It is not workspace-root authority.
    pub policy_evaluation_cwd: &'a AbsolutePathBuf,
}

/// The outer filesystem access mode represented by effective permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemPermissionsMode {
    Restricted,
    Unrestricted,
    External,
}

/// A deny-read glob retained in effective filesystem enforcement inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDenyGlob {
    pattern: String,
}

impl ValidatedDenyGlob {
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

/// Effective filesystem enforcement facts derived from a permission profile.
///
/// This internal representation centralizes effective roots, writable carveouts,
/// protected metadata, and read-deny matching before platform-specific lowering.
pub struct EffectiveFilesystemPermissions {
    pub mode: FilesystemPermissionsMode,
    pub readable_roots: Vec<AbsolutePathBuf>,
    pub writable_roots: Vec<WritableRoot>,
    pub unreadable_roots: Vec<AbsolutePathBuf>,
    pub unreadable_globs: Vec<ValidatedDenyGlob>,
    pub include_platform_defaults: bool,
    pub glob_scan_max_depth: Option<usize>,
    file_system_policy: FileSystemSandboxPolicy,
    policy_evaluation_cwd: AbsolutePathBuf,
    read_deny_matcher: Option<ReadDenyMatcher>,
}

impl fmt::Debug for EffectiveFilesystemPermissions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EffectiveFilesystemPermissions")
            .field("mode", &self.mode)
            .field("readable_roots", &self.readable_roots)
            .field("writable_roots", &self.writable_roots)
            .field("unreadable_roots", &self.unreadable_roots)
            .field("unreadable_globs", &self.unreadable_globs)
            .field("include_platform_defaults", &self.include_platform_defaults)
            .field("glob_scan_max_depth", &self.glob_scan_max_depth)
            .finish_non_exhaustive()
    }
}

impl EffectiveFilesystemPermissions {
    /// Derives effective filesystem enforcement facts for platform consumers.
    ///
    /// Callers must pass an effective `PermissionProfile` after runtime grants and
    /// runtime workspace roots have been applied. Symbolic workspace-root entries
    /// are rejected at this boundary rather than resolved from a working directory.
    pub fn from_profile(
        permission_profile: &PermissionProfile,
        context: FilesystemPermissionsContext<'_>,
    ) -> Result<Self, FilesystemPermissionsError> {
        let file_system_policy = permission_profile.file_system_sandbox_policy();
        if contains_unmaterialized_workspace_roots(&file_system_policy) {
            return Err(FilesystemPermissionsError::UnmaterializedWorkspaceRoots);
        }
        // Direct enforcement queries have historically failed closed for malformed
        // deny patterns. Platform lowering that expands concrete targets can still
        // validate the patterns before acting on the filesystem.
        let read_deny_matcher =
            ReadDenyMatcher::new(&file_system_policy, context.policy_evaluation_cwd.as_path());
        let mode = match file_system_policy.kind {
            FileSystemSandboxKind::Restricted => FilesystemPermissionsMode::Restricted,
            FileSystemSandboxKind::Unrestricted => FilesystemPermissionsMode::Unrestricted,
            FileSystemSandboxKind::ExternalSandbox => FilesystemPermissionsMode::External,
        };
        let readable_roots =
            file_system_policy.get_readable_roots_with_cwd(context.policy_evaluation_cwd.as_path());
        let writable_roots =
            file_system_policy.get_writable_roots_with_cwd(context.policy_evaluation_cwd.as_path());
        let unreadable_roots = file_system_policy
            .get_unreadable_roots_with_cwd(context.policy_evaluation_cwd.as_path());
        let unreadable_globs = file_system_policy
            .get_unreadable_globs_with_cwd(context.policy_evaluation_cwd.as_path())
            .into_iter()
            .map(|pattern| ValidatedDenyGlob { pattern })
            .collect();
        let include_platform_defaults = file_system_policy.include_platform_defaults();
        let glob_scan_max_depth = file_system_policy.glob_scan_max_depth;

        Ok(Self {
            mode,
            readable_roots,
            writable_roots,
            unreadable_roots,
            unreadable_globs,
            include_platform_defaults,
            glob_scan_max_depth,
            file_system_policy,
            policy_evaluation_cwd: context.policy_evaluation_cwd.clone(),
            read_deny_matcher,
        })
    }

    /// Returns whether a read is permitted after applying explicit read denies.
    pub fn can_read(&self, path: &Path) -> bool {
        self.file_system_policy
            .can_read_path_with_cwd(path, self.policy_evaluation_cwd.as_path())
            && !self.is_read_denied(path)
    }

    /// Returns whether a write is permitted, including protected metadata rules.
    pub fn can_write(&self, path: &Path) -> bool {
        self.file_system_policy
            .can_write_path_with_cwd(path, self.policy_evaluation_cwd.as_path())
    }

    /// Returns whether `path` is matched by an explicit deny-read entry.
    pub fn is_read_denied(&self, path: &Path) -> bool {
        self.read_deny_matcher
            .as_ref()
            .is_some_and(|matcher| matcher.is_read_denied(path))
    }

    pub fn has_full_disk_read_access(&self) -> bool {
        self.file_system_policy.has_full_disk_read_access()
    }

    pub fn has_full_disk_write_access(&self) -> bool {
        self.file_system_policy.has_full_disk_write_access()
    }
}

/// An error deriving filesystem enforcement facts from a permission profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemPermissionsError {
    UnmaterializedWorkspaceRoots,
    InvalidDenyGlob(String),
}

impl fmt::Display for FilesystemPermissionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmaterializedWorkspaceRoots => formatter.write_str(
                "effective filesystem permissions require workspace roots to be materialized from runtime workspace roots",
            ),
            Self::InvalidDenyGlob(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for FilesystemPermissionsError {}

fn contains_unmaterialized_workspace_roots(file_system_policy: &FileSystemSandboxPolicy) -> bool {
    let workspace_glob_prefix = project_roots_glob_pattern(Path::new(""));
    file_system_policy
        .entries
        .iter()
        .any(|entry| match &entry.path {
            FileSystemPath::Special {
                value: FileSystemSpecialPath::ProjectRoots { .. },
            } => true,
            FileSystemPath::GlobPattern { pattern } => pattern.starts_with(&workspace_glob_prefix),
            FileSystemPath::Path { .. } | FileSystemPath::Special { .. } => false,
        })
}

#[cfg(test)]
#[path = "effective_filesystem_permissions_tests.rs"]
mod tests;
