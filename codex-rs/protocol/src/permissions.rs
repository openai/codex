use std::collections::HashSet;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use tracing::error;
use ts_rs::TS;

use crate::protocol::NetworkAccess;
use crate::protocol::ReadOnlyAccess;
use crate::protocol::SandboxPolicy;
use crate::protocol::WritableRoot;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum FileSystemAccessMode {
    None,
    Read,
    Write,
}

impl FileSystemAccessMode {
    pub fn can_read(self) -> bool {
        !matches!(self, FileSystemAccessMode::None)
    }

    pub fn can_write(self) -> bool {
        matches!(self, FileSystemAccessMode::Write)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind")]
pub enum FileSystemSpecialPath {
    Root,
    Minimal,
    CurrentWorkingDirectory,
    ProjectRoots {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        subpath: Option<PathBuf>,
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
        subpath: Option<PathBuf>,
    },
}

impl FileSystemSpecialPath {
    pub fn project_roots(subpath: Option<PathBuf>) -> Self {
        Self::ProjectRoots { subpath }
    }

    pub fn unknown(path: impl Into<String>, subpath: Option<PathBuf>) -> Self {
        Self::Unknown {
            path: path.into(),
            subpath,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FileSystemSandboxEntry {
    pub path: FileSystemPath,
    pub access: FileSystemAccessMode,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FileSystemSandboxPolicy {
    pub kind: FileSystemSandboxKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<FileSystemSandboxEntry>,
    /// Additional deny-only read patterns that cannot be expressed as
    /// concrete filesystem entries, such as macOS glob-based deny rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny_read_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type")]
pub enum FileSystemPath {
    Path { path: AbsolutePathBuf },
    Special { value: FileSystemSpecialPath },
}

impl Default for FileSystemSandboxPolicy {
    fn default() -> Self {
        Self {
            kind: FileSystemSandboxKind::Restricted,
            entries: vec![FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            }],
            deny_read_patterns: Vec::new(),
        }
    }
}

impl FileSystemSandboxPolicy {
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

    fn has_explicit_deny_entries(&self) -> bool {
        matches!(self.kind, FileSystemSandboxKind::Restricted)
            && self
                .entries
                .iter()
                .any(|entry| entry.access == FileSystemAccessMode::None)
    }

    pub fn unrestricted() -> Self {
        Self {
            kind: FileSystemSandboxKind::Unrestricted,
            entries: Vec::new(),
            deny_read_patterns: Vec::new(),
        }
    }

    pub fn external_sandbox() -> Self {
        Self {
            kind: FileSystemSandboxKind::ExternalSandbox,
            entries: Vec::new(),
            deny_read_patterns: Vec::new(),
        }
    }

    pub fn restricted(entries: Vec<FileSystemSandboxEntry>) -> Self {
        Self {
            kind: FileSystemSandboxKind::Restricted,
            entries,
            deny_read_patterns: Vec::new(),
        }
    }

    pub fn deny_read_patterns(&self) -> &[String] {
        &self.deny_read_patterns
    }

    pub fn has_denied_read_patterns(&self) -> bool {
        !self.deny_read_patterns.is_empty()
    }

    pub fn has_denied_read_restrictions(&self) -> bool {
        self.has_explicit_deny_entries() || self.has_denied_read_patterns()
    }

    pub fn from_legacy_sandbox_policy_preserving_read_denies(
        sandbox_policy: &SandboxPolicy,
        cwd: &Path,
        existing: &Self,
    ) -> Self {
        let mut rebuilt = Self::from_legacy_sandbox_policy(sandbox_policy, cwd);
        if !matches!(rebuilt.kind, FileSystemSandboxKind::Restricted) {
            return rebuilt;
        }

        for deny_entry in existing
            .entries
            .iter()
            .filter(|entry| entry.access == FileSystemAccessMode::None)
        {
            if !rebuilt.entries.iter().any(|entry| entry == deny_entry) {
                rebuilt.entries.push(deny_entry.clone());
            }
        }

        let mut seen_patterns = HashSet::new();
        rebuilt
            .deny_read_patterns
            .extend(existing.deny_read_patterns.iter().cloned());
        rebuilt
            .deny_read_patterns
            .retain(|pattern| seen_patterns.insert(pattern.clone()));

        rebuilt
    }

    /// Converts a legacy sandbox policy into an equivalent filesystem policy
    /// for the provided cwd.
    ///
    /// Legacy `WorkspaceWrite` policies may list readable roots that live
    /// under an already-writable root. Those paths were redundant in the
    /// legacy model and should not become read-only carveouts when projected
    /// into split filesystem policy.
    pub fn from_legacy_sandbox_policy(sandbox_policy: &SandboxPolicy, cwd: &Path) -> Self {
        let mut file_system_policy = Self::from(sandbox_policy);
        if matches!(sandbox_policy, SandboxPolicy::WorkspaceWrite { .. }) {
            let legacy_writable_roots = sandbox_policy.get_writable_roots_with_cwd(cwd);
            file_system_policy.entries.retain(|entry| {
                if entry.access != FileSystemAccessMode::Read {
                    return true;
                }

                match &entry.path {
                    FileSystemPath::Path { path } => !legacy_writable_roots
                        .iter()
                        .any(|root| root.is_path_writable(path.as_path())),
                    FileSystemPath::Special { .. } => true,
                }
            });
        }

        file_system_policy
    }

    /// Returns true when filesystem reads are unrestricted.
    pub fn has_full_disk_read_access(&self) -> bool {
        match self.kind {
            FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => true,
            FileSystemSandboxKind::Restricted => {
                self.has_root_access(FileSystemAccessMode::can_read)
                    && !self.has_explicit_deny_entries()
            }
        }
    }

    /// Returns true when filesystem writes are unrestricted.
    pub fn has_full_disk_write_access(&self) -> bool {
        match self.kind {
            FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => true,
            FileSystemSandboxKind::Restricted => {
                self.has_root_access(FileSystemAccessMode::can_write)
                    && !self.has_explicit_deny_entries()
            }
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

    /// Returns the explicit readable roots resolved against the provided cwd.
    pub fn get_readable_roots_with_cwd(&self, cwd: &Path) -> Vec<AbsolutePathBuf> {
        if self.has_full_disk_read_access() {
            return Vec::new();
        }

        let cwd_absolute = AbsolutePathBuf::from_absolute_path(cwd).ok();
        let mut readable_roots = Vec::new();
        if self.has_root_access(FileSystemAccessMode::can_read)
            && let Some(cwd_absolute) = cwd_absolute.as_ref()
        {
            readable_roots.push(absolute_root_path_for_cwd(cwd_absolute));
        }

        dedup_absolute_paths(
            readable_roots
                .into_iter()
                .chain(
                    self.entries
                        .iter()
                        .filter(|entry| entry.access.can_read())
                        .filter_map(|entry| {
                            resolve_file_system_path(&entry.path, cwd_absolute.as_ref())
                        }),
                )
                .collect(),
        )
    }

    /// Returns the writable roots together with read-only carveouts resolved
    /// against the provided cwd.
    pub fn get_writable_roots_with_cwd(&self, cwd: &Path) -> Vec<WritableRoot> {
        if self.has_full_disk_write_access() {
            return Vec::new();
        }

        let cwd_absolute = AbsolutePathBuf::from_absolute_path(cwd).ok();
        let read_only_roots = dedup_absolute_paths(
            self.entries
                .iter()
                .filter(|entry| !entry.access.can_write())
                .filter_map(|entry| resolve_file_system_path(&entry.path, cwd_absolute.as_ref()))
                .collect(),
        );
        let mut writable_roots = Vec::new();
        if self.has_root_access(FileSystemAccessMode::can_write)
            && let Some(cwd_absolute) = cwd_absolute.as_ref()
        {
            writable_roots.push(absolute_root_path_for_cwd(cwd_absolute));
        }

        dedup_absolute_paths(
            writable_roots
                .into_iter()
                .chain(
                    self.entries
                        .iter()
                        .filter(|entry| entry.access.can_write())
                        .filter_map(|entry| {
                            resolve_file_system_path(&entry.path, cwd_absolute.as_ref())
                        }),
                )
                .collect(),
        )
        .into_iter()
        .map(|root| {
            let mut read_only_subpaths = default_read_only_subpaths_for_writable_root(&root);
            // Narrower explicit non-write entries carve out broader writable roots.
            // More specific write entries still remain writable because they appear
            // as separate WritableRoot values and are checked independently.
            read_only_subpaths.extend(
                read_only_roots
                    .iter()
                    .filter(|path| path.as_path() != root.as_path())
                    .filter(|path| path.as_path().starts_with(root.as_path()))
                    .cloned(),
            );
            WritableRoot {
                root,
                read_only_subpaths: dedup_absolute_paths(read_only_subpaths),
            }
        })
        .collect()
    }

    /// Returns explicit unreadable roots resolved against the provided cwd.
    pub fn get_unreadable_roots_with_cwd(&self, cwd: &Path) -> Vec<AbsolutePathBuf> {
        if !matches!(self.kind, FileSystemSandboxKind::Restricted) {
            return Vec::new();
        }

        let cwd_absolute = AbsolutePathBuf::from_absolute_path(cwd).ok();
        dedup_absolute_paths(
            self.entries
                .iter()
                .filter(|entry| entry.access == FileSystemAccessMode::None)
                .filter_map(|entry| resolve_file_system_path(&entry.path, cwd_absolute.as_ref()))
                .collect(),
        )
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
                let mut include_platform_defaults = false;
                let mut has_full_disk_read_access = false;
                let mut has_full_disk_write_access = false;
                let mut workspace_root_writable = false;
                let mut writable_roots = Vec::new();
                let mut readable_roots = Vec::new();
                let mut tmpdir_writable = false;
                let mut slash_tmp_writable = false;

                for entry in &self.entries {
                    match &entry.path {
                        FileSystemPath::Path { path } => {
                            if entry.access.can_write() {
                                if cwd_absolute.as_ref().is_some_and(|cwd| cwd == path) {
                                    workspace_root_writable = true;
                                } else {
                                    writable_roots.push(path.clone());
                                }
                            } else if entry.access.can_read() {
                                readable_roots.push(path.clone());
                            }
                        }
                        FileSystemPath::Special { value } => match value {
                            FileSystemSpecialPath::Root => match entry.access {
                                FileSystemAccessMode::None => {}
                                FileSystemAccessMode::Read => has_full_disk_read_access = true,
                                FileSystemAccessMode::Write => {
                                    has_full_disk_read_access = true;
                                    has_full_disk_write_access = true;
                                }
                            },
                            FileSystemSpecialPath::Minimal => {
                                if entry.access.can_read() {
                                    include_platform_defaults = true;
                                }
                            }
                            FileSystemSpecialPath::CurrentWorkingDirectory => {
                                if entry.access.can_write() {
                                    workspace_root_writable = true;
                                } else if entry.access.can_read()
                                    && let Some(path) = resolve_file_system_special_path(
                                        value,
                                        cwd_absolute.as_ref(),
                                    )
                                {
                                    readable_roots.push(path);
                                }
                            }
                            FileSystemSpecialPath::ProjectRoots { subpath } => {
                                if subpath.is_none() && entry.access.can_write() {
                                    workspace_root_writable = true;
                                } else if let Some(path) =
                                    resolve_file_system_special_path(value, cwd_absolute.as_ref())
                                {
                                    if entry.access.can_write() {
                                        writable_roots.push(path);
                                    } else if entry.access.can_read() {
                                        readable_roots.push(path);
                                    }
                                }
                            }
                            FileSystemSpecialPath::Tmpdir => {
                                if entry.access.can_write() {
                                    tmpdir_writable = true;
                                } else if entry.access.can_read()
                                    && let Some(path) = resolve_file_system_special_path(
                                        value,
                                        cwd_absolute.as_ref(),
                                    )
                                {
                                    readable_roots.push(path);
                                }
                            }
                            FileSystemSpecialPath::SlashTmp => {
                                if entry.access.can_write() {
                                    slash_tmp_writable = true;
                                } else if entry.access.can_read()
                                    && let Some(path) = resolve_file_system_special_path(
                                        value,
                                        cwd_absolute.as_ref(),
                                    )
                                {
                                    readable_roots.push(path);
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

                let read_only_access = if has_full_disk_read_access {
                    ReadOnlyAccess::FullAccess
                } else {
                    ReadOnlyAccess::Restricted {
                        include_platform_defaults,
                        readable_roots: dedup_absolute_paths(readable_roots),
                    }
                };

                if workspace_root_writable {
                    SandboxPolicy::WorkspaceWrite {
                        writable_roots: dedup_absolute_paths(writable_roots),
                        read_only_access,
                        network_access: network_policy.is_enabled(),
                        exclude_tmpdir_env_var: !tmpdir_writable,
                        exclude_slash_tmp: !slash_tmp_writable,
                    }
                } else if !writable_roots.is_empty() || tmpdir_writable || slash_tmp_writable {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "permissions profile requests filesystem writes outside the workspace root, which is not supported until the runtime enforces FileSystemSandboxPolicy directly",
                    ));
                } else {
                    SandboxPolicy::ReadOnly {
                        access: read_only_access,
                        network_access: network_policy.is_enabled(),
                    }
                }
            }
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
            SandboxPolicy::ReadOnly { access, .. } => {
                let mut entries = Vec::new();
                match access {
                    ReadOnlyAccess::FullAccess => entries.push(FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Read,
                    }),
                    ReadOnlyAccess::Restricted {
                        include_platform_defaults,
                        readable_roots,
                    } => {
                        entries.push(FileSystemSandboxEntry {
                            path: FileSystemPath::Special {
                                value: FileSystemSpecialPath::CurrentWorkingDirectory,
                            },
                            access: FileSystemAccessMode::Read,
                        });
                        if *include_platform_defaults {
                            entries.push(FileSystemSandboxEntry {
                                path: FileSystemPath::Special {
                                    value: FileSystemSpecialPath::Minimal,
                                },
                                access: FileSystemAccessMode::Read,
                            });
                        }
                        entries.extend(readable_roots.iter().cloned().map(|path| {
                            FileSystemSandboxEntry {
                                path: FileSystemPath::Path { path },
                                access: FileSystemAccessMode::Read,
                            }
                        }));
                    }
                }
                FileSystemSandboxPolicy::restricted(entries)
            }
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                read_only_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
                ..
            } => {
                let mut entries = Vec::new();
                match read_only_access {
                    ReadOnlyAccess::FullAccess => entries.push(FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Read,
                    }),
                    ReadOnlyAccess::Restricted {
                        include_platform_defaults,
                        readable_roots,
                    } => {
                        if *include_platform_defaults {
                            entries.push(FileSystemSandboxEntry {
                                path: FileSystemPath::Special {
                                    value: FileSystemSpecialPath::Minimal,
                                },
                                access: FileSystemAccessMode::Read,
                            });
                        }
                        entries.extend(readable_roots.iter().cloned().map(|path| {
                            FileSystemSandboxEntry {
                                path: FileSystemPath::Path { path },
                                access: FileSystemAccessMode::Read,
                            }
                        }));
                    }
                }

                entries.push(FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::CurrentWorkingDirectory,
                    },
                    access: FileSystemAccessMode::Write,
                });
                if !exclude_slash_tmp {
                    entries.push(FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::SlashTmp,
                        },
                        access: FileSystemAccessMode::Write,
                    });
                }
                if !exclude_tmpdir_env_var {
                    entries.push(FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Tmpdir,
                        },
                        access: FileSystemAccessMode::Write,
                    });
                }
                entries.extend(
                    writable_roots
                        .iter()
                        .cloned()
                        .map(|path| FileSystemSandboxEntry {
                            path: FileSystemPath::Path { path },
                            access: FileSystemAccessMode::Write,
                        }),
                );
                FileSystemSandboxPolicy::restricted(entries)
            }
        }
    }
}

fn resolve_file_system_path(
    path: &FileSystemPath,
    cwd: Option<&AbsolutePathBuf>,
) -> Option<AbsolutePathBuf> {
    match path {
        FileSystemPath::Path { path } => Some(path.clone()),
        FileSystemPath::Special { value } => resolve_file_system_special_path(value, cwd),
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

fn resolve_file_system_special_path(
    value: &FileSystemSpecialPath,
    cwd: Option<&AbsolutePathBuf>,
) -> Option<AbsolutePathBuf> {
    match value {
        FileSystemSpecialPath::Root
        | FileSystemSpecialPath::Minimal
        | FileSystemSpecialPath::Unknown { .. } => None,
        FileSystemSpecialPath::CurrentWorkingDirectory => {
            let cwd = cwd?;
            Some(cwd.clone())
        }
        FileSystemSpecialPath::ProjectRoots { subpath } => {
            let cwd = cwd?;
            match subpath.as_ref() {
                Some(subpath) => {
                    AbsolutePathBuf::resolve_path_against_base(subpath, cwd.as_path()).ok()
                }
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
            #[allow(clippy::expect_used)]
            let slash_tmp = AbsolutePathBuf::from_absolute_path("/tmp").expect("/tmp is absolute");
            if !slash_tmp.as_path().is_dir() {
                return None;
            }
            Some(slash_tmp)
        }
    }
}

fn dedup_absolute_paths(paths: Vec<AbsolutePathBuf>) -> Vec<AbsolutePathBuf> {
    let mut deduped = Vec::with_capacity(paths.len());
    let mut seen = HashSet::new();
    for path in paths {
        if seen.insert(path.to_path_buf()) {
            deduped.push(path);
        }
    }
    deduped
}

fn default_read_only_subpaths_for_writable_root(
    writable_root: &AbsolutePathBuf,
) -> Vec<AbsolutePathBuf> {
    let mut subpaths: Vec<AbsolutePathBuf> = Vec::new();
    #[allow(clippy::expect_used)]
    let top_level_git = writable_root
        .join(".git")
        .expect(".git is a valid relative path");
    // This applies to typical repos (directory .git), worktrees/submodules
    // (file .git with gitdir pointer), and bare repos when the gitdir is the
    // writable root itself.
    let top_level_git_is_file = top_level_git.as_path().is_file();
    let top_level_git_is_dir = top_level_git.as_path().is_dir();
    if top_level_git_is_dir || top_level_git_is_file {
        if top_level_git_is_file
            && is_git_pointer_file(&top_level_git)
            && let Some(gitdir) = resolve_gitdir_from_file(&top_level_git)
        {
            subpaths.push(gitdir);
        }
        subpaths.push(top_level_git);
    }

    // Make .agents/skills and .codex/config.toml and related files read-only
    // to the agent, by default.
    for subdir in &[".agents", ".codex"] {
        #[allow(clippy::expect_used)]
        let top_level_codex = writable_root.join(subdir).expect("valid relative path");
        if top_level_codex.as_path().is_dir() {
            subpaths.push(top_level_codex);
        }
    }

    dedup_absolute_paths(subpaths)
}

fn is_git_pointer_file(path: &AbsolutePathBuf) -> bool {
    path.as_path().is_file() && path.as_path().file_name() == Some(OsStr::new(".git"))
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
        Some(parts) => parts,
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
    let gitdir_path = match AbsolutePathBuf::resolve_path_against_base(gitdir_raw, base) {
        Ok(path) => path,
        Err(err) => {
            error!(
                "Failed to resolve gitdir path {gitdir_raw} from {path}: {err}",
                path = dot_git.as_path().display()
            );
            return None;
        }
    };
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

    #[test]
    fn unknown_special_paths_are_ignored_by_legacy_bridge() -> std::io::Result<()> {
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::unknown(":future_special_path", None),
            },
            access: FileSystemAccessMode::Write,
        }]);

        let sandbox_policy = policy.to_legacy_sandbox_policy(
            NetworkSandboxPolicy::Restricted,
            Path::new("/tmp/workspace"),
        )?;

        assert_eq!(
            sandbox_policy,
            SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: Vec::new(),
                },
                network_access: false,
            }
        );
        Ok(())
    }
}
