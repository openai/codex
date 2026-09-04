use std::borrow::Cow;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::canonicalize_preserving_symlinks;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use globset::Candidate;
use globset::GlobBuilder;
use globset::GlobMatcher;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use tracing::error;
use ts_rs::TS;

use crate::protocol::NetworkAccess;
use crate::protocol::SandboxPolicy;
use crate::protocol::WritableRoot;

const PROTECTED_METADATA_GIT_PATH_NAME: &str = ".git";
const PROTECTED_METADATA_AGENTS_PATH_NAME: &str = ".agents";
const PROTECTED_METADATA_CODEX_PATH_NAME: &str = ".codex";

/// Top-level workspace metadata paths that stay protected under writable roots.
pub const PROTECTED_METADATA_PATH_NAMES: &[&str] = &[
    PROTECTED_METADATA_GIT_PATH_NAME,
    PROTECTED_METADATA_AGENTS_PATH_NAME,
    PROTECTED_METADATA_CODEX_PATH_NAME,
];

/// Returns true when a path basename is one of the protected workspace metadata names.
pub fn is_protected_metadata_name(name: &OsStr) -> bool {
    PROTECTED_METADATA_PATH_NAMES
        .iter()
        .any(|metadata_name| name == OsStr::new(metadata_name))
}

/// Returns the protected workspace metadata name when an agent write to `path`
/// should be blocked before execution.
pub fn forbidden_agent_metadata_write(
    path: &Path,
    cwd: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
) -> Option<&'static str> {
    if !matches!(
        file_system_sandbox_policy.kind,
        FileSystemSandboxKind::Restricted
    ) {
        return None;
    }

    with_local_policy_context(path, cwd, |path, context| {
        file_system_sandbox_policy
            .metadata_write_denial(path, context)
            .filter(|_| !file_system_sandbox_policy.can_write_path(path, context))
    })?
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, Default, JsonSchema, TS,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum NetworkSandboxPolicy {
    #[default]
    Restricted,
    Enabled,
}

impl NetworkSandboxPolicy {
    pub fn is_enabled(self) -> bool {
        matches!(self, NetworkSandboxPolicy::Enabled)
    }
}

/// Access mode for a filesystem entry.
///
/// When two equally specific entries target the same path, we compare these by
/// conflict precedence rather than by capability breadth: `deny` beats
/// `write`, and `write` beats `read`.
#[derive(
    Debug,
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    Display,
    JsonSchema,
    TS,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum FileSystemAccessMode {
    Read,
    Write,
    /// `none` is a legacy input alias retained temporarily for compatibility.
    #[serde(alias = "none")]
    Deny,
}

impl FileSystemAccessMode {
    pub fn can_read(self) -> bool {
        !matches!(self, FileSystemAccessMode::Deny)
    }

    pub fn can_write(self) -> bool {
        matches!(self, FileSystemAccessMode::Write)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind")]
pub enum FileSystemSpecialPath {
    Root,
    Minimal,
    #[serde(alias = "current_working_directory")]
    ProjectRoots {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        subpath: Option<String>,
    },
    Tmpdir,
    SlashTmp,
    /// WARNING: `:special_path` tokens are part of config compatibility.
    /// Do not make older runtimes reject newly introduced tokens.
    /// New parser support should be additive, while unknown values must stay
    /// representable so config from a newer Codex degrades to warn-and-ignore
    /// instead of failing to load. Codex 0.112.0 rejected unknown values here,
    /// which broke forward compatibility for newer config.
    /// Preserves future special-path tokens so older runtimes can ignore them
    /// without rejecting config authored by a newer release.
    Unknown {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        subpath: Option<String>,
    },
}

impl FileSystemSpecialPath {
    pub fn project_roots(subpath: Option<String>) -> Self {
        Self::ProjectRoots { subpath }
    }

    pub fn unknown(path: impl Into<String>, subpath: Option<String>) -> Self {
        Self::Unknown {
            path: path.into(),
            subpath,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileSystemSandboxEntry {
    pub path: FileSystemPath,
    pub access: FileSystemAccessMode,
    pub missing_path_behavior: Option<FileSystemSandboxEntryMissingPathBehavior>,
}

/// Serialized filesystem entry used at legacy string-based seams.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[schemars(rename = "FileSystemSandboxEntry")]
#[ts(rename = "FileSystemSandboxEntry")]
pub struct RawFileSystemSandboxEntry {
    pub path: RawFileSystemPath,
    pub access: FileSystemAccessMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub missing_path_behavior: Option<FileSystemSandboxEntryMissingPathBehavior>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum FileSystemSandboxEntryMissingPathBehavior {
    Skip,
}

impl FileSystemSandboxEntry {
    pub fn new(path: FileSystemPath, access: FileSystemAccessMode) -> Self {
        Self {
            path,
            access,
            missing_path_behavior: None,
        }
    }

    pub fn skip_missing_path(path: FileSystemPath, access: FileSystemAccessMode) -> Self {
        Self {
            path,
            access,
            missing_path_behavior: Some(FileSystemSandboxEntryMissingPathBehavior::Skip),
        }
    }

    pub fn skips_missing_path(&self) -> bool {
        self.missing_path_behavior == Some(FileSystemSandboxEntryMissingPathBehavior::Skip)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, Default, JsonSchema, TS,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum FileSystemSandboxKind {
    #[default]
    Restricted,
    Unrestricted,
    ExternalSandbox,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSystemSandboxPolicy {
    pub kind: FileSystemSandboxKind,
    pub glob_scan_max_depth: Option<usize>,
    pub entries: Vec<FileSystemSandboxEntry>,
}

#[derive(Clone, Copy)]
enum WritableRootPathResolution {
    Effective,
    PreserveMutableComponents,
}

impl WritableRootPathResolution {
    fn resolve(self, path: AbsolutePathBuf) -> AbsolutePathBuf {
        match self {
            Self::Effective => normalize_effective_absolute_path(path),
            Self::PreserveMutableComponents => normalize_trusted_top_level_alias(path),
        }
    }
}

/// Serialized filesystem policy used at legacy string-based seams.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[schemars(rename = "FileSystemSandboxPolicy")]
#[ts(rename = "FileSystemSandboxPolicy")]
pub struct RawFileSystemSandboxPolicy {
    pub kind: FileSystemSandboxKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub glob_scan_max_depth: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<RawFileSystemSandboxEntry>,
}

/// Executor-owned paths needed to interpret filesystem sandbox policy entries.
///
/// Orchestrator callers keep these as `PathUri` values until execution crosses
/// into the executor that owns them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileSystemSandboxPolicyContext<'a> {
    pub cwd: &'a PathUri,
    pub workspace_roots: &'a [PathUri],
    pub user_home_dir: Option<&'a PathUri>,
    pub temporary_directories: Option<&'a [PathUri]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedFileSystemEntry {
    path: AbsolutePathBuf,
    access: FileSystemAccessMode,
}

struct PreparedFileSystemEntry<'a> {
    entry: &'a ResolvedFileSystemEntry,
    effective_path: AbsolutePathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSystemSemanticSignature {
    has_full_disk_read_access: bool,
    has_full_disk_write_access: bool,
    include_platform_defaults: bool,
    readable_roots: Vec<AbsolutePathBuf>,
    writable_roots: Vec<WritableRoot>,
    unreadable_roots: Vec<AbsolutePathBuf>,
    unreadable_globs: Vec<String>,
}

/// Runtime matcher for read-deny entries in a filesystem sandbox policy.
pub struct ReadDenyMatcher {
    native_cwd: Option<AbsolutePathBuf>,
    user_home_dir: Option<PathUri>,
    temporary_directories: Vec<PathUri>,
    prepared: PreparedReadDenyMatcher,
}

/// Prepared PathUri deny roots and globs for repeated executor-owned read checks.
struct PreparedReadDenyMatcher {
    denied_roots: Vec<PathUri>,
    deny_read_matchers: Vec<GlobMatcher>,
    invalid_pattern: bool,
}

impl ReadDenyMatcher {
    /// Builds a matcher for executor-owned URI paths without host projection.
    pub fn from_context(
        file_system_sandbox_policy: &FileSystemSandboxPolicy,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> Option<Self> {
        file_system_sandbox_policy
            .entries
            .iter()
            .any(|entry| entry.access == FileSystemAccessMode::Deny)
            .then(|| {
                file_system_sandbox_policy
                    .prepare_deny_read_matcher(context, InvalidDenyReadGlobBehavior::FailClosed)
                    .map(|prepared| Self {
                        native_cwd: None,
                        user_home_dir: None,
                        temporary_directories: Vec::new(),
                        prepared,
                    })
                    .ok()
            })
            .flatten()
    }

    /// Builds a local-path matcher for callers that must reject malformed glob patterns.
    ///
    /// Runtime read checks intentionally fail closed on malformed deny patterns.
    /// Host-side expansion work should use this constructor instead so a typo
    /// cannot broaden the set of paths it mutates before execution starts.
    pub fn try_new_for_local_paths(
        file_system_sandbox_policy: &FileSystemSandboxPolicy,
        cwd: &Path,
    ) -> Result<Option<Self>, String> {
        Self::build(
            file_system_sandbox_policy,
            cwd,
            InvalidDenyReadGlobBehavior::ReturnError,
        )
    }

    fn build(
        file_system_sandbox_policy: &FileSystemSandboxPolicy,
        cwd: &Path,
        invalid_glob_behavior: InvalidDenyReadGlobBehavior,
    ) -> Result<Option<Self>, String> {
        if !file_system_sandbox_policy.has_denied_read_restrictions() {
            return Ok(None);
        }
        let cwd = AbsolutePathBuf::from_absolute_path(cwd)
            .map_err(|err| format!("invalid read-deny cwd: {err}"))?;
        let cwd_uri = PathUri::from_abs_path(&cwd);
        let user_home_dir = PathUri::from_host_native_path("~").ok();
        let temporary_directories = local_temporary_directories();
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd_uri,
            workspace_roots: std::slice::from_ref(&cwd_uri),
            user_home_dir: user_home_dir.as_ref(),
            temporary_directories: Some(&temporary_directories),
        };
        let prepared = file_system_sandbox_policy
            .prepare_deny_read_matcher(&context, invalid_glob_behavior)?;
        Ok(Some(Self {
            native_cwd: Some(cwd),
            user_home_dir,
            temporary_directories,
            prepared,
        }))
    }

    /// Returns whether a local native `path` is denied by this matcher.
    pub fn is_local_path_read_denied(&self, path: &Path) -> bool {
        let Some(cwd) = self.native_cwd.as_ref() else {
            return true;
        };
        let Some(path) = resolve_candidate_path(path, cwd.as_path()) else {
            return true;
        };
        let path = PathUri::from(path);
        let cwd = PathUri::from_abs_path(cwd);
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd,
            workspace_roots: std::slice::from_ref(&cwd),
            user_home_dir: self.user_home_dir.as_ref(),
            temporary_directories: Some(&self.temporary_directories),
        };
        self.is_read_denied_uri(&path, &context)
    }

    /// Returns whether an executor-owned URI is denied under its matching path context.
    pub fn is_read_denied_uri(
        &self,
        path: &PathUri,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> bool {
        FileSystemSandboxPolicy::matches_prepared_read_deny(path, context, &self.prepared)
    }

    /// Checks an enumerated path using a canonical location already resolved by
    /// the caller, without reopening the file to canonicalize it.
    ///
    /// Bulk filesystem walkers may derive this location from a freshly resolved
    /// parent and a non-symlink directory entry. Symlinks and Windows junctions
    /// must be resolved separately. Do not reuse these locations across walks:
    /// a later operation must observe newly created files and changed links.
    pub fn is_local_path_read_denied_with_canonical_path(
        &self,
        path: &Path,
        canonical_path: &Path,
    ) -> bool {
        self.is_local_path_read_denied(path) || self.is_local_path_read_denied(canonical_path)
    }
}

#[derive(Clone, Copy)]
enum InvalidDenyReadGlobBehavior {
    FailClosed,
    ReturnError,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FileSystemPath {
    Path {
        path: PathUri,
    },
    /// A git-style glob pattern. Pattern entries currently support
    /// FileSystemAccessMode::Deny only.
    GlobPattern {
        pattern: String,
    },
    Special {
        value: FileSystemSpecialPath,
    },
}

/// Serialized filesystem path whose literal path variant preserves the raw
/// legacy string until an explicit seam conversion selects its meaning.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type")]
#[schemars(rename = "FileSystemPath")]
#[ts(rename = "FileSystemPath")]
pub enum RawFileSystemPath {
    Path {
        #[schemars(with = "AbsolutePathBuf")]
        #[ts(type = "string")]
        path: LegacyAppPathString,
    },
    GlobPattern {
        pattern: String,
    },
    Special {
        value: FileSystemSpecialPath,
    },
}

impl From<AbsolutePathBuf> for FileSystemPath {
    fn from(path: AbsolutePathBuf) -> Self {
        Self::Path { path: path.into() }
    }
}

impl From<PathUri> for FileSystemPath {
    fn from(path: PathUri) -> Self {
        Self::Path { path }
    }
}

fn path_uri_from_raw(path: LegacyAppPathString) -> Result<PathUri, String> {
    let native_path =
        serde::de::value::StrDeserializer::<serde::de::value::Error>::new(path.as_str());
    if let Ok(path) = AbsolutePathBuf::deserialize(native_path) {
        return Ok(PathUri::from(path));
    }

    PathUri::try_from(path).map_err(|err| err.to_string())
}

fn raw_path_from_uri(path: PathUri) -> Result<LegacyAppPathString, String> {
    let raw_path = LegacyAppPathString::from(path.clone());
    if path_uri_from_raw(raw_path.clone()).as_ref() == Ok(&path) {
        Ok(raw_path)
    } else {
        Err("permission path cannot be represented losslessly".to_string())
    }
}

impl TryFrom<RawFileSystemPath> for FileSystemPath {
    type Error = String;

    fn try_from(path: RawFileSystemPath) -> Result<Self, Self::Error> {
        Ok(match path {
            RawFileSystemPath::Path { path } => Self::Path {
                path: path_uri_from_raw(path)?,
            },
            RawFileSystemPath::GlobPattern { pattern } => Self::GlobPattern { pattern },
            RawFileSystemPath::Special { value } => Self::Special { value },
        })
    }
}

impl TryFrom<FileSystemPath> for RawFileSystemPath {
    type Error = String;

    fn try_from(path: FileSystemPath) -> Result<Self, Self::Error> {
        Ok(match path {
            FileSystemPath::Path { path } => Self::Path {
                path: raw_path_from_uri(path)?,
            },
            FileSystemPath::GlobPattern { pattern } => Self::GlobPattern { pattern },
            FileSystemPath::Special { value } => Self::Special { value },
        })
    }
}

impl TryFrom<RawFileSystemSandboxEntry> for FileSystemSandboxEntry {
    type Error = String;

    fn try_from(entry: RawFileSystemSandboxEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            path: entry.path.try_into()?,
            access: entry.access,
            missing_path_behavior: entry.missing_path_behavior,
        })
    }
}

impl TryFrom<FileSystemSandboxEntry> for RawFileSystemSandboxEntry {
    type Error = String;

    fn try_from(entry: FileSystemSandboxEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            path: entry.path.try_into()?,
            access: entry.access,
            missing_path_behavior: entry.missing_path_behavior,
        })
    }
}

impl TryFrom<RawFileSystemSandboxPolicy> for FileSystemSandboxPolicy {
    type Error = String;

    fn try_from(policy: RawFileSystemSandboxPolicy) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: policy.kind,
            glob_scan_max_depth: policy.glob_scan_max_depth,
            entries: policy
                .entries
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl TryFrom<FileSystemSandboxPolicy> for RawFileSystemSandboxPolicy {
    type Error = String;

    fn try_from(policy: FileSystemSandboxPolicy) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: policy.kind,
            glob_scan_max_depth: policy.glob_scan_max_depth,
            entries: policy
                .entries
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

const PROJECT_ROOTS_GLOB_PATTERN_PREFIX: &str = "codex-project-roots://";

pub fn project_roots_glob_pattern(subpath: &Path) -> String {
    format!("{PROJECT_ROOTS_GLOB_PATTERN_PREFIX}{}", subpath.display())
}

fn read_only_file_system_entries() -> Vec<FileSystemSandboxEntry> {
    vec![FileSystemSandboxEntry::new(
        FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        FileSystemAccessMode::Read,
    )]
}

impl Default for FileSystemSandboxPolicy {
    fn default() -> Self {
        Self::read_only()
    }
}

impl FileSystemSandboxPolicy {
    pub fn read_only() -> Self {
        Self::restricted(read_only_file_system_entries())
    }

    pub fn unrestricted() -> Self {
        Self {
            kind: FileSystemSandboxKind::Unrestricted,
            glob_scan_max_depth: None,
            entries: Vec::new(),
        }
    }

    pub fn external_sandbox() -> Self {
        Self {
            kind: FileSystemSandboxKind::ExternalSandbox,
            glob_scan_max_depth: None,
            entries: Vec::new(),
        }
    }

    pub fn restricted(entries: Vec<FileSystemSandboxEntry>) -> Self {
        Self {
            kind: FileSystemSandboxKind::Restricted,
            glob_scan_max_depth: None,
            entries,
        }
    }

    /// Removes entries that should be skipped when their paths are missing.
    ///
    /// Callers that materialize filesystem ACL targets should not turn these
    /// entries into newly-created sentinel paths.
    pub fn remove_skip_missing_path_entries(&mut self) {
        self.entries.retain(|entry| !entry.skips_missing_path());
    }

    /// Native-path compatibility adapter for local executor boundaries.
    /// Selected-executor orchestrators must keep paths as [`PathUri`] and use
    /// context-aware policy APIs instead.
    pub fn has_explicit_non_write_entry_for_local_path_with_cwd(
        &self,
        path: &Path,
        cwd: &Path,
    ) -> bool {
        let Some(path) = resolve_candidate_path(path, cwd) else {
            return false;
        };
        let cwd = AbsolutePathBuf::from_absolute_path(cwd).ok();
        self.entries.iter().any(|entry| {
            !entry.skips_missing_path()
                && !entry.access.can_write()
                && resolve_entry_path(&entry.path, cwd.as_ref()).as_ref() == Some(&path)
        })
    }

    fn has_root_access(&self, predicate: impl Fn(FileSystemAccessMode) -> bool) -> bool {
        matches!(self.kind, FileSystemSandboxKind::Restricted)
            && self.entries.iter().any(|entry| {
                matches!(
                    &entry.path,
                    FileSystemPath::Special { value }
                        if matches!(value, FileSystemSpecialPath::Root) && predicate(entry.access)
                )
            })
    }

    pub fn has_denied_read_restrictions(&self) -> bool {
        matches!(self.kind, FileSystemSandboxKind::Restricted)
            && self.entries.iter().any(|entry| {
                entry.access == FileSystemAccessMode::Deny
                    && !matches!(
                        &entry.path,
                        FileSystemPath::Special {
                            value: FileSystemSpecialPath::SlashTmp,
                        } if !cfg!(unix)
                    )
            })
    }

    pub fn from_legacy_sandbox_policy_preserving_deny_entries(
        sandbox_policy: &SandboxPolicy,
        cwd: &Path,
        existing: &Self,
    ) -> Self {
        let mut rebuilt = Self::from_legacy_sandbox_policy_for_cwd(sandbox_policy, cwd);
        if !matches!(rebuilt.kind, FileSystemSandboxKind::Restricted) {
            return rebuilt;
        }
        rebuilt.glob_scan_max_depth = existing.glob_scan_max_depth;

        for deny_entry in existing
            .entries
            .iter()
            .filter(|entry| entry.access == FileSystemAccessMode::Deny)
        {
            if !rebuilt.entries.iter().any(|entry| entry == deny_entry) {
                rebuilt.entries.push(deny_entry.clone());
            }
        }

        rebuilt
    }

    /// Preserve explicit read-deny rules from `existing` when a caller
    /// replaces the allow side of a policy.
    pub fn preserve_deny_read_restrictions_from(&mut self, existing: &Self) {
        let has_deny_read_entries = existing
            .entries
            .iter()
            .any(|entry| entry.access == FileSystemAccessMode::Deny);
        if matches!(self.kind, FileSystemSandboxKind::Unrestricted) && has_deny_read_entries {
            *self = Self::restricted(vec![FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Write,
            )]);
        }

        if !matches!(self.kind, FileSystemSandboxKind::Restricted) {
            return;
        }

        if self.glob_scan_max_depth.is_none() {
            self.glob_scan_max_depth = existing.glob_scan_max_depth;
        }

        for deny_entry in existing
            .entries
            .iter()
            .filter(|entry| entry.access == FileSystemAccessMode::Deny)
        {
            if !self.entries.iter().any(|entry| entry == deny_entry) {
                self.entries.push(deny_entry.clone());
            }
        }
    }

    /// Returns true when a restricted policy contains any entry that really
    /// reduces a broader `:root = write` grant.
    ///
    /// Raw entry presence is not enough here: an equally specific `write`
    /// entry for the same target wins under the normal precedence rules, so a
    /// shadowed `read` entry must not downgrade the policy out of full-disk
    /// write mode.
    fn has_write_narrowing_entries(&self, convention: PathConvention) -> bool {
        matches!(self.kind, FileSystemSandboxKind::Restricted)
            && self.entries.iter().any(|entry| {
                if entry.access.can_write() {
                    return false;
                }

                match &entry.path {
                    FileSystemPath::Path { .. } => {
                        !self.has_same_target_write_override(entry, convention)
                    }
                    FileSystemPath::GlobPattern { .. } => true,
                    FileSystemPath::Special { value } => match value {
                        FileSystemSpecialPath::Root => entry.access == FileSystemAccessMode::Deny,
                        FileSystemSpecialPath::SlashTmp
                            if convention == PathConvention::Windows =>
                        {
                            false
                        }
                        FileSystemSpecialPath::Minimal | FileSystemSpecialPath::Unknown { .. } => {
                            false
                        }
                        _ => !self.has_same_target_write_override(entry, convention),
                    },
                }
            })
    }

    /// Returns true when a higher-priority `write` entry targets the same
    /// location as `entry`, so `entry` cannot narrow effective write access.
    fn has_same_target_write_override(
        &self,
        entry: &FileSystemSandboxEntry,
        convention: PathConvention,
    ) -> bool {
        self.entries.iter().any(|candidate| {
            candidate.access.can_write()
                && candidate.access > entry.access
                && file_system_paths_share_target(&candidate.path, &entry.path, convention)
        })
    }

    /// Filesystem policy matching `WorkspaceWrite` semantics without requiring
    /// callers to construct a legacy [`SandboxPolicy`] first.
    pub fn workspace_write(
        writable_roots: &[AbsolutePathBuf],
        exclude_tmpdir_env_var: bool,
        exclude_slash_tmp: bool,
    ) -> Self {
        let mut entries = vec![FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        )];

        entries.push(FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            FileSystemAccessMode::Write,
        ));
        if !exclude_slash_tmp {
            entries.push(FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::SlashTmp,
                },
                FileSystemAccessMode::Write,
            ));
        }
        if !exclude_tmpdir_env_var {
            entries.push(FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Tmpdir,
                },
                FileSystemAccessMode::Write,
            ));
        }
        entries.extend(
            writable_roots
                .iter()
                .cloned()
                .map(|path| FileSystemSandboxEntry::new(path.into(), FileSystemAccessMode::Write)),
        );

        append_default_read_only_project_root_subpath_if_no_explicit_rule(&mut entries, ".git");
        append_default_read_only_project_root_subpath_if_no_explicit_rule(&mut entries, ".agents");
        append_default_read_only_project_root_subpath_if_no_explicit_rule(&mut entries, ".codex");
        for writable_root in writable_roots {
            for protected_path in default_read_only_subpaths_for_writable_root(
                writable_root,
                /*protect_missing_dot_codex*/ false,
            ) {
                append_default_read_only_path_if_no_explicit_rule(&mut entries, protected_path);
            }
        }

        FileSystemSandboxPolicy::restricted(entries)
    }

    /// Converts a legacy sandbox policy into an equivalent filesystem policy
    /// after resolving cwd-sensitive legacy defaults for the provided cwd.
    ///
    /// Legacy `WorkspaceWrite` policies may list readable roots that live
    /// under an already-writable root. Those paths were redundant in the
    /// legacy model and should not become read-only carveouts when projected
    /// into split filesystem policy.
    pub fn from_legacy_sandbox_policy_for_cwd(sandbox_policy: &SandboxPolicy, cwd: &Path) -> Self {
        let mut file_system_policy = Self::from(sandbox_policy);
        if let SandboxPolicy::WorkspaceWrite { writable_roots, .. } = sandbox_policy {
            if let Ok(cwd_root) = AbsolutePathBuf::from_absolute_path(cwd) {
                for protected_path in default_read_only_subpaths_for_writable_root(
                    &cwd_root, /*protect_missing_dot_codex*/ true,
                ) {
                    append_default_read_only_path_if_no_explicit_rule(
                        &mut file_system_policy.entries,
                        protected_path,
                    );
                }
            }
            for writable_root in writable_roots {
                for protected_path in default_read_only_subpaths_for_writable_root(
                    writable_root,
                    /*protect_missing_dot_codex*/ false,
                ) {
                    append_default_read_only_path_if_no_explicit_rule(
                        &mut file_system_policy.entries,
                        protected_path,
                    );
                }
            }
        }

        file_system_policy
    }

    /// Returns true when filesystem reads are unrestricted.
    pub fn has_full_disk_read_access(&self) -> bool {
        match self.kind {
            FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => true,
            FileSystemSandboxKind::Restricted => {
                self.has_root_access(FileSystemAccessMode::can_read)
                    && !self.has_denied_read_restrictions()
            }
        }
    }

    /// Returns true when filesystem writes are unrestricted on this host.
    pub fn has_full_disk_write_access(&self) -> bool {
        self.has_full_disk_write_access_for_convention(Some(PathConvention::native()))
    }

    /// Returns true when filesystem writes are unrestricted for the selected executor.
    pub fn has_full_disk_write_access_with_context(
        &self,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> bool {
        self.has_full_disk_write_access_for_convention(context.cwd.infer_path_convention())
    }

    fn has_full_disk_write_access_for_convention(
        &self,
        convention: Option<PathConvention>,
    ) -> bool {
        match self.kind {
            FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => true,
            FileSystemSandboxKind::Restricted => convention.is_some_and(|convention| {
                self.has_root_access(FileSystemAccessMode::can_write)
                    && !self.has_write_narrowing_entries(convention)
            }),
        }
    }

    /// Returns true when platform-default readable roots should be included.
    pub fn include_platform_defaults(&self) -> bool {
        !self.has_full_disk_read_access()
            && matches!(self.kind, FileSystemSandboxKind::Restricted)
            && self.entries.iter().any(|entry| {
                matches!(
                    &entry.path,
                    FileSystemPath::Special { value }
                        if matches!(value, FileSystemSpecialPath::Minimal)
                            && entry.access.can_read()
                )
            })
    }

    /// Native-path compatibility adapter for local executor boundaries.
    /// Selected-executor orchestrators must use [`Self::resolve_access`] with
    /// a [`PathUri`] and [`FileSystemSandboxPolicyContext`] instead.
    pub fn resolve_access_for_local_path_with_cwd(
        &self,
        path: &Path,
        cwd: &Path,
    ) -> FileSystemAccessMode {
        with_local_policy_context(path, cwd, |path, context| {
            self.resolve_access(path, context)
        })
        .unwrap_or(FileSystemAccessMode::Deny)
    }

    /// Native-path compatibility adapter for local executor boundaries.
    pub fn can_read_local_path_with_cwd(&self, path: &Path, cwd: &Path) -> bool {
        self.resolve_access_for_local_path_with_cwd(path, cwd)
            .can_read()
    }

    /// Native-path compatibility adapter for local executor boundaries.
    pub fn can_write_local_path_with_cwd(&self, path: &Path, cwd: &Path) -> bool {
        with_local_policy_context(path, cwd, |path, context| {
            self.can_write_path(path, context)
        })
        .unwrap_or(false)
    }

    pub fn resolve_access(
        &self,
        path: &PathUri,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> FileSystemAccessMode {
        match self.kind {
            FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => {
                return FileSystemAccessMode::Write;
            }
            FileSystemSandboxKind::Restricted => {}
        }

        let Some(convention) = context.cwd.infer_path_convention() else {
            return FileSystemAccessMode::Deny;
        };
        if path.infer_path_convention() != Some(convention)
            || path.lexical_depth().is_none()
            || context.cwd.lexical_depth().is_none()
        {
            return FileSystemAccessMode::Deny;
        }

        let Some(entries) = self.resolved_entries(context).into_iter().try_fold(
            Vec::new(),
            |mut entries, (root, access)| {
                match root.lexical_depth() {
                    Some(depth) => entries.push((root, access, depth)),
                    None if root.is_opaque() => {}
                    None => return None,
                }
                Some(entries)
            },
        ) else {
            return FileSystemAccessMode::Deny;
        };

        entries
            .into_iter()
            .filter(|(root, _, _)| path.starts_with(root))
            .max_by_key(|(_, access, depth)| (*depth, *access))
            .map(|(_, access, _)| access)
            .unwrap_or(FileSystemAccessMode::Deny)
    }

    pub fn can_write_path(
        &self,
        path: &PathUri,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> bool {
        if !self.resolve_access(path, context).can_write() {
            return false;
        }
        self.has_full_disk_write_access_with_context(context)
            || self.metadata_write_denial(path, context).is_none()
    }

    fn metadata_write_denial(
        &self,
        path: &PathUri,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> Option<&'static str> {
        if !matches!(self.kind, FileSystemSandboxKind::Restricted) {
            return None;
        }
        let entries = self.resolved_entries(context);
        let (protected, metadata_name) = entries
            .iter()
            .filter(|(_, access)| access.can_write())
            .find_map(|(root, _)| {
                PROTECTED_METADATA_PATH_NAMES
                    .iter()
                    .find_map(|metadata_name| {
                        let protected = root.join_descendant(metadata_name).ok()?;
                        path.starts_with(&protected)
                            .then_some((protected, *metadata_name))
                    })
            })?;
        (!entries.iter().any(|(root, access)| {
            access.can_write() && path.starts_with(root) && root.starts_with(&protected)
        }))
        .then_some(metadata_name)
    }

    fn prepare_deny_read_matcher(
        &self,
        context: &FileSystemSandboxPolicyContext<'_>,
        invalid_glob_behavior: InvalidDenyReadGlobBehavior,
    ) -> Result<PreparedReadDenyMatcher, String> {
        let file_system_root = file_system_root(context);
        let denied_roots = self
            .resolved_entries(context)
            .into_iter()
            .filter(|(_, access)| *access == FileSystemAccessMode::Deny)
            .filter(|(root, _)| {
                !file_system_root.as_ref().is_some_and(|file_system_root| {
                    root.starts_with(file_system_root) && file_system_root.starts_with(root)
                })
            })
            .map(|(root, _)| root)
            .collect();
        let Some(convention) = context.cwd.infer_path_convention() else {
            return Ok(PreparedReadDenyMatcher {
                denied_roots,
                deny_read_matchers: Vec::new(),
                invalid_pattern: true,
            });
        };
        let mut deny_read_matchers = Vec::new();
        let mut invalid_pattern = false;
        let patterns = match self.deny_read_globs(context) {
            Ok(patterns) => patterns,
            Err(err) => match invalid_glob_behavior {
                InvalidDenyReadGlobBehavior::FailClosed => {
                    return Ok(PreparedReadDenyMatcher {
                        denied_roots,
                        deny_read_matchers: Vec::new(),
                        invalid_pattern: true,
                    });
                }
                InvalidDenyReadGlobBehavior::ReturnError => return Err(err),
            },
        };
        for pattern in patterns {
            match build_glob_matcher(&pattern, convention) {
                Ok(matcher) => deny_read_matchers.push(matcher),
                Err(err) => match invalid_glob_behavior {
                    InvalidDenyReadGlobBehavior::FailClosed => invalid_pattern = true,
                    InvalidDenyReadGlobBehavior::ReturnError => {
                        return Err(format!("invalid deny-read glob pattern `{pattern}`: {err}"));
                    }
                },
            }
        }
        Ok(PreparedReadDenyMatcher {
            denied_roots,
            deny_read_matchers,
            invalid_pattern,
        })
    }

    fn matches_prepared_read_deny(
        path: &PathUri,
        context: &FileSystemSandboxPolicyContext<'_>,
        prepared: &PreparedReadDenyMatcher,
    ) -> bool {
        let Some(convention) = context.cwd.infer_path_convention() else {
            return true;
        };
        if path.infer_path_convention() != Some(convention)
            || path.lexical_depth().is_none()
            || context.cwd.lexical_depth().is_none()
        {
            return true;
        }
        if prepared.invalid_pattern {
            return true;
        }
        prepared
            .denied_roots
            .iter()
            .any(|root| path.starts_with(root))
            || prepared.deny_read_matchers.iter().any(|matcher| {
                let path = match convention {
                    PathConvention::Posix => path.decoded_path_bytes(),
                    PathConvention::Windows => Cow::Owned(
                        path.inferred_native_path_string()
                            .replace('\\', "/")
                            .into_bytes(),
                    ),
                };
                matcher.is_match_candidate(&Candidate::from_bytes(path.as_ref()))
            })
    }

    fn deny_read_globs(
        &self,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> Result<Vec<String>, String> {
        self.entries
            .iter()
            .filter(|entry| entry.access == FileSystemAccessMode::Deny)
            .filter_map(|entry| {
                let FileSystemPath::GlobPattern { pattern } = &entry.path else {
                    return None;
                };
                let is_windows =
                    context.cwd.infer_path_convention() == Some(PathConvention::Windows);
                let home_relative = pattern.strip_prefix("~/").or_else(|| {
                    is_windows.then(|| pattern.strip_prefix("~\\")).flatten()
                });
                let (root, pattern) = match home_relative {
                    Some(suffix) => match context.user_home_dir {
                        Some(home) => (
                            home,
                            suffix.trim_start_matches(|separator| {
                                separator == '/' || is_windows && separator == '\\'
                            }),
                        ),
                        None => {
                            return Some(Err(format!(
                                "unable to resolve deny-read glob pattern `{pattern}` without executor home"
                            )));
                        }
                    },
                    None => (context.cwd, pattern.as_str()),
                };
                Some(
                    root
                        .join(pattern)
                        .map(|path| path.inferred_native_path_string())
                        .map_err(|_| {
                            format!("unable to resolve deny-read glob pattern `{pattern}`")
                        }),
                )
            })
            .collect()
    }

    /// Replaces symbolic `:workspace_roots` entries with concrete entries for
    /// each workspace root.
    pub fn materialize_project_roots_with_workspace_roots(
        mut self,
        workspace_roots: &[AbsolutePathBuf],
    ) -> Self {
        let mut entries = Vec::with_capacity(self.entries.len());
        for entry in self.entries {
            match entry.path {
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::ProjectRoots { subpath },
                } => {
                    entries.extend(workspace_roots.iter().map(|root| FileSystemSandboxEntry {
                        path: FileSystemPath::from(match subpath.as_ref() {
                            Some(subpath) => {
                                AbsolutePathBuf::resolve_path_against_base(subpath, root.as_path())
                            }
                            None => root.clone(),
                        }),
                        access: entry.access,
                        missing_path_behavior: entry.missing_path_behavior,
                    }));
                }
                FileSystemPath::GlobPattern { pattern } => {
                    if let Some(subpath) = parse_project_roots_glob_pattern(&pattern) {
                        entries.extend(workspace_roots.iter().map(|root| FileSystemSandboxEntry {
                            path: FileSystemPath::GlobPattern {
                                pattern: resolve_project_roots_glob_pattern(subpath, root),
                            },
                            access: entry.access,
                            missing_path_behavior: entry.missing_path_behavior,
                        }));
                    } else {
                        entries.push(FileSystemSandboxEntry {
                            path: FileSystemPath::GlobPattern { pattern },
                            access: entry.access,
                            missing_path_behavior: entry.missing_path_behavior,
                        });
                    }
                }
                FileSystemPath::Path { path } => {
                    entries.push(FileSystemSandboxEntry {
                        path: path.into(),
                        access: entry.access,
                        missing_path_behavior: entry.missing_path_behavior,
                    });
                }
                FileSystemPath::Special { value } => {
                    entries.push(FileSystemSandboxEntry {
                        path: FileSystemPath::Special { value },
                        access: entry.access,
                        missing_path_behavior: entry.missing_path_behavior,
                    });
                }
            }
        }
        self.entries = entries;
        self
    }

    /// Materializes workspace-root entries without projecting executor paths onto the host.
    pub fn materialize_project_roots_with_path_uris(mut self, workspace_roots: &[PathUri]) -> Self {
        if let Ok(native_workspace_roots) = workspace_roots
            .iter()
            .map(PathUri::to_abs_path)
            .collect::<Result<Vec<_>, _>>()
        {
            return self.materialize_project_roots_with_workspace_roots(&native_workspace_roots);
        }

        let mut entries = Vec::with_capacity(self.entries.len());
        for entry in self.entries {
            let (subpath, is_glob) = match &entry.path {
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::ProjectRoots { subpath },
                } => (subpath.as_deref(), false),
                FileSystemPath::GlobPattern { pattern }
                    if pattern.starts_with(PROJECT_ROOTS_GLOB_PATTERN_PREFIX) =>
                {
                    (
                        Some(&pattern[PROJECT_ROOTS_GLOB_PATTERN_PREFIX.len()..]),
                        true,
                    )
                }
                _ => {
                    entries.push(entry);
                    continue;
                }
            };
            entries.extend(workspace_roots.iter().filter_map(|root| {
                let path = subpath.map_or_else(
                    || Some(root.clone()),
                    |subpath| resolve_scoped_workspace_path(root, subpath),
                );
                let (path, access) = match path {
                    Some(path) if is_glob => (
                        FileSystemPath::GlobPattern {
                            pattern: path.inferred_native_path_string(),
                        },
                        entry.access,
                    ),
                    Some(path) => (FileSystemPath::Path { path }, entry.access),
                    None if !entry.access.can_write() => (
                        FileSystemPath::Path { path: root.clone() },
                        FileSystemAccessMode::Deny,
                    ),
                    None => return None,
                };
                Some(FileSystemSandboxEntry {
                    path,
                    access,
                    missing_path_behavior: entry.missing_path_behavior,
                })
            }));
        }
        self.entries = entries;
        self
    }

    /// Preserves symbolic `:workspace_roots` entries while also adding concrete
    /// entries for each provided workspace root.
    pub fn with_materialized_project_roots_for_workspace_roots(
        mut self,
        workspace_roots: &[AbsolutePathBuf],
    ) -> Self {
        let materialized = self
            .clone()
            .materialize_project_roots_with_workspace_roots(workspace_roots);
        for entry in materialized.entries {
            if !self.entries.contains(&entry) {
                self.entries.push(entry);
            }
        }
        self
    }

    pub fn with_additional_readable_roots(
        mut self,
        cwd: &Path,
        additional_readable_roots: &[AbsolutePathBuf],
    ) -> Self {
        if self.has_full_disk_read_access() {
            return self;
        }

        for path in additional_readable_roots {
            if self.can_read_local_path_with_cwd(path.as_path(), cwd) {
                continue;
            }

            self.entries.push(FileSystemSandboxEntry::new(
                path.clone().into(),
                FileSystemAccessMode::Read,
            ));
        }

        self
    }

    pub fn with_additional_writable_roots(
        mut self,
        cwd: &Path,
        additional_writable_roots: &[AbsolutePathBuf],
    ) -> Self {
        for path in additional_writable_roots {
            if self.can_write_local_path_with_cwd(path.as_path(), cwd) {
                continue;
            }

            self.entries.push(FileSystemSandboxEntry::new(
                path.clone().into(),
                FileSystemAccessMode::Write,
            ));
        }

        self
    }

    /// Add roots using legacy `WorkspaceWrite` behavior.
    ///
    /// Unlike [`Self::with_additional_writable_roots`], this mirrors legacy
    /// writable-roots semantics by adding exact roots even when they are
    /// already writable through `:workspace_roots`, and by adding the default
    /// read-only protected subpaths for each new root.
    pub fn with_additional_legacy_workspace_writable_roots(
        mut self,
        additional_writable_roots: &[AbsolutePathBuf],
    ) -> Self {
        if !matches!(self.kind, FileSystemSandboxKind::Restricted) {
            return self;
        }

        for path in additional_writable_roots {
            if !self.entries.iter().any(|entry| {
                entry.access.can_write()
                    && matches!(&entry.path, FileSystemPath::Path { path: existing } if existing == &PathUri::from_abs_path(path))
            }) {
                self.entries.push(FileSystemSandboxEntry::new(
                    path.clone().into(),
                    FileSystemAccessMode::Write,
                ));
            }

            for protected_path in default_read_only_subpaths_for_writable_root(
                path, /*protect_missing_dot_codex*/ false,
            ) {
                append_default_read_only_path_if_no_explicit_rule(
                    &mut self.entries,
                    protected_path,
                );
            }
        }

        self
    }

    pub fn needs_direct_runtime_enforcement(
        &self,
        network_policy: NetworkSandboxPolicy,
        cwd: &Path,
    ) -> bool {
        if !matches!(self.kind, FileSystemSandboxKind::Restricted) {
            return false;
        }

        let Ok(legacy_policy) = self.to_legacy_sandbox_policy(network_policy, cwd) else {
            return true;
        };

        if protected_metadata_names_need_direct_runtime_enforcement(self, &legacy_policy, cwd) {
            return true;
        }

        self.semantic_signature(cwd)
            != legacy_runtime_file_system_policy_for_cwd(&legacy_policy, cwd)
                .semantic_signature(cwd)
    }

    /// Returns true when two policies resolve to the same filesystem access
    /// model for `cwd`, ignoring incidental entry ordering.
    pub fn is_semantically_equivalent_to(&self, other: &Self, cwd: &Path) -> bool {
        self.semantic_signature(cwd) == other.semantic_signature(cwd)
    }

    /// Returns the explicit readable roots resolved against the provided cwd.
    pub fn get_readable_roots_with_cwd(&self, cwd: &Path) -> Vec<AbsolutePathBuf> {
        if self.has_full_disk_read_access() {
            return Vec::new();
        }

        dedup_absolute_paths(
            self.resolved_entries_with_cwd(cwd)
                .into_iter()
                .filter(|entry| entry.access.can_read())
                .filter(|entry| self.can_read_local_path_with_cwd(entry.path.as_path(), cwd))
                .map(|entry| entry.path)
                .collect(),
            /*normalize_effective_paths*/ true,
        )
    }

    /// Returns the writable roots together with read-only carveouts resolved
    /// against the provided cwd.
    pub fn get_writable_roots_with_cwd(&self, cwd: &Path) -> Vec<WritableRoot> {
        self.get_writable_roots_with_cwd_impl(cwd, WritableRootPathResolution::Effective)
    }

    /// Reports configured writable roots for diagnostics without inspecting the filesystem.
    ///
    /// Unlike runtime root resolution, this includes configured roots even if they
    /// do not currently exist (including `/tmp`). Do not use this result to authorize
    /// filesystem access or replace the resolution needed for sandbox enforcement.
    pub fn has_configured_writable_roots_with_cwd(&self, cwd: &Path) -> bool {
        with_local_policy_context(cwd, cwd, |_, context| {
            self.has_configured_writable_roots(context)
        })
        .unwrap_or(false)
    }

    /// Reports configured writable roots for executor-context diagnostics, excluding
    /// full-disk policies and without inspecting the filesystem.
    pub fn has_configured_writable_roots(
        &self,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> bool {
        !self.has_full_disk_write_access_with_context(context)
            && self
                .resolved_entries(context)
                .into_iter()
                .any(|(path, access)| access.can_write() && self.can_write_path(&path, context))
    }

    /// Returns writable roots without following attacker-mutable path components.
    ///
    /// Trusted top-level aliases such as `/tmp -> /private/tmp` are still
    /// normalized so roots and carveouts are compared in the same namespace.
    /// Deeper components remain exactly as configured until the platform
    /// sandbox binds them.
    pub fn get_writable_roots_with_cwd_preserving_mutable_paths(
        &self,
        cwd: &Path,
    ) -> Vec<WritableRoot> {
        self.get_writable_roots_with_cwd_impl(
            cwd,
            WritableRootPathResolution::PreserveMutableComponents,
        )
    }

    fn get_writable_roots_with_cwd_impl(
        &self,
        cwd: &Path,
        path_resolution: WritableRootPathResolution,
    ) -> Vec<WritableRoot> {
        if self.has_full_disk_write_access() {
            return Vec::new();
        }

        let resolved_entries = self.resolved_entries_with_cwd(cwd);
        if !resolved_entries
            .iter()
            .any(|entry| entry.access.can_write())
        {
            return Vec::new();
        }
        // Resolve precedence and filesystem aliases once per entry, rather than repeating
        // that work for every writable root while collecting its read-only carveouts.
        let effective_entries: Vec<&ResolvedFileSystemEntry> = resolved_entries
            .iter()
            .filter(|entry| {
                entry.access.can_write()
                    == self.can_write_local_path_with_cwd(entry.path.as_path(), cwd)
            })
            .collect();
        if !effective_entries
            .iter()
            .any(|entry| entry.access.can_write())
        {
            return Vec::new();
        }
        let prepared_entries: Vec<PreparedFileSystemEntry<'_>> = effective_entries
            .into_iter()
            .map(|entry| PreparedFileSystemEntry {
                entry,
                effective_path: path_resolution.resolve(entry.path.clone()),
            })
            .collect();
        let writable_entries: Vec<&PreparedFileSystemEntry<'_>> = prepared_entries
            .iter()
            .filter(|entry| entry.entry.access.can_write())
            .collect();

        let effective_cwd = AbsolutePathBuf::from_absolute_path(cwd)
            .ok()
            .map(|cwd| path_resolution.resolve(cwd));

        dedup_absolute_paths(
            writable_entries
                .iter()
                .map(|entry| entry.effective_path.clone())
                .collect(),
            /*normalize_effective_paths*/ false,
        )
        .into_iter()
        .map(|root| {
            // Filesystem-root policies stay in their effective canonical form
            // so root-wide aliases do not create duplicate top-level masks.
            // Example: keep `/var/...` normalized under `/` instead of
            // materializing both `/var/...` and `/private/var/...`.
            // Nested symlink paths under a writable root stay logical so
            // downstream sandboxes can still bind the real target while
            // masking the user-visible symlink inode when needed.
            let preserve_raw_carveout_paths = root.as_path().parent().is_some();
            let raw_writable_roots: Vec<&AbsolutePathBuf> = writable_entries
                .iter()
                .filter(|entry| entry.effective_path == root)
                .map(|entry| &entry.entry.path)
                .collect();
            let protected_metadata_names =
                protected_metadata_names_for_writable_root(self, &root, &raw_writable_roots, cwd);
            let protect_missing_dot_codex = effective_cwd.as_ref() == Some(&root);
            let mut read_only_subpaths: Vec<AbsolutePathBuf> =
                default_read_only_subpaths_for_writable_root(&root, protect_missing_dot_codex)
                    .into_iter()
                    .filter(|path| !has_explicit_resolved_path_entry(&resolved_entries, path))
                    .collect();
            // Narrower explicit non-write entries carve out broader writable roots.
            // More specific write entries still remain writable because they appear
            // as separate WritableRoot values and are checked independently.
            // Preserve symlink path components that live under the writable root
            // so downstream sandboxes can still mask the symlink inode itself.
            // Example: if `<root>/.codex -> <root>/decoy`, bwrap must still see
            // `<root>/.codex`, not only the resolved `<root>/decoy`.
            read_only_subpaths.extend(
                prepared_entries
                    .iter()
                    .filter(|entry| !entry.entry.access.can_write())
                    .filter_map(|prepared| {
                        let entry = prepared.entry;
                        let effective_path = &prepared.effective_path;
                        // Preserve the literal in-root path whenever the
                        // carveout itself lives under this writable root, even
                        // if following symlinks would resolve back to the root
                        // or escape outside it. Downstream sandboxes need that
                        // raw path so they can mask the symlink inode itself.
                        // Examples:
                        // - `<root>/linked-private -> <root>/decoy-private`
                        // - `<root>/linked-private -> /tmp/outside-private`
                        // - `<root>/alias-root -> <root>`
                        let raw_carveout_path = if preserve_raw_carveout_paths {
                            if entry.path == root {
                                None
                            } else if entry.path.as_path().starts_with(root.as_path()) {
                                Some(entry.path.clone())
                            } else {
                                raw_writable_roots.iter().find_map(|raw_root| {
                                    let suffix = entry
                                        .path
                                        .as_path()
                                        .strip_prefix(raw_root.as_path())
                                        .ok()?;
                                    if suffix.as_os_str().is_empty() {
                                        return None;
                                    }
                                    Some(root.join(suffix))
                                })
                            }
                        } else {
                            None
                        };

                        if let Some(raw_carveout_path) = raw_carveout_path {
                            return Some(raw_carveout_path);
                        }

                        if effective_path == &root
                            || !effective_path.as_path().starts_with(root.as_path())
                        {
                            return None;
                        }

                        Some(effective_path.clone())
                    }),
            );
            WritableRoot {
                protected_metadata_names,
                root,
                // Preserve literal in-root protected paths like `.git` and
                // `.codex` so downstream sandboxes can still detect and mask
                // the symlink itself instead of only its resolved target.
                read_only_subpaths: dedup_absolute_paths(
                    read_only_subpaths,
                    /*normalize_effective_paths*/ false,
                ),
            }
        })
        .collect()
    }

    /// Returns explicit unreadable roots resolved against the provided cwd.
    pub fn get_unreadable_roots_with_cwd(&self, cwd: &Path) -> Vec<AbsolutePathBuf> {
        if !matches!(self.kind, FileSystemSandboxKind::Restricted) {
            return Vec::new();
        }

        let root = AbsolutePathBuf::from_absolute_path(cwd)
            .ok()
            .map(|cwd| absolute_root_path_for_cwd(&cwd));

        dedup_absolute_paths(
            self.resolved_entries_with_cwd(cwd)
                .iter()
                .filter(|entry| entry.access == FileSystemAccessMode::Deny)
                .filter(|entry| !self.can_read_local_path_with_cwd(entry.path.as_path(), cwd))
                // Restricted policies already deny reads outside explicit allow roots,
                // so materializing the filesystem root here would erase narrower
                // readable carveouts when downstream sandboxes apply deny masks last.
                .filter(|entry| root.as_ref() != Some(&entry.path))
                .map(|entry| entry.path.clone())
                .collect(),
            /*normalize_effective_paths*/ true,
        )
    }

    /// Returns unreadable glob patterns resolved against the provided cwd.
    pub fn get_unreadable_globs_with_cwd(&self, cwd: &Path) -> Vec<String> {
        if !matches!(self.kind, FileSystemSandboxKind::Restricted) {
            return Vec::new();
        }

        let mut patterns = self
            .entries
            .iter()
            .filter(|entry| entry.access == FileSystemAccessMode::Deny)
            .filter_map(|entry| match &entry.path {
                FileSystemPath::GlobPattern { pattern } => {
                    Some(AbsolutePathBuf::resolve_path_against_base(pattern, cwd))
                }
                FileSystemPath::Path { .. } | FileSystemPath::Special { .. } => None,
            })
            .map(|pattern| pattern.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        patterns.sort();
        patterns.dedup();
        patterns
    }

    pub fn to_legacy_sandbox_policy(
        &self,
        network_policy: NetworkSandboxPolicy,
        cwd: &Path,
    ) -> io::Result<SandboxPolicy> {
        Ok(match self.kind {
            FileSystemSandboxKind::ExternalSandbox => SandboxPolicy::ExternalSandbox {
                network_access: if network_policy.is_enabled() {
                    NetworkAccess::Enabled
                } else {
                    NetworkAccess::Restricted
                },
            },
            FileSystemSandboxKind::Unrestricted => {
                if network_policy.is_enabled() {
                    SandboxPolicy::DangerFullAccess
                } else {
                    SandboxPolicy::ExternalSandbox {
                        network_access: NetworkAccess::Restricted,
                    }
                }
            }
            FileSystemSandboxKind::Restricted => {
                let cwd_absolute = AbsolutePathBuf::from_absolute_path(cwd).ok();
                let has_full_disk_write_access = self.has_full_disk_write_access();
                let mut workspace_root_writable = false;
                let mut writable_roots = Vec::new();
                let mut tmpdir_writable = false;
                let mut slash_tmp_writable = false;
                let mut unbridgeable_root_write = false;

                for entry in &self.entries {
                    match &entry.path {
                        FileSystemPath::GlobPattern { .. } => {}
                        FileSystemPath::Path { path } => {
                            if entry.access.can_write() {
                                let path = path.to_abs_path()?;
                                if cwd_absolute.as_ref().is_some_and(|cwd| cwd == &path) {
                                    workspace_root_writable = true;
                                } else {
                                    writable_roots.push(path);
                                }
                            }
                        }
                        FileSystemPath::Special { value } => match value {
                            FileSystemSpecialPath::Root => match entry.access {
                                FileSystemAccessMode::Deny => {}
                                FileSystemAccessMode::Read => {}
                                FileSystemAccessMode::Write => {
                                    unbridgeable_root_write = true;
                                }
                            },
                            FileSystemSpecialPath::Minimal => {}
                            FileSystemSpecialPath::ProjectRoots { subpath } => {
                                if subpath.is_none() && entry.access.can_write() {
                                    workspace_root_writable = true;
                                } else if let Some(path) =
                                    resolve_file_system_special_path(value, cwd_absolute.as_ref())
                                    && entry.access.can_write()
                                {
                                    writable_roots.push(path);
                                }
                            }
                            FileSystemSpecialPath::Tmpdir => {
                                if entry.access.can_write() {
                                    tmpdir_writable = true;
                                }
                            }
                            FileSystemSpecialPath::SlashTmp => {
                                if entry.access.can_write() {
                                    slash_tmp_writable = true;
                                }
                            }
                            FileSystemSpecialPath::Unknown { .. } => {}
                        },
                    }
                }

                if has_full_disk_write_access {
                    return Ok(if network_policy.is_enabled() {
                        SandboxPolicy::DangerFullAccess
                    } else {
                        SandboxPolicy::ExternalSandbox {
                            network_access: NetworkAccess::Restricted,
                        }
                    });
                }

                if workspace_root_writable {
                    SandboxPolicy::WorkspaceWrite {
                        writable_roots: dedup_absolute_paths(
                            writable_roots,
                            /*normalize_effective_paths*/ false,
                        ),
                        network_access: network_policy.is_enabled(),
                        exclude_tmpdir_env_var: !tmpdir_writable,
                        exclude_slash_tmp: !slash_tmp_writable,
                    }
                } else if unbridgeable_root_write
                    || !writable_roots.is_empty()
                    || tmpdir_writable
                    || (cfg!(unix) && slash_tmp_writable)
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "permissions profile requests filesystem writes outside the workspace root, which is not supported until the runtime enforces FileSystemSandboxPolicy directly",
                    ));
                } else {
                    SandboxPolicy::ReadOnly {
                        network_access: network_policy.is_enabled(),
                    }
                }
            }
        })
    }

    fn resolved_entries_with_cwd(&self, cwd: &Path) -> Vec<ResolvedFileSystemEntry> {
        let cwd_absolute = AbsolutePathBuf::from_absolute_path(cwd).ok();
        self.entries
            .iter()
            .filter_map(|entry| {
                resolve_entry_path(&entry.path, cwd_absolute.as_ref()).map(|path| {
                    ResolvedFileSystemEntry {
                        path,
                        access: entry.access,
                    }
                })
            })
            .collect()
    }

    fn resolved_entries(
        &self,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> Vec<(PathUri, FileSystemAccessMode)> {
        let convention = context.cwd.infer_path_convention();
        self.entries
            .iter()
            .flat_map(|entry| {
                let paths = match &entry.path {
                    FileSystemPath::Path { path } => vec![path.clone()],
                    FileSystemPath::GlobPattern { .. } => Vec::new(),
                    FileSystemPath::Special { value } => match value {
                        FileSystemSpecialPath::Root => {
                            file_system_root(context).into_iter().collect()
                        }
                        FileSystemSpecialPath::ProjectRoots { subpath } => context
                            .workspace_roots
                            .iter()
                            .filter_map(|root| match subpath {
                                Some(subpath) => root.join(subpath).ok(),
                                None => Some(root.clone()),
                            })
                            .collect(),
                        FileSystemSpecialPath::Tmpdir => {
                            context.temporary_directories.unwrap_or_default().to_vec()
                        }
                        FileSystemSpecialPath::SlashTmp
                            if convention == Some(PathConvention::Posix) =>
                        {
                            context.cwd.join("/tmp").into_iter().collect()
                        }
                        FileSystemSpecialPath::SlashTmp
                        | FileSystemSpecialPath::Minimal
                        | FileSystemSpecialPath::Unknown { .. } => Vec::new(),
                    },
                };
                paths
                    .into_iter()
                    .filter(move |path| path.infer_path_convention() == convention)
                    .map(move |path| (path, entry.access))
            })
            .collect()
    }

    fn semantic_signature(&self, cwd: &Path) -> FileSystemSemanticSignature {
        FileSystemSemanticSignature {
            has_full_disk_read_access: self.has_full_disk_read_access(),
            has_full_disk_write_access: self.has_full_disk_write_access(),
            include_platform_defaults: self.include_platform_defaults(),
            readable_roots: sorted_absolute_paths(self.get_readable_roots_with_cwd(cwd)),
            writable_roots: sorted_writable_roots(self.get_writable_roots_with_cwd(cwd)),
            unreadable_roots: sorted_absolute_paths(self.get_unreadable_roots_with_cwd(cwd)),
            unreadable_globs: self.get_unreadable_globs_with_cwd(cwd),
        }
    }
}

impl From<&SandboxPolicy> for NetworkSandboxPolicy {
    fn from(value: &SandboxPolicy) -> Self {
        if value.has_full_network_access() {
            NetworkSandboxPolicy::Enabled
        } else {
            NetworkSandboxPolicy::Restricted
        }
    }
}

impl From<&SandboxPolicy> for FileSystemSandboxPolicy {
    fn from(value: &SandboxPolicy) -> Self {
        match value {
            SandboxPolicy::DangerFullAccess => FileSystemSandboxPolicy::unrestricted(),
            SandboxPolicy::ExternalSandbox { .. } => FileSystemSandboxPolicy::external_sandbox(),
            SandboxPolicy::ReadOnly { .. } => {
                FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    FileSystemAccessMode::Read,
                )])
            }
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
                ..
            } => FileSystemSandboxPolicy::workspace_write(
                writable_roots,
                *exclude_tmpdir_env_var,
                *exclude_slash_tmp,
            ),
        }
    }
}

fn resolve_file_system_path(
    path: &FileSystemPath,
    cwd: Option<&AbsolutePathBuf>,
) -> Option<AbsolutePathBuf> {
    match path {
        FileSystemPath::Path { path } => path.to_abs_path().ok(),
        FileSystemPath::GlobPattern { .. } => None,
        FileSystemPath::Special { value } => resolve_file_system_special_path(value, cwd),
    }
}

fn resolve_entry_path(
    path: &FileSystemPath,
    cwd: Option<&AbsolutePathBuf>,
) -> Option<AbsolutePathBuf> {
    match path {
        FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        } => cwd.map(absolute_root_path_for_cwd),
        _ => resolve_file_system_path(path, cwd),
    }
}

fn parse_project_roots_glob_pattern(pattern: &str) -> Option<&Path> {
    pattern
        .strip_prefix(PROJECT_ROOTS_GLOB_PATTERN_PREFIX)
        .map(Path::new)
}

fn resolve_project_roots_glob_pattern(subpath: &Path, root: &AbsolutePathBuf) -> String {
    AbsolutePathBuf::resolve_path_against_base(subpath, root.as_path())
        .to_string_lossy()
        .into_owned()
}

fn resolve_candidate_path(path: &Path, cwd: &Path) -> Option<AbsolutePathBuf> {
    if path.is_absolute() {
        AbsolutePathBuf::from_absolute_path(path).ok()
    } else {
        Some(AbsolutePathBuf::from_absolute_path(cwd).ok()?.join(path))
    }
}

/// Resolves a workspace-relative path using the root's own path convention.
///
/// Rejects absolute paths, traversal, Windows drive changes, and any result
/// outside the root so foreign-platform permission rules cannot escape scope.
fn resolve_scoped_workspace_path(root: &PathUri, subpath: &str) -> Option<PathUri> {
    let convention = root.infer_path_convention()?;
    if subpath.starts_with('/')
        || convention == PathConvention::Windows && subpath.starts_with('\\')
        || convention
            .path_segments(subpath)
            .any(|segment| segment == "." || segment == "..")
        || convention == PathConvention::Windows
            && convention
                .path_segments(subpath)
                .any(|segment| segment.contains(':'))
    {
        return None;
    }
    let path = root.join(subpath).ok()?;
    path.starts_with(root).then_some(path)
}

fn with_local_policy_context<T>(
    path: &Path,
    cwd: &Path,
    evaluate: impl FnOnce(&PathUri, &FileSystemSandboxPolicyContext<'_>) -> T,
) -> Option<T> {
    let cwd = AbsolutePathBuf::from_absolute_path(cwd).ok()?;
    let path = PathUri::from(resolve_candidate_path(path, cwd.as_path())?);
    let cwd = PathUri::from(cwd);
    let user_home_dir = PathUri::from_host_native_path("~").ok();
    let temporary_directories = local_temporary_directories();
    let context = FileSystemSandboxPolicyContext {
        cwd: &cwd,
        workspace_roots: std::slice::from_ref(&cwd),
        user_home_dir: user_home_dir.as_ref(),
        temporary_directories: Some(&temporary_directories),
    };
    Some(evaluate(&path, &context))
}

pub fn file_system_root(context: &FileSystemSandboxPolicyContext<'_>) -> Option<PathUri> {
    context.cwd.lexical_depth()?;
    context.cwd.ancestors().last()
}

fn local_temporary_directories() -> Vec<PathUri> {
    let Some(tmpdir) = std::env::var_os("TMPDIR").filter(|path| !path.is_empty()) else {
        return Vec::new();
    };
    AbsolutePathBuf::from_absolute_path(PathBuf::from(tmpdir))
        .ok()
        .map(PathUri::from)
        .into_iter()
        .collect()
}

/// Returns true when two config paths refer to the same exact target before
/// any prefix matching is applied.
///
/// This is intentionally narrower than full path resolution: it only answers
/// the "can one entry shadow another at the same specificity?" question used
/// by `has_write_narrowing_entries`.
fn file_system_paths_share_target(
    left: &FileSystemPath,
    right: &FileSystemPath,
    convention: PathConvention,
) -> bool {
    match (left, right) {
        (FileSystemPath::Path { path: left }, FileSystemPath::Path { path: right }) => {
            left == right
        }
        (FileSystemPath::Special { value: left }, FileSystemPath::Special { value: right }) => {
            special_paths_share_target(left, right)
        }
        (FileSystemPath::Path { path }, FileSystemPath::Special { value })
        | (FileSystemPath::Special { value }, FileSystemPath::Path { path }) => {
            path.infer_path_convention() == Some(convention)
                && special_path_matches_path_uri(value, path)
        }
        (
            FileSystemPath::GlobPattern { pattern: left },
            FileSystemPath::GlobPattern { pattern: right },
        ) => left == right,
        (FileSystemPath::GlobPattern { .. }, _) | (_, FileSystemPath::GlobPattern { .. }) => false,
    }
}

/// Compares special-path tokens that resolve to the same concrete target
/// without needing a cwd.
fn special_paths_share_target(left: &FileSystemSpecialPath, right: &FileSystemSpecialPath) -> bool {
    match (left, right) {
        (FileSystemSpecialPath::Root, FileSystemSpecialPath::Root)
        | (FileSystemSpecialPath::Minimal, FileSystemSpecialPath::Minimal)
        | (FileSystemSpecialPath::Tmpdir, FileSystemSpecialPath::Tmpdir)
        | (FileSystemSpecialPath::SlashTmp, FileSystemSpecialPath::SlashTmp) => true,
        (
            FileSystemSpecialPath::ProjectRoots { subpath: left },
            FileSystemSpecialPath::ProjectRoots { subpath: right },
        ) => left == right,
        (
            FileSystemSpecialPath::Unknown {
                path: left,
                subpath: left_subpath,
            },
            FileSystemSpecialPath::Unknown {
                path: right,
                subpath: right_subpath,
            },
        ) => left == right && left_subpath == right_subpath,
        _ => false,
    }
}

/// Matches cwd-independent special paths against `PathUri` entries when
/// they name the same location.
///
/// We intentionally only fold the special paths whose concrete meaning is
/// stable without a cwd, such as `/` and `/tmp`.
fn special_path_matches_path_uri(value: &FileSystemSpecialPath, path: &PathUri) -> bool {
    match value {
        FileSystemSpecialPath::Root => path.lexical_depth().is_some() && path.parent().is_none(),
        FileSystemSpecialPath::SlashTmp => {
            path.infer_path_convention() == Some(PathConvention::Posix)
                && path.lexical_depth() == Some(1)
                && path.basename().as_deref() == Some("tmp")
        }
        _ => false,
    }
}

fn absolute_root_path_for_cwd(cwd: &AbsolutePathBuf) -> AbsolutePathBuf {
    let root = cwd
        .as_path()
        .ancestors()
        .last()
        .unwrap_or_else(|| panic!("cwd must have a filesystem root"));
    AbsolutePathBuf::from_absolute_path(root)
        .unwrap_or_else(|err| panic!("cwd root must be an absolute path: {err}"))
}

fn build_glob_matcher(pattern: &str, convention: PathConvention) -> Result<GlobMatcher, String> {
    // Keep `*` and `?` within a single path component and preserve an unclosed
    // `[` as a literal so matcher behavior stays aligned with config parsing.
    let pattern = if convention == PathConvention::Windows {
        pattern.replace('\\', "/")
    } else {
        pattern.to_string()
    };
    GlobBuilder::new(&pattern)
        .literal_separator(true)
        .allow_unclosed_class(true)
        .backslash_escape(convention == PathConvention::Posix)
        .case_insensitive(convention == PathConvention::Windows)
        .build()
        .map(|glob| glob.compile_matcher())
        .map_err(|err| err.to_string())
}

fn resolve_file_system_special_path(
    value: &FileSystemSpecialPath,
    cwd: Option<&AbsolutePathBuf>,
) -> Option<AbsolutePathBuf> {
    match value {
        FileSystemSpecialPath::Root
        | FileSystemSpecialPath::Minimal
        | FileSystemSpecialPath::Unknown { .. } => None,
        FileSystemSpecialPath::ProjectRoots { subpath } => {
            let cwd = cwd?;
            match subpath.as_ref() {
                Some(subpath) => Some(AbsolutePathBuf::resolve_path_against_base(
                    subpath,
                    cwd.as_path(),
                )),
                None => Some(cwd.clone()),
            }
        }
        FileSystemSpecialPath::Tmpdir => {
            let tmpdir = std::env::var_os("TMPDIR")?;
            if tmpdir.is_empty() {
                None
            } else {
                let tmpdir = AbsolutePathBuf::from_absolute_path(PathBuf::from(tmpdir)).ok()?;
                Some(tmpdir)
            }
        }
        FileSystemSpecialPath::SlashTmp => {
            if !cfg!(unix) {
                return None;
            }
            #[allow(clippy::expect_used)]
            let slash_tmp = AbsolutePathBuf::from_absolute_path("/tmp").expect("/tmp is absolute");
            if !slash_tmp.as_path().is_dir() {
                return None;
            }
            Some(slash_tmp)
        }
    }
}

fn dedup_absolute_paths(
    paths: Vec<AbsolutePathBuf>,
    normalize_effective_paths: bool,
) -> Vec<AbsolutePathBuf> {
    let mut deduped = Vec::with_capacity(paths.len());
    let mut seen = HashSet::new();
    for path in paths {
        let dedup_path = if normalize_effective_paths {
            normalize_effective_absolute_path(path)
        } else {
            path
        };
        if seen.insert(dedup_path.to_path_buf()) {
            deduped.push(dedup_path);
        }
    }
    deduped
}

fn sorted_absolute_paths(mut paths: Vec<AbsolutePathBuf>) -> Vec<AbsolutePathBuf> {
    paths.sort_by(|left, right| left.as_path().cmp(right.as_path()));
    paths
}

fn sorted_writable_roots(mut roots: Vec<WritableRoot>) -> Vec<WritableRoot> {
    for root in &mut roots {
        root.read_only_subpaths =
            sorted_absolute_paths(std::mem::take(&mut root.read_only_subpaths));
        root.protected_metadata_names.sort();
        root.protected_metadata_names.dedup();
    }
    roots.sort_by(|left, right| left.root.as_path().cmp(right.root.as_path()));
    roots
}

fn normalize_effective_absolute_path(path: AbsolutePathBuf) -> AbsolutePathBuf {
    let raw_path = path.to_path_buf();
    for ancestor in raw_path.ancestors() {
        if std::fs::symlink_metadata(ancestor).is_err() {
            continue;
        }
        let Ok(normalized_ancestor) = canonicalize_preserving_symlinks(ancestor) else {
            continue;
        };
        let Ok(suffix) = raw_path.strip_prefix(ancestor) else {
            continue;
        };
        if let Ok(normalized_path) =
            AbsolutePathBuf::from_absolute_path(normalized_ancestor.join(suffix))
        {
            return normalized_path;
        }
    }
    path
}

fn normalize_trusted_top_level_alias(path: AbsolutePathBuf) -> AbsolutePathBuf {
    let Some(top_level) = path.as_path().ancestors().find(|ancestor| {
        ancestor.parent().is_some() && ancestor.parent().and_then(Path::parent).is_none()
    }) else {
        return path;
    };
    let Ok(metadata) = std::fs::symlink_metadata(top_level) else {
        return path;
    };
    if !metadata.file_type().is_symlink() {
        return path;
    }
    let Ok(canonical_top_level) = top_level.canonicalize() else {
        return path;
    };
    let Ok(suffix) = path.as_path().strip_prefix(top_level) else {
        return path;
    };
    AbsolutePathBuf::from_absolute_path(canonical_top_level.join(suffix)).unwrap_or(path)
}

pub(crate) fn default_read_only_subpaths_for_writable_root(
    writable_root: &AbsolutePathBuf,
    protect_missing_dot_codex: bool,
) -> Vec<AbsolutePathBuf> {
    let mut subpaths: Vec<AbsolutePathBuf> = Vec::new();
    let top_level_git = writable_root.join(PROTECTED_METADATA_GIT_PATH_NAME);
    // This applies to typical repos (directory .git), worktrees/submodules
    // (file .git with gitdir pointer), and bare repos when the gitdir is the
    // writable root itself.
    let top_level_git_is_file = top_level_git.as_path().is_file();
    let top_level_git_is_dir = top_level_git.as_path().is_dir();
    let should_protect_top_level = top_level_git_is_dir || top_level_git_is_file;
    if should_protect_top_level {
        if top_level_git_is_file
            && is_git_pointer_file(&top_level_git)
            && let Some(gitdir) = resolve_gitdir_from_file(&top_level_git)
        {
            subpaths.push(gitdir);
        }
        subpaths.push(top_level_git);
    }

    let top_level_agents = writable_root.join(PROTECTED_METADATA_AGENTS_PATH_NAME);
    if top_level_agents.as_path().is_dir() {
        subpaths.push(top_level_agents);
    }

    // Keep top-level project metadata under .codex read-only to the agent by
    // default. For the workspace root itself, protect it even before the
    // directory exists so first-time creation still goes through the
    // protected-path approval flow.
    let top_level_codex = writable_root.join(PROTECTED_METADATA_CODEX_PATH_NAME);
    if protect_missing_dot_codex || top_level_codex.as_path().is_dir() {
        subpaths.push(top_level_codex);
    }

    dedup_absolute_paths(subpaths, /*normalize_effective_paths*/ false)
}

/// Rebuilds the filesystem policy that legacy sandbox runtimes enforce for a
/// concrete cwd.
///
/// Unlike [`FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd`], this
/// intentionally does not add symbolic project-root metadata carveouts. Legacy
/// runtime expansion only protected `.git`/`.agents` when those paths already
/// existed, so missing-path carveouts still require direct profile enforcement.
fn legacy_runtime_file_system_policy_for_cwd(
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> FileSystemSandboxPolicy {
    let SandboxPolicy::WorkspaceWrite {
        writable_roots,
        exclude_tmpdir_env_var,
        exclude_slash_tmp,
        ..
    } = sandbox_policy
    else {
        return FileSystemSandboxPolicy::from(sandbox_policy);
    };

    let mut entries = vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            FileSystemAccessMode::Write,
        ),
    ];

    if !*exclude_slash_tmp {
        entries.push(FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::SlashTmp,
            },
            FileSystemAccessMode::Write,
        ));
    }
    if !*exclude_tmpdir_env_var {
        entries.push(FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            },
            FileSystemAccessMode::Write,
        ));
    }
    entries.extend(
        writable_roots
            .iter()
            .cloned()
            .map(|path| FileSystemSandboxEntry::new(path.into(), FileSystemAccessMode::Write)),
    );

    if let Ok(cwd_root) = AbsolutePathBuf::from_absolute_path(cwd) {
        for protected_path in default_read_only_subpaths_for_writable_root(
            &cwd_root, /*protect_missing_dot_codex*/ true,
        ) {
            append_default_read_only_path_if_no_explicit_rule(&mut entries, protected_path);
        }
    }
    for writable_root in writable_roots {
        for protected_path in default_read_only_subpaths_for_writable_root(
            writable_root,
            /*protect_missing_dot_codex*/ false,
        ) {
            append_default_read_only_path_if_no_explicit_rule(&mut entries, protected_path);
        }
    }

    FileSystemSandboxPolicy::restricted(entries)
}

fn append_default_read_only_project_root_subpath_if_no_explicit_rule(
    entries: &mut Vec<FileSystemSandboxEntry>,
    subpath: impl Into<String>,
) {
    append_default_read_only_entry_if_no_explicit_rule(
        entries,
        FileSystemPath::Special {
            value: FileSystemSpecialPath::project_roots(Some(subpath.into())),
        },
    );
}

fn append_default_read_only_path_if_no_explicit_rule(
    entries: &mut Vec<FileSystemSandboxEntry>,
    path: AbsolutePathBuf,
) {
    append_default_read_only_entry_if_no_explicit_rule(entries, path.into());
}

fn append_default_read_only_entry_if_no_explicit_rule(
    entries: &mut Vec<FileSystemSandboxEntry>,
    path: FileSystemPath,
) {
    if entries
        .iter()
        .any(|entry| file_system_paths_share_target(&entry.path, &path, PathConvention::native()))
    {
        return;
    }

    entries.push(FileSystemSandboxEntry::skip_missing_path(
        path,
        FileSystemAccessMode::Read,
    ));
}

fn has_explicit_resolved_path_entry(
    entries: &[ResolvedFileSystemEntry],
    path: &AbsolutePathBuf,
) -> bool {
    entries.iter().any(|entry| &entry.path == path)
}

fn protected_metadata_names_for_writable_root(
    policy: &FileSystemSandboxPolicy,
    root: &AbsolutePathBuf,
    raw_writable_roots: &[&AbsolutePathBuf],
    cwd: &Path,
) -> Vec<String> {
    let mut protected_names = Vec::new();
    for metadata_name in PROTECTED_METADATA_PATH_NAMES {
        let mut metadata_paths = vec![root.join(*metadata_name)];
        metadata_paths.extend(
            raw_writable_roots
                .iter()
                .map(|raw_root| raw_root.join(*metadata_name)),
        );

        if metadata_paths.iter().all(|metadata_path| {
            !policy.can_write_local_path_with_cwd(metadata_path.as_path(), cwd)
        }) {
            protected_names.push((*metadata_name).to_string());
        }
    }
    protected_names
}

fn protected_metadata_names_need_direct_runtime_enforcement(
    policy: &FileSystemSandboxPolicy,
    legacy_policy: &SandboxPolicy,
    cwd: &Path,
) -> bool {
    let legacy_roots = legacy_policy.get_writable_roots_with_cwd(cwd);
    policy
        .get_writable_roots_with_cwd(cwd)
        .into_iter()
        .any(|writable_root| {
            let Some(legacy_root) = legacy_roots
                .iter()
                .find(|candidate| candidate.root == writable_root.root)
            else {
                return !writable_root.protected_metadata_names.is_empty();
            };

            writable_root
                .protected_metadata_names
                .iter()
                .any(|metadata_name| {
                    let metadata_path = writable_root.root.join(metadata_name);
                    !legacy_root
                        .read_only_subpaths
                        .iter()
                        .any(|subpath| subpath == &metadata_path)
                })
        })
}

fn is_git_pointer_file(path: &AbsolutePathBuf) -> bool {
    path.as_path().is_file()
        && path.as_path().file_name() == Some(OsStr::new(PROTECTED_METADATA_GIT_PATH_NAME))
}

fn resolve_gitdir_from_file(dot_git: &AbsolutePathBuf) -> Option<AbsolutePathBuf> {
    let contents = match std::fs::read_to_string(dot_git.as_path()) {
        Ok(contents) => contents,
        Err(err) => {
            error!(
                "Failed to read {path} for gitdir pointer: {err}",
                path = dot_git.as_path().display()
            );
            return None;
        }
    };

    let trimmed = contents.trim();
    let (_, gitdir_raw) = match trimmed.split_once(':') {
        Some((prefix, gitdir_raw)) if prefix.trim() == "gitdir" => (prefix, gitdir_raw),
        Some(_) => {
            error!(
                "Expected {path} to contain a gitdir pointer, but it did not match `gitdir: <path>`.",
                path = dot_git.as_path().display()
            );
            return None;
        }
        None => {
            error!(
                "Expected {path} to contain a gitdir pointer, but it did not match `gitdir: <path>`.",
                path = dot_git.as_path().display()
            );
            return None;
        }
    };
    let gitdir_raw = gitdir_raw.trim();
    if gitdir_raw.is_empty() {
        error!(
            "Expected {path} to contain a gitdir pointer, but it was empty.",
            path = dot_git.as_path().display()
        );
        return None;
    }
    let base = match dot_git.as_path().parent() {
        Some(base) => base,
        None => {
            error!(
                "Unable to resolve parent directory for {path}.",
                path = dot_git.as_path().display()
            );
            return None;
        }
    };
    let gitdir_path = AbsolutePathBuf::resolve_path_against_base(gitdir_raw, base);
    if !gitdir_path.as_path().exists() {
        error!(
            "Resolved gitdir path {path} does not exist.",
            path = gitdir_path.as_path().display()
        );
        return None;
    }
    Some(gitdir_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    #[cfg(unix)]
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[cfg(unix)]
    const SYMLINKED_TMPDIR_TEST_ENV: &str = "CODEX_PROTOCOL_TEST_SYMLINKED_TMPDIR";

    #[cfg(unix)]
    fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(original, link)
    }

    #[test]
    fn permission_paths_preserve_native_strings_across_path_conventions() {
        for path in [
            "/workspace/src",
            r"C:\workspace\src",
            r"\\server\share\src",
            r"\\localhost\share",
        ] {
            let expected = serde_json::json!({ "type": "path", "path": path });
            let actual = serde_json::from_value::<RawFileSystemPath>(expected.clone())
                .expect("valid raw permission path");
            assert_eq!(
                serde_json::to_value(actual).expect("lossless raw permission path"),
                expected
            );
        }
    }

    #[test]
    fn writable_root_presence_matches_materialized_roots() {
        let cwd = TempDir::new().expect("tempdir");
        let writable_root = AbsolutePathBuf::resolve_path_against_base("work", cwd.path());
        let policies = [
            FileSystemSandboxPolicy::read_only(),
            FileSystemSandboxPolicy::unrestricted(),
            FileSystemSandboxPolicy::external_sandbox(),
            FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Write,
            )]),
            FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
                writable_root.clone().into(),
                FileSystemAccessMode::Write,
            )]),
            FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry::new(
                    writable_root.clone().into(),
                    FileSystemAccessMode::Write,
                ),
                FileSystemSandboxEntry::new(writable_root.into(), FileSystemAccessMode::Deny),
            ]),
            FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::unknown(
                        ":future_special_path",
                        /*subpath*/ None,
                    ),
                },
                FileSystemAccessMode::Write,
            )]),
        ];

        for policy in policies {
            assert_eq!(
                policy.has_configured_writable_roots_with_cwd(cwd.path()),
                !policy.get_writable_roots_with_cwd(cwd.path()).is_empty()
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn permission_paths_preserve_native_slash_unc_strings() {
        for path in ["//server/share/src", r"/\server/share/src"] {
            let expected = serde_json::json!({ "type": "path", "path": path });
            let actual = serde_json::from_value::<RawFileSystemPath>(expected.clone())
                .expect("valid raw slash UNC permission path");
            assert_eq!(
                serde_json::to_value(actual).expect("lossless raw slash UNC permission path"),
                expected
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn native_ambiguous_permission_paths_keep_deny_semantics() {
        let cwd = TempDir::new().expect("tempdir");
        for path in ["//server/share/secret", "/C:/secret"] {
            let denied_path = serde_json::from_value::<RawFileSystemPath>(serde_json::json!({
                "type": "path",
                "path": path,
            }))
            .expect("raw permission path")
            .try_into()
            .expect("runtime permission path");
            let policy = FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    FileSystemAccessMode::Read,
                ),
                FileSystemSandboxEntry::new(denied_path, FileSystemAccessMode::Deny),
            ]);

            assert!(
                !policy.can_read_local_path_with_cwd(Path::new(path), cwd.path()),
                "deny should apply to {path}"
            );
            assert!(
                policy.can_read_local_path_with_cwd(&cwd.path().join("ordinary"), cwd.path()),
                "opaque deny for {path} should not poison ordinary paths"
            );
        }
    }

    #[test]
    fn uri_matcher_resolves_selected_executor_paths() {
        let path = |path| PathUri::parse(path).expect("valid path URI");
        let cwd = path("file:///C:/workspace");
        let workspace_roots = [cwd.clone()];
        let temporary_directories = [path("file:///C:/Temp")];
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd,
            workspace_roots: &workspace_roots,
            user_home_dir: None,
            temporary_directories: Some(&temporary_directories),
        };
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                FileSystemAccessMode::Write,
            ),
            FileSystemSandboxEntry::new(
                path("file:///C:/workspace/private").into(),
                FileSystemAccessMode::Deny,
            ),
            FileSystemSandboxEntry::new(
                path("file:///C:/workspace/private/public").into(),
                FileSystemAccessMode::Write,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Tmpdir,
                },
                FileSystemAccessMode::Write,
            ),
        ]);

        for (candidate, expected) in [
            (
                "file:///C:/workspace/src/main.rs",
                FileSystemAccessMode::Write,
            ),
            (
                "file:///c:/WORKSPACE/private/key",
                FileSystemAccessMode::Deny,
            ),
            (
                "file:///C:/workspace/private/public/ok",
                FileSystemAccessMode::Write,
            ),
            ("file:///C:/Temp/cache", FileSystemAccessMode::Write),
            ("file:///C:/outside", FileSystemAccessMode::Read),
            ("file:///tmp/cache", FileSystemAccessMode::Deny),
        ] {
            assert_eq!(
                policy.resolve_access(&path(candidate), &context),
                expected,
                "resolving {candidate}"
            );
        }
        assert!(!policy.can_write_path(&path("file:///c:/WORKSPACE/.git/config"), &context,));

        let scoped = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some("docs/../public".to_string())),
            },
            FileSystemAccessMode::Write,
        )]);
        assert_eq!(
            scoped.resolve_access(&path("file:///C:/workspace/public/file"), &context),
            FileSystemAccessMode::Write
        );
        assert_eq!(
            scoped.resolve_access(&path("file:///C:/outside/file"), &context),
            FileSystemAccessMode::Deny
        );
    }

    #[test]
    fn uri_matcher_uses_validated_native_components_for_precedence() {
        let path = |path| PathUri::parse(path).expect("valid path URI");
        let cwd = path("file:///workspace");
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd,
            workspace_roots: std::slice::from_ref(&cwd),
            user_home_dir: None,
            temporary_directories: None,
        };
        let candidate = path("file:///workspace/private/secret/key");

        for (case, writable_path, denied_path) in [
            (
                "encoded component",
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                path("file:///workspace/%70rivate").into(),
            ),
            (
                "encoded separator",
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                path("file:///workspace/private%2Fsecret").into(),
            ),
            (
                "repeated separators",
                path("file:///workspace////").into(),
                path("file:///workspace/private").into(),
            ),
        ] {
            let policy = FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry::new(writable_path, FileSystemAccessMode::Write),
                FileSystemSandboxEntry::new(denied_path, FileSystemAccessMode::Deny),
            ]);

            assert_eq!(
                policy.resolve_access(&candidate, &context),
                FileSystemAccessMode::Deny,
                "resolving {case}"
            );
        }
    }

    #[test]
    fn uri_matcher_fails_closed_without_executor_roots_or_lexical_paths() {
        let path = |path| PathUri::parse(path).expect("valid path URI");
        let cwd = path("file://server/share/workspace");
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd,
            workspace_roots: &[],
            user_home_dir: None,
            temporary_directories: None,
        };
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                FileSystemAccessMode::Write,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Tmpdir,
                },
                FileSystemAccessMode::Write,
            ),
        ]);

        assert_eq!(
            policy.resolve_access(&cwd, &context),
            FileSystemAccessMode::Deny
        );
        assert_eq!(
            policy.resolve_access(&path("file:///tmp/cache"), &context),
            FileSystemAccessMode::Deny
        );

        let opaque = path("file:///%00/bad/path/YQ");
        let opaque_context = FileSystemSandboxPolicyContext {
            cwd: &opaque,
            workspace_roots: std::slice::from_ref(&opaque),
            user_home_dir: None,
            temporary_directories: None,
        };
        let opaque_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            opaque.clone().into(),
            FileSystemAccessMode::Write,
        )]);
        assert_eq!(
            opaque_policy.resolve_access(&opaque, &opaque_context),
            FileSystemAccessMode::Deny
        );
    }

    #[test]
    fn uri_deny_and_metadata_matcher_use_executor_paths() {
        let path = |path| PathUri::parse(path).expect("valid path URI");
        let cwd = path("file:///C:/workspace");
        let roots = [cwd.clone()];
        let context = FileSystemSandboxPolicyContext {
            cwd: &cwd,
            workspace_roots: &roots,
            user_home_dir: None,
            temporary_directories: None,
        };
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(cwd.clone().into(), FileSystemAccessMode::Write),
            FileSystemSandboxEntry::new(path("file:///C:/").into(), FileSystemAccessMode::Deny),
            FileSystemSandboxEntry::new(
                path("file:///C:/workspace/.codex").into(),
                FileSystemAccessMode::Read,
            ),
            unreadable_glob_entry(r"C:\workspace\**\*.env".to_string()),
        ]);
        let prepared = policy
            .prepare_deny_read_matcher(&context, InvalidDenyReadGlobBehavior::ReturnError)
            .expect("remote deny matcher");

        assert!(prepared.denied_roots.is_empty());
        assert!(FileSystemSandboxPolicy::matches_prepared_read_deny(
            &path("file:///c:/WORKSPACE/app/.ENV"),
            &context,
            &prepared,
        ));
        for candidate in ["file:///%00/bad/path/YQ", "file:///C:/workspace/%2Fsecret"] {
            assert!(FileSystemSandboxPolicy::matches_prepared_read_deny(
                &path(candidate),
                &context,
                &prepared,
            ));
        }
        assert!(!policy.can_write_path(&path("file:///C:/workspace/.codex/config"), &context));
        assert_eq!(
            policy.metadata_write_denial(&path("file:///C:/workspace/.codex/config"), &context),
            Some(".codex"),
        );
    }

    #[test]
    fn uri_deny_globs_use_executor_home() {
        for (cwd, home, pattern, candidate) in [
            (
                "file:///workspace",
                "file:///home/executor",
                "~/private/*.env",
                "file:///home/executor/private/secret.env",
            ),
            (
                "file:///workspace",
                "file:///home/executor",
                "~//private/*.env",
                "file:///home/executor/private/secret.env",
            ),
            (
                "file:///C:/workspace",
                "file:///C:/Users/executor",
                r"~\private\*.env",
                "file:///C:/Users/executor/private/secret.env",
            ),
            (
                "file:///C:/workspace",
                "file:///C:/Users/executor",
                r"~\\private\*.env",
                "file:///C:/Users/executor/private/secret.env",
            ),
        ] {
            let cwd = PathUri::parse(cwd).expect("executor cwd");
            let home = PathUri::parse(home).expect("executor home");
            let candidate = PathUri::parse(candidate).expect("denied path");
            let context = FileSystemSandboxPolicyContext {
                cwd: &cwd,
                workspace_roots: std::slice::from_ref(&cwd),
                user_home_dir: Some(&home),
                temporary_directories: None,
            };
            let policy = FileSystemSandboxPolicy::restricted(vec![unreadable_glob_entry(
                pattern.to_string(),
            )]);
            let prepared = policy
                .prepare_deny_read_matcher(&context, InvalidDenyReadGlobBehavior::ReturnError)
                .expect("home-relative deny glob");

            assert!(FileSystemSandboxPolicy::matches_prepared_read_deny(
                &candidate, &context, &prepared,
            ));

            let without_home = FileSystemSandboxPolicyContext {
                user_home_dir: None,
                ..context
            };
            assert!(
                policy
                    .prepare_deny_read_matcher(
                        &without_home,
                        InvalidDenyReadGlobBehavior::ReturnError,
                    )
                    .is_err()
            );
            let prepared = policy
                .prepare_deny_read_matcher(&without_home, InvalidDenyReadGlobBehavior::FailClosed)
                .expect("missing executor home fails closed");
            assert!(prepared.invalid_pattern);
        }
    }

    #[test]
    fn unknown_special_paths_are_ignored_by_legacy_bridge() -> std::io::Result<()> {
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::unknown(
                        ":future_special_path",
                        /*subpath*/ None,
                    ),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
        ]);

        let sandbox_policy = policy.to_legacy_sandbox_policy(
            NetworkSandboxPolicy::Restricted,
            Path::new("/tmp/workspace"),
        )?;

        assert_eq!(
            sandbox_policy,
            SandboxPolicy::ReadOnly {
                network_access: false,
            }
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn workspace_write_ignores_slash_tmp_special_path_but_preserves_literal_path() {
        let cwd = TempDir::new().expect("tempdir");
        let slash_tmp = AbsolutePathBuf::from_absolute_path("/tmp").expect("absolute tmp path");
        let slash_tmp_only_policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::SlashTmp,
                },
                FileSystemAccessMode::Write,
            ),
        ]);
        assert_eq!(
            slash_tmp_only_policy
                .to_legacy_sandbox_policy(NetworkSandboxPolicy::Restricted, cwd.path())
                .expect("legacy sandbox policy"),
            SandboxPolicy::ReadOnly {
                network_access: false,
            }
        );
        assert!(
            !slash_tmp_only_policy
                .needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path())
        );

        for access in [FileSystemAccessMode::Read, FileSystemAccessMode::Deny] {
            let policy = FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    FileSystemAccessMode::Write,
                ),
                FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::SlashTmp,
                    },
                    access,
                ),
            ]);
            assert!(policy.has_full_disk_write_access());
            assert!(policy.has_full_disk_read_access());
        }

        let legacy_policy = SandboxPolicy::new_workspace_write_policy();
        assert_eq!(
            FileSystemSandboxPolicy::from(&legacy_policy)
                .to_legacy_sandbox_policy(NetworkSandboxPolicy::Restricted, cwd.path())
                .expect("legacy workspace-write policy"),
            legacy_policy
        );
        assert_eq!(
            FileSystemSandboxPolicy::from(&legacy_policy)
                .get_writable_roots_with_cwd(cwd.path())
                .into_iter()
                .map(|root| root.root)
                .collect::<Vec<_>>(),
            legacy_policy
                .get_writable_roots_with_cwd(cwd.path())
                .into_iter()
                .map(|root| normalize_effective_absolute_path(root.root))
                .collect::<Vec<_>>()
        );
        assert!(
            FileSystemSandboxPolicy::workspace_write(
                std::slice::from_ref(&slash_tmp),
                /*exclude_tmpdir_env_var*/ true,
                /*exclude_slash_tmp*/ false,
            )
            .can_write_local_path_with_cwd(slash_tmp.as_path(), cwd.path())
        );
    }

    #[cfg(unix)]
    #[test]
    fn writable_roots_proactively_protect_missing_dot_codex() {
        let cwd = TempDir::new().expect("tempdir");
        let expected_root = AbsolutePathBuf::from_absolute_path(
            cwd.path().canonicalize().expect("canonicalize cwd"),
        )
        .expect("absolute canonical root");
        let expected_dot_codex = expected_root.join(".codex");

        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        }]);

        let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
        assert_eq!(writable_roots.len(), 1);
        assert_eq!(writable_roots[0].root, expected_root);
        assert!(
            writable_roots[0]
                .read_only_subpaths
                .contains(&expected_dot_codex)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn preserving_mutable_paths_normalizes_top_level_aliases_consistently() {
        let root = TempDir::new_in("/tmp").expect("tempdir under /tmp");
        let logical_root =
            AbsolutePathBuf::from_absolute_path(root.path()).expect("absolute logical root");
        let canonical_root = AbsolutePathBuf::from_absolute_path(
            root.path().canonicalize().expect("canonicalize root"),
        )
        .expect("absolute canonical root");
        let protected = canonical_root.join("protected");
        fs::create_dir(&protected).expect("create protected path");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(logical_root.into(), FileSystemAccessMode::Write),
            FileSystemSandboxEntry::new(protected.clone().into(), FileSystemAccessMode::Read),
        ]);

        let roots = policy.get_writable_roots_with_cwd_preserving_mutable_paths(root.path());

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].root, canonical_root);
        assert!(roots[0].read_only_subpaths.contains(&protected));
    }

    #[cfg(unix)]
    #[test]
    fn preserving_writable_roots_cannot_be_rebound_during_projection() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;
        use std::sync::atomic::AtomicUsize;
        use std::sync::atomic::Ordering;
        use std::thread;

        let tmp = TempDir::new().expect("tempdir");
        let active_ancestor = tmp.path().join("active");
        let parked_ancestor = tmp.path().join("parked");
        let outside_ancestor = tmp.path().join("outside");
        let writable_root = active_ancestor.join("workspace");
        let outside_root = outside_ancestor.join("workspace");
        fs::create_dir_all(&writable_root).expect("create writable root");
        fs::create_dir_all(&outside_root).expect("create outside root");
        let writable_root =
            AbsolutePathBuf::from_absolute_path(writable_root).expect("absolute writable root");
        let outside_root =
            AbsolutePathBuf::from_absolute_path(outside_root).expect("absolute outside root");
        let expected_writable_root = normalize_trusted_top_level_alias(writable_root.clone());
        let expected_outside_root = normalize_trusted_top_level_alias(outside_root);
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            writable_root.into(),
            FileSystemAccessMode::Write,
        )]);

        let stop = Arc::new(AtomicBool::new(false));
        let swaps = Arc::new(AtomicUsize::new(0));
        let racer_stop = Arc::clone(&stop);
        let racer_swaps = Arc::clone(&swaps);
        let racer = thread::spawn(move || {
            while !racer_stop.load(Ordering::Relaxed) {
                if fs::rename(&active_ancestor, &parked_ancestor).is_err() {
                    thread::yield_now();
                    continue;
                }
                if symlink_dir(&outside_ancestor, &active_ancestor).is_ok() {
                    racer_swaps.fetch_add(1, Ordering::Relaxed);
                    thread::yield_now();
                    let _ = fs::remove_file(&active_ancestor);
                }
                fs::rename(&parked_ancestor, &active_ancestor).expect("restore writable ancestor");
            }
        });

        let mut rebound_root = None;
        for _ in 0..2_000 {
            let roots = policy.get_writable_roots_with_cwd_preserving_mutable_paths(tmp.path());
            if roots.len() != 1 || roots[0].root != expected_writable_root {
                rebound_root = roots.first().map(|root| root.root.clone());
                break;
            }
            assert_ne!(roots[0].root, expected_outside_root);
            thread::yield_now();
        }
        stop.store(true, Ordering::Relaxed);
        racer.join().expect("join path racer");

        assert!(swaps.load(Ordering::Relaxed) > 0, "racer did not run");
        assert_eq!(rebound_root, None);
    }

    #[test]
    fn legacy_workspace_write_projection_preserves_symbolic_project_root() {
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::new(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        assert_eq!(
            FileSystemSandboxPolicy::from(&policy),
            FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Read,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry::skip_missing_path(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::project_roots(Some(".git".into())),
                    },
                    FileSystemAccessMode::Read,
                ),
                FileSystemSandboxEntry::skip_missing_path(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::project_roots(Some(".agents".into())),
                    },
                    FileSystemAccessMode::Read,
                ),
                FileSystemSandboxEntry::skip_missing_path(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::project_roots(Some(".codex".into())),
                    },
                    FileSystemAccessMode::Read,
                ),
            ])
        );
    }

    #[test]
    fn legacy_current_working_directory_special_path_deserializes_as_project_roots()
    -> serde_json::Result<()> {
        let value = serde_json::json!({
            "kind": "current_working_directory",
        });

        let special_path = serde_json::from_value::<FileSystemSpecialPath>(value)?;
        assert_eq!(
            special_path,
            FileSystemSpecialPath::project_roots(/*subpath*/ None)
        );
        assert_eq!(
            serde_json::to_value(&special_path)?,
            serde_json::json!({
                "kind": "project_roots",
            })
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn writable_roots_skip_default_dot_codex_when_explicit_user_rule_exists() {
        let cwd = TempDir::new().expect("tempdir");
        let expected_root = AbsolutePathBuf::from_absolute_path(
            cwd.path().canonicalize().expect("canonicalize cwd"),
        )
        .expect("absolute canonical root");
        let explicit_dot_codex = expected_root.join(".codex");

        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: explicit_dot_codex.clone().into(),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
        ]);

        let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
        let workspace_root = writable_roots
            .iter()
            .find(|root| root.root == expected_root)
            .expect("workspace writable root");
        assert!(
            !workspace_root
                .protected_metadata_names
                .contains(&".codex".to_string()),
            "explicit .codex rule should remove the metadata-name protection"
        );
        assert!(
            !workspace_root
                .read_only_subpaths
                .contains(&explicit_dot_codex),
            "explicit .codex rule should win over the default protected carveout"
        );
        assert!(policy.can_write_local_path_with_cwd(
            explicit_dot_codex.join("config.toml").as_path(),
            cwd.path()
        ));
    }

    #[test]
    fn filesystem_policy_blocks_protected_metadata_path_writes_by_default() {
        let cwd = TempDir::new().expect("tempdir");
        let dot_git_config = cwd.path().join(".git").join("config");
        let dot_agents_config = cwd.path().join(".agents").join("config");
        let dot_codex_config = cwd.path().join(".codex").join("config.toml");
        let root = AbsolutePathBuf::from_absolute_path(cwd.path()).expect("absolute cwd");
        let file_system_policy =
            FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
                path: root.into(),
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            }]);

        assert!(!file_system_policy.can_write_local_path_with_cwd(&dot_git_config, cwd.path()));
        assert!(!file_system_policy.can_write_local_path_with_cwd(&dot_agents_config, cwd.path()));
        assert!(!file_system_policy.can_write_local_path_with_cwd(&dot_codex_config, cwd.path()));

        let writable_roots = file_system_policy.get_writable_roots_with_cwd(cwd.path());
        assert_eq!(writable_roots.len(), 1);
        assert_eq!(
            writable_roots[0].protected_metadata_names,
            vec![
                ".git".to_string(),
                ".agents".to_string(),
                ".codex".to_string(),
            ]
        );
        assert!(!writable_roots[0].is_path_writable(&dot_git_config));
        assert!(!writable_roots[0].is_path_writable(&dot_agents_config));
        assert!(!writable_roots[0].is_path_writable(&dot_codex_config));
    }

    #[test]
    fn legacy_workspace_write_projection_accepts_relative_cwd() {
        let relative_cwd = Path::new("workspace");
        let expected_root = AbsolutePathBuf::from_absolute_path(
            std::env::current_dir()
                .expect("current dir")
                .join(relative_cwd),
        )
        .expect("absolute root");
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let file_system_policy =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(&policy, relative_cwd);

        let mut expected_entries = vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
        ];
        expected_entries.extend(PROTECTED_METADATA_PATH_NAMES.iter().map(|name| {
            FileSystemSandboxEntry::skip_missing_path(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(Some((*name).into())),
                },
                FileSystemAccessMode::Read,
            )
        }));
        expected_entries.extend(
            default_read_only_subpaths_for_writable_root(
                &expected_root,
                /*protect_missing_dot_codex*/ true,
            )
            .into_iter()
            .map(|path| {
                FileSystemSandboxEntry::skip_missing_path(path.into(), FileSystemAccessMode::Read)
            }),
        );

        assert_eq!(
            file_system_policy,
            FileSystemSandboxPolicy::restricted(expected_entries)
        );
        assert_eq!(
            forbidden_agent_metadata_write(
                Path::new(".git/config"),
                relative_cwd,
                &file_system_policy,
            ),
            Some(".git")
        );
        assert!(
            file_system_policy
                .can_write_local_path_with_cwd(Path::new("src/main.rs"), relative_cwd,)
        );
        assert!(
            !file_system_policy
                .can_write_local_path_with_cwd(Path::new(".codex/config.toml"), relative_cwd,)
        );
        assert!(!file_system_policy.can_write_local_path_with_cwd(
            Path::new(".agents/skills/example/SKILL.md"),
            relative_cwd,
        ));
    }

    #[cfg(unix)]
    #[test]
    fn effective_runtime_roots_preserve_symlinked_paths() {
        let cwd = TempDir::new().expect("tempdir");
        let real_root = cwd.path().join("real");
        let link_root = cwd.path().join("link");
        let blocked = real_root.join("blocked");
        let codex_dir = real_root.join(".codex");

        fs::create_dir_all(&blocked).expect("create blocked");
        fs::create_dir_all(&codex_dir).expect("create .codex");
        symlink_dir(&real_root, &link_root).expect("create symlinked root");

        let link_root =
            AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
        let link_blocked = link_root.join("blocked");
        let expected_root = link_root.clone();
        let expected_blocked = link_blocked.clone();
        let expected_codex = link_root.join(".codex");

        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: link_root.into(),
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: link_blocked.into(),
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]);

        assert_eq!(
            policy.get_unreadable_roots_with_cwd(cwd.path()),
            vec![expected_blocked.clone()]
        );

        let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
        assert_eq!(writable_roots.len(), 1);
        assert_eq!(writable_roots[0].root, expected_root);
        assert!(
            writable_roots[0]
                .read_only_subpaths
                .contains(&expected_blocked)
        );
        assert!(
            writable_roots[0]
                .read_only_subpaths
                .contains(&expected_codex)
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_roots_special_path_preserves_symlinked_root() {
        let cwd = TempDir::new().expect("tempdir");
        let real_root = cwd.path().join("real");
        let link_root = cwd.path().join("link");
        let blocked = real_root.join("blocked");
        let agents_dir = real_root.join(".agents");
        let codex_dir = real_root.join(".codex");

        fs::create_dir_all(&blocked).expect("create blocked");
        fs::create_dir_all(&agents_dir).expect("create .agents");
        fs::create_dir_all(&codex_dir).expect("create .codex");
        symlink_dir(&real_root, &link_root).expect("create symlinked cwd");

        let link_blocked =
            AbsolutePathBuf::from_absolute_path(link_root.join("blocked")).expect("link blocked");
        let expected_root =
            AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
        let expected_blocked = link_blocked.clone();
        let expected_agents = expected_root.join(".agents");
        let expected_codex = expected_root.join(".codex");

        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Minimal,
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: link_blocked.into(),
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]);

        assert_eq!(
            policy.get_readable_roots_with_cwd(&link_root),
            vec![expected_root.clone()]
        );
        assert_eq!(
            policy.get_unreadable_roots_with_cwd(&link_root),
            vec![expected_blocked.clone()]
        );

        let writable_roots = policy.get_writable_roots_with_cwd(&link_root);
        assert_eq!(writable_roots.len(), 1);
        assert_eq!(writable_roots[0].root, expected_root);
        assert!(
            writable_roots[0]
                .read_only_subpaths
                .contains(&expected_blocked)
        );
        assert!(
            writable_roots[0]
                .read_only_subpaths
                .contains(&expected_agents)
        );
        assert!(
            writable_roots[0]
                .read_only_subpaths
                .contains(&expected_codex)
        );
    }

    #[cfg(unix)]
    #[test]
    fn writable_roots_preserve_symlinked_protected_subpaths() {
        let cwd = TempDir::new().expect("tempdir");
        let root = cwd.path().join("root");
        let decoy = root.join("decoy-codex");
        let dot_codex = root.join(".codex");
        fs::create_dir_all(&decoy).expect("create decoy");
        symlink_dir(&decoy, &dot_codex).expect("create .codex symlink");

        let root = AbsolutePathBuf::from_absolute_path(&root).expect("absolute root");
        let expected_dot_codex = AbsolutePathBuf::from_absolute_path(
            root.as_path()
                .canonicalize()
                .expect("canonicalize root")
                .join(".codex"),
        )
        .expect("absolute .codex symlink");
        let unexpected_decoy =
            AbsolutePathBuf::from_absolute_path(decoy.canonicalize().expect("canonicalize decoy"))
                .expect("absolute canonical decoy");

        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: root.into(),
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        }]);

        let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
        assert_eq!(writable_roots.len(), 1);
        assert_eq!(
            writable_roots[0].read_only_subpaths,
            vec![expected_dot_codex]
        );
        assert!(
            !writable_roots[0]
                .read_only_subpaths
                .contains(&unexpected_decoy)
        );
    }

    #[cfg(unix)]
    #[test]
    fn writable_roots_preserve_explicit_symlinked_carveouts_under_symlinked_roots() {
        let cwd = TempDir::new().expect("tempdir");
        let real_root = cwd.path().join("real");
        let link_root = cwd.path().join("link");
        let decoy = real_root.join("decoy-private");
        let linked_private = real_root.join("linked-private");
        fs::create_dir_all(&decoy).expect("create decoy");
        symlink_dir(&real_root, &link_root).expect("create symlinked root");
        symlink_dir(&decoy, &linked_private).expect("create linked-private symlink");

        let link_root =
            AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
        let link_private = link_root.join("linked-private");
        let expected_root = link_root.clone();
        let expected_linked_private = link_private.clone();
        let unexpected_decoy =
            AbsolutePathBuf::from_absolute_path(decoy.canonicalize().expect("canonicalize decoy"))
                .expect("absolute canonical decoy");

        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: link_root.into(),
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: link_private.into(),
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]);

        let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
        assert_eq!(writable_roots.len(), 1);
        assert_eq!(writable_roots[0].root, expected_root);
        assert_eq!(
            writable_roots[0].read_only_subpaths,
            vec![expected_linked_private]
        );
        assert!(
            !writable_roots[0]
                .read_only_subpaths
                .contains(&unexpected_decoy)
        );
    }

    #[cfg(unix)]
    #[test]
    fn writable_roots_preserve_explicit_symlinked_carveouts_that_escape_root() {
        let cwd = TempDir::new().expect("tempdir");
        let real_root = cwd.path().join("real");
        let link_root = cwd.path().join("link");
        let decoy = cwd.path().join("outside-private");
        let linked_private = real_root.join("linked-private");
        fs::create_dir_all(&decoy).expect("create decoy");
        fs::create_dir_all(&real_root).expect("create real root");
        symlink_dir(&real_root, &link_root).expect("create symlinked root");
        symlink_dir(&decoy, &linked_private).expect("create linked-private symlink");

        let link_root =
            AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
        let link_private = link_root.join("linked-private");
        let expected_root = link_root.clone();
        let expected_linked_private = link_private.clone();
        let unexpected_decoy =
            AbsolutePathBuf::from_absolute_path(decoy.canonicalize().expect("canonicalize decoy"))
                .expect("absolute canonical decoy");

        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: link_root.into(),
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: link_private.into(),
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]);

        let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
        assert_eq!(writable_roots.len(), 1);
        assert_eq!(writable_roots[0].root, expected_root);
        assert_eq!(
            writable_roots[0].read_only_subpaths,
            vec![expected_linked_private]
        );
        assert!(
            !writable_roots[0]
                .read_only_subpaths
                .contains(&unexpected_decoy)
        );
    }

    #[cfg(unix)]
    #[test]
    fn writable_roots_preserve_explicit_symlinked_carveouts_that_alias_root() {
        let cwd = TempDir::new().expect("tempdir");
        let root = cwd.path().join("root");
        let alias = root.join("alias-root");
        fs::create_dir_all(&root).expect("create root");
        symlink_dir(&root, &alias).expect("create alias symlink");

        let root = AbsolutePathBuf::from_absolute_path(&root).expect("absolute root");
        let alias = root.join("alias-root");
        let expected_root = AbsolutePathBuf::from_absolute_path(
            root.as_path().canonicalize().expect("canonicalize root"),
        )
        .expect("absolute canonical root");
        let expected_alias = expected_root.join("alias-root");

        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: root.into(),
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: alias.into(),
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]);

        let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
        assert_eq!(writable_roots.len(), 1);
        assert_eq!(writable_roots[0].root, expected_root);
        assert_eq!(writable_roots[0].read_only_subpaths, vec![expected_alias]);
    }

    #[cfg(unix)]
    #[test]
    fn tmpdir_special_path_preserves_symlinked_tmpdir() {
        if std::env::var_os(SYMLINKED_TMPDIR_TEST_ENV).is_none() {
            let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .env(SYMLINKED_TMPDIR_TEST_ENV, "1")
                .arg("--exact")
                .arg("permissions::tests::tmpdir_special_path_preserves_symlinked_tmpdir")
                .output()
                .expect("run tmpdir subprocess test");

            assert!(
                output.status.success(),
                "tmpdir subprocess test failed\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let cwd = TempDir::new().expect("tempdir");
        let real_tmpdir = cwd.path().join("real-tmpdir");
        let link_tmpdir = cwd.path().join("link-tmpdir");
        let blocked = real_tmpdir.join("blocked");
        let codex_dir = real_tmpdir.join(".codex");

        fs::create_dir_all(&blocked).expect("create blocked");
        fs::create_dir_all(&codex_dir).expect("create .codex");
        symlink_dir(&real_tmpdir, &link_tmpdir).expect("create symlinked tmpdir");

        let link_blocked =
            AbsolutePathBuf::from_absolute_path(link_tmpdir.join("blocked")).expect("link blocked");
        let expected_root =
            AbsolutePathBuf::from_absolute_path(&link_tmpdir).expect("absolute symlinked tmpdir");
        let expected_blocked = link_blocked.clone();
        let expected_codex = expected_root.join(".codex");

        unsafe {
            std::env::set_var("TMPDIR", &link_tmpdir);
        }

        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Tmpdir,
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: link_blocked.into(),
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]);

        assert_eq!(
            policy.get_unreadable_roots_with_cwd(cwd.path()),
            vec![expected_blocked.clone()]
        );

        let writable_roots = policy.get_writable_roots_with_cwd(cwd.path());
        assert_eq!(writable_roots.len(), 1);
        assert_eq!(writable_roots[0].root, expected_root);
        assert!(
            writable_roots[0]
                .read_only_subpaths
                .contains(&expected_blocked)
        );
        assert!(
            writable_roots[0]
                .read_only_subpaths
                .contains(&expected_codex)
        );
    }

    #[test]
    fn resolve_access_for_local_path_with_cwd_uses_most_specific_entry() {
        let cwd = TempDir::new().expect("tempdir");
        let docs = AbsolutePathBuf::resolve_path_against_base("docs", cwd.path());
        let docs_private = AbsolutePathBuf::resolve_path_against_base("docs/private", cwd.path());
        let docs_private_public =
            AbsolutePathBuf::resolve_path_against_base("docs/private/public", cwd.path());
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: docs.clone().into(),
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: docs_private.clone().into(),
                },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: docs_private_public.clone().into(),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
        ]);

        assert_eq!(
            policy.resolve_access_for_local_path_with_cwd(cwd.path(), cwd.path()),
            FileSystemAccessMode::Write
        );
        assert_eq!(
            policy.resolve_access_for_local_path_with_cwd(docs.as_path(), cwd.path()),
            FileSystemAccessMode::Read
        );
        assert_eq!(
            policy.resolve_access_for_local_path_with_cwd(docs_private.as_path(), cwd.path()),
            FileSystemAccessMode::Deny
        );
        assert_eq!(
            policy
                .resolve_access_for_local_path_with_cwd(docs_private_public.as_path(), cwd.path()),
            FileSystemAccessMode::Write
        );
    }

    #[test]
    fn split_only_nested_carveouts_need_direct_runtime_enforcement() {
        let cwd = TempDir::new().expect("tempdir");
        let docs = AbsolutePathBuf::resolve_path_against_base("docs", cwd.path());
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: docs.into(),
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
        ]);

        assert!(
            policy.needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path(),)
        );

        let legacy_workspace_write = legacy_runtime_file_system_policy_for_cwd(
            &SandboxPolicy::new_workspace_write_policy(),
            cwd.path(),
        );
        assert!(
            legacy_workspace_write
                .needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path(),),
            "metadata-name protections must stay in the direct enforcement path even when legacy concrete read-only paths match"
        );
    }

    #[test]
    fn legacy_projection_runtime_enforcement_ignores_entry_order() {
        let cwd = TempDir::new().expect("tempdir");
        let legacy_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::new(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };
        let legacy_order = legacy_runtime_file_system_policy_for_cwd(&legacy_policy, cwd.path());
        let mut reordered_entries = legacy_order.entries.clone();
        reordered_entries.reverse();
        let reordered = FileSystemSandboxPolicy::restricted(reordered_entries);

        assert!(
            legacy_order.is_semantically_equivalent_to(&reordered, cwd.path()),
            "entry order should not affect filesystem semantics"
        );
        assert_eq!(
            legacy_order
                .needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path()),
            reordered
                .needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path()),
            "entry order should not affect direct-enforcement classification"
        );
    }

    #[test]
    fn missing_symbolic_metadata_carveouts_need_direct_runtime_enforcement() {
        let cwd = TempDir::new().expect("tempdir");
        let legacy_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::new(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let profile_projection =
            FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(&legacy_policy, cwd.path());
        assert!(
            profile_projection
                .needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path()),
            "symbolic .git/.agents carveouts protect missing paths that legacy sandboxes cannot represent"
        );

        let legacy_runtime_projection =
            legacy_runtime_file_system_policy_for_cwd(&legacy_policy, cwd.path());
        assert!(
            legacy_runtime_projection
                .needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path()),
            "metadata-name protections are outside the legacy SandboxPolicy writable-root contract"
        );
    }

    #[test]
    fn root_write_with_read_only_child_is_not_full_disk_write() {
        let cwd = TempDir::new().expect("tempdir");
        let docs = AbsolutePathBuf::resolve_path_against_base("docs", cwd.path());
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: docs.clone().into(),
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
        ]);

        assert!(!policy.has_full_disk_write_access());
        assert_eq!(
            policy.resolve_access_for_local_path_with_cwd(docs.as_path(), cwd.path()),
            FileSystemAccessMode::Read
        );
        assert!(
            policy.needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, cwd.path(),)
        );
        assert!(
            policy
                .to_legacy_sandbox_policy(NetworkSandboxPolicy::Restricted, cwd.path())
                .is_err()
        );
    }

    #[test]
    fn root_deny_does_not_materialize_as_unreadable_root() {
        let cwd = TempDir::new().expect("tempdir");
        let docs = AbsolutePathBuf::resolve_path_against_base("docs", cwd.path());
        let expected_docs = AbsolutePathBuf::from_absolute_path(
            canonicalize_preserving_symlinks(cwd.path())
                .expect("canonicalize cwd")
                .join("docs"),
        )
        .expect("canonical docs");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: docs.clone().into(),
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
        ]);

        assert_eq!(
            policy.resolve_access_for_local_path_with_cwd(docs.as_path(), cwd.path()),
            FileSystemAccessMode::Read
        );
        assert_eq!(
            policy.get_readable_roots_with_cwd(cwd.path()),
            vec![expected_docs]
        );
        assert!(policy.get_unreadable_roots_with_cwd(cwd.path()).is_empty());
    }

    #[test]
    fn duplicate_root_deny_prevents_full_disk_write_access() {
        let cwd = TempDir::new().expect("tempdir");
        let root = AbsolutePathBuf::from_absolute_path(cwd.path())
            .map(|cwd| absolute_root_path_for_cwd(&cwd))
            .expect("resolve filesystem root");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]);

        assert!(!policy.has_full_disk_write_access());
        assert_eq!(
            policy.resolve_access_for_local_path_with_cwd(root.as_path(), cwd.path()),
            FileSystemAccessMode::Deny
        );
    }

    #[test]
    fn same_specificity_write_override_keeps_full_disk_write_access() {
        let cwd = TempDir::new().expect("tempdir");
        let docs = AbsolutePathBuf::resolve_path_against_base("docs", cwd.path());
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: docs.clone().into(),
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: docs.clone().into(),
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
        ]);

        assert!(policy.has_full_disk_write_access());
        assert_eq!(
            policy.resolve_access_for_local_path_with_cwd(docs.as_path(), cwd.path()),
            FileSystemAccessMode::Write
        );
    }

    #[test]
    fn with_additional_readable_roots_skips_existing_effective_access() {
        let cwd = TempDir::new().expect("tempdir");
        let cwd_root = AbsolutePathBuf::from_absolute_path(cwd.path()).expect("absolute cwd");
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        }]);

        let actual = policy
            .clone()
            .with_additional_readable_roots(cwd.path(), std::slice::from_ref(&cwd_root));

        assert_eq!(actual, policy);
    }

    #[test]
    fn with_additional_writable_roots_skips_existing_effective_access() {
        let cwd = TempDir::new().expect("tempdir");
        let cwd_root = AbsolutePathBuf::from_absolute_path(cwd.path()).expect("absolute cwd");
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        }]);

        let actual = policy
            .clone()
            .with_additional_writable_roots(cwd.path(), std::slice::from_ref(&cwd_root));

        assert_eq!(actual, policy);
    }

    #[test]
    fn with_additional_writable_roots_adds_new_root() {
        let temp_dir = TempDir::new().expect("tempdir");
        let cwd = temp_dir.path().join("workspace");
        let extra = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("extra"))
            .expect("resolve extra root");
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        }]);

        let actual = policy.with_additional_writable_roots(&cwd, std::slice::from_ref(&extra));

        assert_eq!(
            actual,
            FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: extra.into(),
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
            ])
        );
    }

    #[test]
    fn materialize_project_roots_with_workspace_roots_expands_exact_and_glob_entries() {
        let temp_dir = TempDir::new().expect("tempdir");
        let first = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("first"))
            .expect("resolve first root");
        let second = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("second"))
            .expect("resolve second root");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(Some(".git".into())),
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::GlobPattern {
                    pattern: project_roots_glob_pattern(Path::new("**/*.env")),
                },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]);

        let actual =
            policy.materialize_project_roots_with_workspace_roots(&[first.clone(), second.clone()]);

        assert_eq!(
            actual,
            FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: first.clone().into(),
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: second.clone().into(),
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: first.join(".git").into(),
                    },
                    access: FileSystemAccessMode::Read,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: second.join(".git").into(),
                    },
                    access: FileSystemAccessMode::Read,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::GlobPattern {
                        pattern: AbsolutePathBuf::resolve_path_against_base(
                            "**/*.env",
                            first.as_path(),
                        )
                        .to_string_lossy()
                        .into_owned(),
                    },
                    access: FileSystemAccessMode::Deny,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::GlobPattern {
                        pattern: AbsolutePathBuf::resolve_path_against_base(
                            "**/*.env",
                            second.as_path(),
                        )
                        .to_string_lossy()
                        .into_owned(),
                    },
                    access: FileSystemAccessMode::Deny,
                    missing_path_behavior: None,
                },
            ])
        );
    }

    #[test]
    fn with_additional_legacy_workspace_writable_roots_protects_metadata() {
        let temp_dir = TempDir::new().expect("tempdir");
        let extra = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("extra"))
            .expect("resolve extra root");
        std::fs::create_dir_all(extra.join(".git")).expect("create .git dir");
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        }]);

        let actual =
            policy.with_additional_legacy_workspace_writable_roots(std::slice::from_ref(&extra));

        assert_eq!(
            actual,
            FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: extra.clone().into()
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry::skip_missing_path(
                    FileSystemPath::Path {
                        path: extra.join(".git").into()
                    },
                    FileSystemAccessMode::Read,
                ),
            ])
        );
    }

    #[test]
    fn file_system_access_mode_orders_by_conflict_precedence() {
        assert!(FileSystemAccessMode::Write > FileSystemAccessMode::Read);
        assert!(FileSystemAccessMode::Deny > FileSystemAccessMode::Write);
    }

    #[test]
    fn legacy_bridge_preserves_explicit_deny_entries() {
        let denied = AbsolutePathBuf::try_from("/tmp/private").expect("absolute path");
        let existing = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: denied.clone().into(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        }]);

        let rebuilt = FileSystemSandboxPolicy::from_legacy_sandbox_policy_preserving_deny_entries(
            &SandboxPolicy::new_workspace_write_policy(),
            Path::new("/tmp/workspace"),
            &existing,
        );

        assert!(
            rebuilt.entries.iter().any(|entry| {
                entry.path
                    == FileSystemPath::Path {
                        path: denied.clone().into(),
                    }
                    && entry.access == FileSystemAccessMode::Deny
            }),
            "expected explicit deny entry to be preserved"
        );
    }

    #[test]
    fn preserving_deny_entries_keeps_unrestricted_policy_enforceable() {
        let deny_entry = unreadable_glob_entry("/tmp/project/**/*.env".to_string());
        let mut existing = FileSystemSandboxPolicy::restricted(vec![deny_entry.clone()]);
        existing.glob_scan_max_depth = Some(2);
        let mut replacement = FileSystemSandboxPolicy::unrestricted();

        replacement.preserve_deny_read_restrictions_from(&existing);

        let mut expected = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            deny_entry,
        ]);
        expected.glob_scan_max_depth = Some(2);
        assert_eq!(replacement, expected);
    }

    fn deny_policy(path: &Path) -> FileSystemSandboxPolicy {
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::try_from(path)
                    .expect("absolute deny path")
                    .into(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        }])
    }

    fn unreadable_glob_entry(pattern: String) -> FileSystemSandboxEntry {
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern { pattern },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        }
    }

    fn default_policy_with_unreadable_glob(pattern: String) -> FileSystemSandboxPolicy {
        let mut policy = FileSystemSandboxPolicy::default();
        policy.entries.push(unreadable_glob_entry(pattern));
        policy
    }

    fn is_read_denied(
        path: &Path,
        file_system_sandbox_policy: &FileSystemSandboxPolicy,
        cwd: &Path,
    ) -> bool {
        ReadDenyMatcher::try_new_for_local_paths(file_system_sandbox_policy, cwd)
            .expect("valid deny-read globs")
            .is_some_and(|matcher| matcher.is_local_path_read_denied(path))
    }

    #[test]
    fn exact_path_and_descendants_are_denied() {
        let temp = TempDir::new().expect("tempdir");
        let denied_dir = temp.path().join("denied");
        let nested = denied_dir.join("nested.txt");
        std::fs::create_dir_all(&denied_dir).expect("create denied dir");
        std::fs::write(&nested, "secret").expect("write secret");

        let policy = deny_policy(&denied_dir);
        assert!(is_read_denied(&denied_dir, &policy, temp.path()));
        assert!(is_read_denied(&nested, &policy, temp.path()));
        assert!(!is_read_denied(
            &temp.path().join("other.txt"),
            &policy,
            temp.path()
        ));
    }

    #[cfg(unix)]
    #[test]
    fn native_read_deny_matching_uses_uri_matcher_semantics() {
        let temp = TempDir::new().expect("tempdir");
        let real_dir = temp.path().join("real");
        let alias_dir = temp.path().join("alias");
        std::fs::create_dir_all(&real_dir).expect("create real dir");
        symlink_dir(&real_dir, &alias_dir).expect("symlink alias");

        let secret = real_dir.join("secret.txt");
        std::fs::write(&secret, "secret").expect("write secret");
        let alias_secret = alias_dir.join("secret.txt");

        for (denied_root, candidate) in [(&real_dir, &alias_secret), (&alias_dir, &secret)] {
            let policy = deny_policy(denied_root);
            let denied_root_uri =
                PathUri::from_host_native_path(denied_root).expect("deny root URI");
            let candidate_uri = PathUri::from_host_native_path(candidate).expect("candidate URI");

            assert_eq!(
                is_read_denied(candidate, &policy, temp.path()),
                candidate_uri.starts_with(&denied_root_uri)
            );
        }
    }

    #[test]
    fn literal_patterns_and_globs_are_denied() {
        let temp = TempDir::new().expect("tempdir");
        let literal = temp.path().join("private");
        let other = temp.path().join("notes.txt");
        std::fs::create_dir_all(&literal).expect("create literal dir");
        std::fs::write(&other, "notes").expect("write notes");

        let mut policy = deny_policy(&literal);
        policy.entries.push(unreadable_glob_entry(format!(
            "{}/**/*.txt",
            temp.path().display()
        )));

        assert!(is_read_denied(&literal, &policy, temp.path()));
        assert!(is_read_denied(&other, &policy, temp.path()));
    }

    #[test]
    fn glob_patterns_deny_matching_paths() {
        let temp = TempDir::new().expect("tempdir");
        let denied = temp.path().join("private").join("secret1.txt");
        std::fs::create_dir_all(denied.parent().expect("parent")).expect("create parent");
        std::fs::write(&denied, "secret").expect("write secret");

        let policy = default_policy_with_unreadable_glob(format!(
            "{}/private/secret?.txt",
            temp.path().display()
        ));

        assert!(is_read_denied(&denied, &policy, temp.path()));

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;

            let non_utf8 = denied
                .parent()
                .expect("parent")
                .join(OsStr::from_bytes(b"secret\xff.txt"));
            assert!(is_read_denied(&non_utf8, &policy, temp.path()));
        }
    }

    #[test]
    fn glob_patterns_do_not_cross_path_separators() {
        let temp = TempDir::new().expect("tempdir");
        let matching = temp.path().join("app").join("file42.txt");
        let nested = temp.path().join("app").join("nested").join("file42.txt");
        let short = temp.path().join("app").join("file4.txt");
        let letters = temp.path().join("app").join("fileab.txt");
        std::fs::create_dir_all(nested.parent().expect("parent")).expect("create parent");
        std::fs::write(&matching, "secret").expect("write matching");
        std::fs::write(&nested, "secret").expect("write nested");
        std::fs::write(&short, "secret").expect("write short");
        std::fs::write(&letters, "secret").expect("write letters");

        let policy = default_policy_with_unreadable_glob(format!(
            "{}/*/file[0-9]?.txt",
            temp.path().display()
        ));

        assert!(is_read_denied(&matching, &policy, temp.path()));
        assert!(!is_read_denied(&nested, &policy, temp.path()));
        assert!(!is_read_denied(&short, &policy, temp.path()));
        assert!(!is_read_denied(&letters, &policy, temp.path()));
    }

    #[test]
    fn globstar_patterns_deny_root_and_nested_matches() {
        let temp = TempDir::new().expect("tempdir");
        let root_env = temp.path().join(".env");
        let nested_env = temp.path().join("app").join(".env");
        let other = temp.path().join("app").join("notes.txt");
        std::fs::create_dir_all(nested_env.parent().expect("parent")).expect("create parent");
        std::fs::write(&root_env, "secret").expect("write root env");
        std::fs::write(&nested_env, "secret").expect("write nested env");
        std::fs::write(&other, "notes").expect("write notes");

        let policy =
            default_policy_with_unreadable_glob(format!("{}/**/*.env", temp.path().display()));

        assert!(is_read_denied(&root_env, &policy, temp.path()));
        assert!(is_read_denied(&nested_env, &policy, temp.path()));
        assert!(!is_read_denied(&other, &policy, temp.path()));
    }

    #[test]
    fn unclosed_character_classes_match_literal_brackets() {
        let temp = TempDir::new().expect("tempdir");
        let bracket_file = temp.path().join("[");
        let other = temp.path().join("notes.txt");
        std::fs::write(&bracket_file, "secret").expect("write bracket file");
        std::fs::write(&other, "notes").expect("write notes");
        let policy = default_policy_with_unreadable_glob(format!("{}/[", temp.path().display()));

        assert!(is_read_denied(&bracket_file, &policy, temp.path()));
        assert!(!is_read_denied(&other, &policy, temp.path()));
    }
}
