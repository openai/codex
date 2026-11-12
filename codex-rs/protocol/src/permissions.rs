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

mod deny_read_validator;
mod enforcement;
mod local_aliases;
mod target;
mod windows_glob;

use local_aliases::LocalPolicyContext;

pub use deny_read_validator::DenyReadValidator;
pub use deny_read_validator::DenyReadViolation;
pub use enforcement::FileSystemPolicyProjection;
pub use enforcement::ResolvedFileSystemPermissions;
pub use local_aliases::LocalFileSystemPolicyMatcher;
pub use windows_glob::WindowsDenyReadGlobScan;
pub use windows_glob::windows_deny_read_glob_scan;

/// Selects native execution projection or an unresolved, lossless transport template.
#[derive(Clone, Copy)]
pub(crate) enum LegacySandboxPolicyProjection<'a> {
    Cwd(&'a Path),
    Template,
}

const PROTECTED_METADATA_GIT_PATH_NAME: &str = ".git";
const PROTECTED_METADATA_AGENTS_PATH_NAME: &str = ".agents";
const PROTECTED_METADATA_CODEX_PATH_NAME: &str = ".codex";
const PROTECTED_METADATA_AWS_PATH_NAME: &str = ".aws";

/// Top-level workspace metadata paths that stay protected under writable roots.
pub const PROTECTED_METADATA_PATH_NAMES: &[&str] = &[
    PROTECTED_METADATA_GIT_PATH_NAME,
    PROTECTED_METADATA_AGENTS_PATH_NAME,
    PROTECTED_METADATA_CODEX_PATH_NAME,
    PROTECTED_METADATA_AWS_PATH_NAME,
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

    /// Projects an explicit writable path for the legacy native-path sandbox adapter.
    /// Special paths and glob patterns are handled separately by that adapter.
    pub fn legacy_writable_path(&self) -> io::Result<Option<AbsolutePathBuf>> {
        match &self.path {
            FileSystemPath::Path { path } if self.access.can_write() => {
                path.to_abs_path().map(Some)
            }
            FileSystemPath::Path { .. }
            | FileSystemPath::Special { .. }
            | FileSystemPath::GlobPattern { .. } => Ok(None),
        }
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
        // TODO(anp) Migrate callers to select the executor's path convention explicitly.
        self.has_denied_read_restrictions_for_convention(Some(PathConvention::native()))
    }

    /// Returns whether this policy denies reads on the selected executor.
    fn has_denied_read_restrictions_for_convention(
        &self,
        convention: Option<PathConvention>,
    ) -> bool {
        matches!(self.kind, FileSystemSandboxKind::Restricted)
            && self.entries.iter().any(|entry| {
                entry.access == FileSystemAccessMode::Deny
                    && !matches!(
                        &entry.path,
                        FileSystemPath::Special {
                            value: FileSystemSpecialPath::SlashTmp,
                        } if convention == Some(PathConvention::Windows)
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
        let mut policy = Self::workspace_write_template(
            writable_roots,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        );
        for writable_root in writable_roots {
            for protected_path in default_read_only_subpaths_for_writable_root(
                writable_root,
                /*protect_missing_dot_codex*/ false,
            ) {
                append_default_read_only_path_if_no_explicit_rule(
                    &mut policy.entries,
                    protected_path,
                );
            }
        }

        policy
    }

    /// Builds an unresolved template; admission must bind native workspace metadata.
    pub(crate) fn workspace_write_template(
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
        append_default_read_only_project_root_subpath_if_no_explicit_rule(&mut entries, ".aws");
        Self::restricted(entries)
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

    /// Returns true when filesystem reads are unrestricted on this host.
    pub fn has_full_disk_read_access(&self) -> bool {
        // TODO(anp) Migrate callers to select the executor's path convention explicitly.
        self.has_full_disk_read_access_for_convention(Some(PathConvention::native()))
    }

    /// Returns true when filesystem reads are unrestricted on the selected executor.
    /// If the convention is unknown, a `:slash_tmp` denial is still treated as a restriction.
    pub fn has_full_disk_read_access_for_convention(
        &self,
        convention: Option<PathConvention>,
    ) -> bool {
        match self.kind {
            FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => true,
            FileSystemSandboxKind::Restricted => {
                self.has_root_access(FileSystemAccessMode::can_read)
                    && !self.has_denied_read_restrictions_for_convention(convention)
            }
        }
    }

    /// Returns true when filesystem writes are unrestricted on this host.
    pub fn has_full_disk_write_access(&self) -> bool {
        // TODO(anp) Migrate callers to select the executor's path convention explicitly.
        self.has_full_disk_write_access_for_convention(Some(PathConvention::native()))
    }

    /// Returns true when filesystem writes are unrestricted for the selected executor.
    pub fn has_full_disk_write_access_with_context(
        &self,
        context: &FileSystemSandboxPolicyContext<'_>,
    ) -> bool {
        self.has_full_disk_write_access_for_convention(context.cwd.infer_path_convention())
    }

    /// Returns true when filesystem writes are unrestricted for the selected path convention.
    pub fn has_full_disk_write_access_for_convention(
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
            self.prepare_local_matching(context)
                .map(|matching| matching.resolve_access(path))
                .unwrap_or(FileSystemAccessMode::Deny)
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
            self.prepare_local_matching(context)
                .and_then(|matching| matching.can_write_path(path))
                .unwrap_or(false)
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
                let convention = if is_windows {
                    PathConvention::Windows
                } else {
                    PathConvention::Posix
                };
                if LegacyAppPathString::from_string(pattern)
                    .to_path_uri(convention)
                    .is_err()
                    && let Err(error) = root.validate_glob_directory(convention)
                {
                    return Some(Err(error.to_string()));
                }
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
        self,
        workspace_roots: &[AbsolutePathBuf],
    ) -> Self {
        let roots = workspace_roots
            .iter()
            .map(PathUri::from_abs_path)
            .collect::<Vec<_>>();
        self.materialize_project_roots_with_path_uris(&roots)
    }

    /// Materializes workspace-root entries without projecting executor paths onto the host.
    /// Legacy home-relative workspace denials clear all grants when their target is unknown.
    pub fn materialize_project_roots_with_path_uris(self, workspace_roots: &[PathUri]) -> Self {
        self.try_materialize_project_roots_with_path_uris(workspace_roots)
            .unwrap_or_else(|| Self::restricted(Vec::new()))
    }

    fn try_materialize_project_roots_with_path_uris(
        mut self,
        workspace_roots: &[PathUri],
    ) -> Option<Self> {
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
            if entry.access == FileSystemAccessMode::Deny
                && subpath.is_some_and(|subpath| {
                    workspace_roots.iter().any(|root| {
                        root.infer_path_convention().is_some_and(|convention| {
                            convention.home_relative_suffix(subpath).is_some()
                        })
                    })
                })
            {
                // Older policies could expand these rules outside the workspace.
                // Without a home fact, denying only the workspace would weaken them.
                return None;
            }
            entries.extend(workspace_roots.iter().filter_map(|root| {
                let path = if is_glob
                    && root
                        .infer_path_convention()
                        .is_none_or(|convention| root.validate_glob_directory(convention).is_err())
                {
                    // An infallible materializer cannot return an unsafe glob.
                    // Use the existing deny-root fallback for unresolvable rules.
                    None
                } else {
                    subpath.map_or_else(
                        || Some(root.clone()),
                        |subpath| resolve_scoped_workspace_path(root, subpath),
                    )
                };
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
        Some(self)
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

        !self.is_semantically_equivalent_to(
            &legacy_runtime_file_system_policy_for_cwd(&legacy_policy, cwd),
            cwd,
        )
    }

    /// Returns true when two policies resolve to the same filesystem access
    /// model for `cwd`, ignoring incidental entry ordering.
    pub fn is_semantically_equivalent_to(&self, other: &Self, cwd: &Path) -> bool {
        let Ok(signature) = self.semantic_signature(cwd) else {
            return false;
        };
        other
            .semantic_signature(cwd)
            .is_ok_and(|other| signature == other)
    }

    /// Returns the explicit readable roots resolved against the provided cwd.
    pub fn get_readable_roots_with_cwd(&self, cwd: &Path) -> Vec<AbsolutePathBuf> {
        if self.has_full_disk_read_access() {
            return Vec::new();
        }
        let Some(local_context) = LocalPolicyContext::new(cwd) else {
            return Vec::new();
        };
        let Ok(matching) = self.prepare_local_matching(&local_context.as_context()) else {
            return Vec::new();
        };

        Self::readable_roots(&self.resolved_entries_with_cwd(cwd), &matching)
    }

    fn readable_roots(
        entries: &[ResolvedFileSystemEntry],
        matching: &LocalFileSystemPolicyMatcher<'_>,
    ) -> Vec<AbsolutePathBuf> {
        dedup_absolute_paths(
            entries
                .iter()
                .filter(|entry| entry.access.can_read())
                .filter(|entry| {
                    matching
                        .resolve_access(&PathUri::from_abs_path(&entry.path))
                        .can_read()
                })
                .map(|entry| entry.path.clone())
                .collect(),
            /*normalize_effective_paths*/ true,
        )
    }

    /// Returns the writable roots together with read-only carveouts resolved
    /// against the provided cwd.
    pub fn get_writable_roots_with_cwd(&self, cwd: &Path) -> Vec<WritableRoot> {
        self.get_writable_roots_with_cwd_impl(cwd, FileSystemPolicyProjection::Native)
    }

    /// Omits redundant root-metadata mounts when the filesystem root is writable.
    ///
    /// Explicit root metadata writes still disable their default protection. Linux
    /// inherits those writes from its root bind so a metadata symlink cannot cause
    /// a second bind to reopen its target under an explicitly denied directory.
    pub fn get_writable_roots_with_cwd_inheriting_root_metadata(
        &self,
        cwd: &Path,
    ) -> Vec<WritableRoot> {
        self.get_writable_roots_with_cwd_impl(cwd, FileSystemPolicyProjection::LinuxMounts)
    }

    /// Reports full-disk writes when every explicit writable path can be projected locally.
    /// Preserves the legacy fallback used by startup checks and diagnostic labels,
    /// using this host's path convention.
    pub fn has_legacy_full_disk_write_access(&self) -> bool {
        self.has_full_disk_write_access()
            && self
                .entries
                .iter()
                .all(|entry| entry.legacy_writable_path().is_ok())
    }

    /// Reports entry write intent recognized by the legacy adapter and its diagnostic labels.
    /// This does not inspect path existence or apply read/deny precedence: missing and
    /// unrenderable writable paths still count, while unknown special paths and globs do not.
    pub fn has_legacy_write_intent(&self, cwd: &Path) -> bool {
        self.entries.iter().any(|entry| {
            entry.access.can_write()
                && match &entry.path {
                    FileSystemPath::Path { .. } => true,
                    FileSystemPath::GlobPattern { .. } => false,
                    FileSystemPath::Special { value } => match value {
                        FileSystemSpecialPath::Root
                        | FileSystemSpecialPath::Tmpdir
                        | FileSystemSpecialPath::ProjectRoots { subpath: None } => true,
                        FileSystemSpecialPath::ProjectRoots { subpath: Some(_) } => {
                            AbsolutePathBuf::from_absolute_path(cwd).is_ok()
                        }
                        FileSystemSpecialPath::SlashTmp => cfg!(unix),
                        FileSystemSpecialPath::Minimal | FileSystemSpecialPath::Unknown { .. } => {
                            false
                        }
                    },
                }
        })
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
        self.get_writable_roots_with_cwd_impl(cwd, FileSystemPolicyProjection::PreserveMutableRoots)
    }

    fn get_writable_roots_with_cwd_impl(
        &self,
        cwd: &Path,
        projection: FileSystemPolicyProjection,
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
        let Some(local_context) = LocalPolicyContext::new(cwd) else {
            return Vec::new();
        };
        let Ok(matching) = self.prepare_local_matching(&local_context.as_context()) else {
            return Vec::new();
        };
        Self::writable_roots(cwd, &resolved_entries, &matching, projection)
    }

    fn writable_roots(
        cwd: &Path,
        resolved_entries: &[ResolvedFileSystemEntry],
        matching: &LocalFileSystemPolicyMatcher<'_>,
        projection: FileSystemPolicyProjection,
    ) -> Vec<WritableRoot> {
        // Resolve precedence and filesystem aliases once per entry, rather than repeating
        // that work for every writable root while collecting its read-only carveouts.
        let effective_entries: Vec<&ResolvedFileSystemEntry> = resolved_entries
            .iter()
            .filter(|entry| {
                entry.access.can_write()
                    == matching
                        .can_write_path(&PathUri::from_abs_path(&entry.path))
                        .unwrap_or(false)
            })
            .collect();
        if !effective_entries
            .iter()
            .any(|entry| entry.access.can_write())
        {
            return Vec::new();
        }
        // Include resolved gitdirs in the entries used to carve out broader grants.
        // Seatbelt grants are independent, and later bubblewrap binds cover earlier mounts.
        let include_resolved_gitdirs = match projection {
            FileSystemPolicyProjection::Native | FileSystemPolicyProjection::LinuxMounts => {
                cfg!(target_os = "linux")
            }
            FileSystemPolicyProjection::PreserveMutableRoots => cfg!(target_os = "macos"),
        };
        let resolved_gitdir_entries: Vec<ResolvedFileSystemEntry> = if include_resolved_gitdirs {
            effective_entries
                .iter()
                .filter(|entry| entry.access.can_write())
                .filter_map(|entry| {
                    let dot_git = projection
                        .resolve_writable_path(entry.path.clone())
                        .join(PROTECTED_METADATA_GIT_PATH_NAME);
                    is_git_pointer_file(&dot_git)
                        .then(|| resolve_gitdir_from_file(&dot_git))
                        .flatten()
                })
                .filter(|path| !has_explicit_resolved_path_entry(resolved_entries, path))
                .map(|path| ResolvedFileSystemEntry {
                    path,
                    access: FileSystemAccessMode::Read,
                })
                .collect()
        } else {
            Vec::new()
        };
        let prepared_entries: Vec<PreparedFileSystemEntry<'_>> = effective_entries
            .into_iter()
            .map(|entry| PreparedFileSystemEntry {
                entry,
                effective_path: projection.resolve_writable_path(entry.path.clone()),
            })
            .chain(resolved_gitdir_entries.iter().map(|entry| {
                PreparedFileSystemEntry {
                    entry,
                    // Resolve the exclusion target while retaining the logical entry path.
                    effective_path: entry
                        .path
                        .as_path()
                        .canonicalize()
                        .ok()
                        .and_then(|path| AbsolutePathBuf::from_absolute_path(path).ok())
                        .unwrap_or_else(|| normalize_effective_absolute_path(entry.path.clone())),
                }
            }))
            .collect();
        let inherits_root_metadata = projection == FileSystemPolicyProjection::LinuxMounts
            && prepared_entries.iter().any(|entry| {
                entry.entry.access.can_write() && entry.entry.path.as_path().parent().is_none()
            });
        let writable_entries: Vec<&PreparedFileSystemEntry<'_>> = prepared_entries
            .iter()
            .filter(|entry| entry.entry.access.can_write())
            .filter(|entry| {
                let path = entry.entry.path.as_path();
                !inherits_root_metadata
                    || path.parent().is_none_or(|parent| parent.parent().is_some())
                    || !path.file_name().is_some_and(is_protected_metadata_name)
            })
            .collect();

        let effective_cwd = AbsolutePathBuf::from_absolute_path(cwd)
            .ok()
            .map(|cwd| projection.resolve_writable_path(cwd));

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
                protected_metadata_names_for_writable_root(matching, &root, &raw_writable_roots);
            let protect_missing_dot_codex = effective_cwd.as_ref() == Some(&root);
            let mut read_only_subpaths: Vec<AbsolutePathBuf> =
                default_read_only_subpaths_for_writable_root(&root, protect_missing_dot_codex)
                    .into_iter()
                    .filter(|path| !has_explicit_resolved_path_entry(resolved_entries, path))
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
        self.get_unreadable_roots_with_cwd_impl(cwd, FileSystemPolicyProjection::Native)
    }

    /// Includes literal denies alongside their resolved targets so Linux can reject a denial
    /// through a writable symlink instead of only masking the symlink's current target.
    pub fn get_unreadable_roots_with_cwd_preserving_symlinks(
        &self,
        cwd: &Path,
    ) -> Vec<AbsolutePathBuf> {
        self.get_unreadable_roots_with_cwd_impl(cwd, FileSystemPolicyProjection::LinuxMounts)
    }

    fn get_unreadable_roots_with_cwd_impl(
        &self,
        cwd: &Path,
        projection: FileSystemPolicyProjection,
    ) -> Vec<AbsolutePathBuf> {
        if !matches!(self.kind, FileSystemSandboxKind::Restricted) {
            return Vec::new();
        }

        let local_context = LocalPolicyContext::new(cwd);
        let matching = local_context
            .as_ref()
            .and_then(|context| self.prepare_local_matching(&context.as_context()).ok());
        Self::unreadable_roots(
            cwd,
            &self.resolved_entries_with_cwd(cwd),
            matching.as_ref(),
            projection,
        )
    }

    fn unreadable_roots(
        cwd: &Path,
        entries: &[ResolvedFileSystemEntry],
        matching: Option<&LocalFileSystemPolicyMatcher<'_>>,
        projection: FileSystemPolicyProjection,
    ) -> Vec<AbsolutePathBuf> {
        let root = AbsolutePathBuf::from_absolute_path(cwd)
            .ok()
            .map(|cwd| absolute_root_path_for_cwd(&cwd));
        let denied: Vec<_> = entries
            .iter()
            .filter(|entry| entry.access == FileSystemAccessMode::Deny)
            .filter(|entry| {
                !matching.is_some_and(|matching| {
                    matching
                        .resolve_access(&PathUri::from_abs_path(&entry.path))
                        .can_read()
                })
            })
            // A root denial is the default, not a mask that should hide narrower readable grants.
            .filter(|entry| root.as_ref() != Some(&entry.path))
            .map(|entry| entry.path.clone())
            .collect();
        if projection != FileSystemPolicyProjection::LinuxMounts {
            return dedup_absolute_paths(denied, /*normalize_effective_paths*/ true);
        }
        // A mount must protect both the configured name and the target it resolves to now.
        let mut roots =
            dedup_absolute_paths(denied.clone(), /*normalize_effective_paths*/ true);
        for path in denied {
            if path.as_path().parent().is_some() && !roots.contains(&path) {
                roots.push(path);
            }
        }
        roots
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

    /// Replace deny globs with a backend's concrete snapshot, preserving other entries.
    /// An empty snapshot still removes the original patterns.
    pub fn with_expanded_deny_globs(mut self, paths: Vec<AbsolutePathBuf>) -> Self {
        self.entries
            .retain(|entry| !matches!(entry.path, FileSystemPath::GlobPattern { .. }));
        self.entries.extend(
            paths
                .into_iter()
                .map(|path| FileSystemSandboxEntry::new(path.into(), FileSystemAccessMode::Deny)),
        );
        self
    }

    pub fn to_legacy_sandbox_policy(
        &self,
        network_policy: NetworkSandboxPolicy,
        cwd: &Path,
    ) -> io::Result<SandboxPolicy> {
        self.to_legacy_sandbox_policy_with_projection(
            network_policy,
            LegacySandboxPolicyProjection::Cwd(cwd),
        )
    }

    pub(crate) fn to_legacy_sandbox_policy_with_projection(
        &self,
        network_policy: NetworkSandboxPolicy,
        projection: LegacySandboxPolicyProjection<'_>,
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
                let cwd = match projection {
                    LegacySandboxPolicyProjection::Cwd(cwd) => Some(cwd),
                    LegacySandboxPolicyProjection::Template => None,
                };
                let cwd_absolute =
                    cwd.and_then(|cwd| AbsolutePathBuf::from_absolute_path(cwd).ok());
                let has_full_disk_write_access = self.has_full_disk_write_access();
                let mut workspace_root_writable = false;
                let mut writable_roots = Vec::new();
                let mut tmpdir_writable = false;
                let mut slash_tmp_writable = false;

                for entry in &self.entries {
                    match &entry.path {
                        FileSystemPath::GlobPattern { .. } => {}
                        FileSystemPath::Path { .. } => {
                            if let Some(path) = entry.legacy_writable_path()? {
                                if cwd_absolute.as_ref().is_some_and(|cwd| cwd == &path) {
                                    workspace_root_writable = true;
                                } else {
                                    writable_roots.push(path);
                                }
                            }
                        }
                        FileSystemPath::Special { value } => match value {
                            FileSystemSpecialPath::Root | FileSystemSpecialPath::Minimal => {}
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
                        writable_roots: match projection {
                            LegacySandboxPolicyProjection::Cwd(_) => dedup_absolute_paths(
                                writable_roots,
                                /*normalize_effective_paths*/ false,
                            ),
                            LegacySandboxPolicyProjection::Template => writable_roots,
                        },
                        network_access: network_policy.is_enabled(),
                        exclude_tmpdir_env_var: !tmpdir_writable,
                        exclude_slash_tmp: !slash_tmp_writable,
                    }
                } else if cwd.is_some_and(|cwd| self.has_legacy_write_intent(cwd)) {
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

    /// Resolves configured roots using executor paths without inspecting the filesystem.
    /// Glob patterns are excluded; access precedence is evaluated by `resolve_access`.
    pub fn resolved_entries(
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

    fn semantic_signature(&self, cwd: &Path) -> io::Result<ResolvedFileSystemPermissions> {
        let resolved = self.resolve_for_enforcement(cwd, FileSystemPolicyProjection::Native)?;
        Ok(ResolvedFileSystemPermissions {
            readable_roots: sorted_absolute_paths(resolved.readable_roots),
            writable_roots: sorted_writable_roots(resolved.writable_roots),
            unreadable_roots: sorted_absolute_paths(resolved.unreadable_roots),
            // Compatibility compares access rules, independently of the backend's scan limit.
            glob_scan_max_depth: None,
            ..resolved
        })
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
            SandboxPolicy::ReadOnly { .. } => FileSystemSandboxPolicy::read_only(),
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
    let context = LocalPolicyContext::new(cwd.as_path())?;
    Some(evaluate(&path, &context.as_context()))
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

    // AWS profiles can select credential helpers that the application executes.
    let top_level_aws = writable_root.join(PROTECTED_METADATA_AWS_PATH_NAME);
    if top_level_aws.as_path().is_dir() {
        subpaths.push(top_level_aws);
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
    matching: &LocalFileSystemPolicyMatcher<'_>,
    root: &AbsolutePathBuf,
    raw_writable_roots: &[&AbsolutePathBuf],
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
            !matching
                .can_write_path(&PathUri::from_abs_path(metadata_path))
                .unwrap_or(false)
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
#[path = "permissions_tests.rs"]
mod tests;
