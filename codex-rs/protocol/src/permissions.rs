use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use ts_rs::TS;

use crate::protocol::NetworkAccess;
use crate::protocol::ReadOnlyAccess;
use crate::protocol::SandboxPolicy;

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
}

impl FileSystemSpecialPath {
    pub fn project_roots(subpath: Option<PathBuf>) -> Self {
        Self::ProjectRoots { subpath }
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
        }
    }
}

impl FileSystemSandboxPolicy {
    pub fn unrestricted() -> Self {
        Self {
            kind: FileSystemSandboxKind::Unrestricted,
            entries: Vec::new(),
        }
    }

    pub fn external_sandbox() -> Self {
        Self {
            kind: FileSystemSandboxKind::ExternalSandbox,
            entries: Vec::new(),
        }
    }

    pub fn restricted(entries: Vec<FileSystemSandboxEntry>) -> Self {
        Self {
            kind: FileSystemSandboxKind::Restricted,
            entries,
        }
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

fn resolve_file_system_special_path(
    value: &FileSystemSpecialPath,
    cwd: Option<&AbsolutePathBuf>,
) -> Option<AbsolutePathBuf> {
    match value {
        FileSystemSpecialPath::Root | FileSystemSpecialPath::Minimal => None,
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
