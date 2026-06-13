use std::io;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use codex_file_system::FileSystemSandboxContext;
use codex_protocol::config_types::WindowsSandboxLevel as CoreWindowsSandboxLevel;
use codex_protocol::models::ManagedFileSystemPermissions as CoreManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile as CorePermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode as CoreFileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath as CoreFileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry as CoreFileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath as CoreFileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy as CoreNetworkSandboxPolicy;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Serialize;

mod legacy;

/// Network sandbox policy carried by the exec-server protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecServerNetworkSandboxPolicy {
    #[default]
    Restricted,
    Enabled,
}

/// Windows sandbox level carried by the exec-server protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecServerWindowsSandboxLevel {
    #[default]
    Disabled,
    RestrictedToken,
    Elevated,
}

/// Access mode for an exec-server filesystem permission entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecServerFileSystemAccessMode {
    Read,
    Write,
    #[serde(alias = "none")]
    Deny,
}

/// Symbolic filesystem location carried by the exec-server protocol.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecServerFileSystemSpecialPath {
    Root,
    Minimal,
    #[serde(alias = "current_working_directory")]
    ProjectRoots {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath: Option<PathBuf>,
    },
    Tmpdir,
    SlashTmp,
    Unknown {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subpath: Option<PathBuf>,
    },
}

/// Filesystem location or pattern carried by the exec-server protocol.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecServerFileSystemPath {
    Path {
        path: PathUri,
    },
    GlobPattern {
        pattern: String,
    },
    Special {
        value: ExecServerFileSystemSpecialPath,
    },
}

/// Filesystem permission entry carried by the exec-server protocol.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecServerFileSystemSandboxEntry {
    pub path: ExecServerFileSystemPath,
    pub access: ExecServerFileSystemAccessMode,
}

/// Filesystem permissions for an exec-server managed sandbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecServerManagedFileSystemPermissions {
    #[serde(rename_all = "snake_case")]
    Restricted {
        entries: Vec<ExecServerFileSystemSandboxEntry>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        glob_scan_max_depth: Option<NonZeroUsize>,
    },
    Unrestricted,
}

/// Active sandbox permissions carried by the exec-server protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecServerPermissionProfile {
    #[serde(rename_all = "snake_case")]
    Managed {
        file_system: ExecServerManagedFileSystemPermissions,
        network: ExecServerNetworkSandboxPolicy,
    },
    Disabled,
    #[serde(rename_all = "snake_case")]
    External {
        network: ExecServerNetworkSandboxPolicy,
    },
}

/// Sandbox context carried by exec-server filesystem requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecServerFileSystemSandboxContext {
    pub permissions: ExecServerPermissionProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathUri>,
    pub windows_sandbox_level: ExecServerWindowsSandboxLevel,
    #[serde(default)]
    pub windows_sandbox_private_desktop: bool,
    #[serde(default)]
    pub use_legacy_landlock: bool,
}

impl From<CoreNetworkSandboxPolicy> for ExecServerNetworkSandboxPolicy {
    fn from(value: CoreNetworkSandboxPolicy) -> Self {
        match value {
            CoreNetworkSandboxPolicy::Restricted => Self::Restricted,
            CoreNetworkSandboxPolicy::Enabled => Self::Enabled,
        }
    }
}

impl From<ExecServerNetworkSandboxPolicy> for CoreNetworkSandboxPolicy {
    fn from(value: ExecServerNetworkSandboxPolicy) -> Self {
        match value {
            ExecServerNetworkSandboxPolicy::Restricted => Self::Restricted,
            ExecServerNetworkSandboxPolicy::Enabled => Self::Enabled,
        }
    }
}

impl From<CoreWindowsSandboxLevel> for ExecServerWindowsSandboxLevel {
    fn from(value: CoreWindowsSandboxLevel) -> Self {
        match value {
            CoreWindowsSandboxLevel::Disabled => Self::Disabled,
            CoreWindowsSandboxLevel::RestrictedToken => Self::RestrictedToken,
            CoreWindowsSandboxLevel::Elevated => Self::Elevated,
        }
    }
}

impl From<ExecServerWindowsSandboxLevel> for CoreWindowsSandboxLevel {
    fn from(value: ExecServerWindowsSandboxLevel) -> Self {
        match value {
            ExecServerWindowsSandboxLevel::Disabled => Self::Disabled,
            ExecServerWindowsSandboxLevel::RestrictedToken => Self::RestrictedToken,
            ExecServerWindowsSandboxLevel::Elevated => Self::Elevated,
        }
    }
}

impl From<CoreFileSystemAccessMode> for ExecServerFileSystemAccessMode {
    fn from(value: CoreFileSystemAccessMode) -> Self {
        match value {
            CoreFileSystemAccessMode::Read => Self::Read,
            CoreFileSystemAccessMode::Write => Self::Write,
            CoreFileSystemAccessMode::Deny => Self::Deny,
        }
    }
}

impl From<ExecServerFileSystemAccessMode> for CoreFileSystemAccessMode {
    fn from(value: ExecServerFileSystemAccessMode) -> Self {
        match value {
            ExecServerFileSystemAccessMode::Read => Self::Read,
            ExecServerFileSystemAccessMode::Write => Self::Write,
            ExecServerFileSystemAccessMode::Deny => Self::Deny,
        }
    }
}

impl From<CoreFileSystemSpecialPath> for ExecServerFileSystemSpecialPath {
    fn from(value: CoreFileSystemSpecialPath) -> Self {
        match value {
            CoreFileSystemSpecialPath::Root => Self::Root,
            CoreFileSystemSpecialPath::Minimal => Self::Minimal,
            CoreFileSystemSpecialPath::ProjectRoots { subpath } => Self::ProjectRoots { subpath },
            CoreFileSystemSpecialPath::Tmpdir => Self::Tmpdir,
            CoreFileSystemSpecialPath::SlashTmp => Self::SlashTmp,
            CoreFileSystemSpecialPath::Unknown { path, subpath } => Self::Unknown { path, subpath },
        }
    }
}

impl From<ExecServerFileSystemSpecialPath> for CoreFileSystemSpecialPath {
    fn from(value: ExecServerFileSystemSpecialPath) -> Self {
        match value {
            ExecServerFileSystemSpecialPath::Root => Self::Root,
            ExecServerFileSystemSpecialPath::Minimal => Self::Minimal,
            ExecServerFileSystemSpecialPath::ProjectRoots { subpath } => {
                Self::ProjectRoots { subpath }
            }
            ExecServerFileSystemSpecialPath::Tmpdir => Self::Tmpdir,
            ExecServerFileSystemSpecialPath::SlashTmp => Self::SlashTmp,
            ExecServerFileSystemSpecialPath::Unknown { path, subpath } => {
                Self::Unknown { path, subpath }
            }
        }
    }
}

impl From<CoreFileSystemPath> for ExecServerFileSystemPath {
    fn from(value: CoreFileSystemPath) -> Self {
        match value {
            CoreFileSystemPath::Path { path } => Self::Path {
                path: PathUri::from_abs_path(&path),
            },
            CoreFileSystemPath::GlobPattern { pattern } => Self::GlobPattern { pattern },
            CoreFileSystemPath::Special { value } => Self::Special {
                value: value.into(),
            },
        }
    }
}

impl TryFrom<ExecServerFileSystemPath> for CoreFileSystemPath {
    type Error = io::Error;

    fn try_from(value: ExecServerFileSystemPath) -> Result<Self, Self::Error> {
        match value {
            ExecServerFileSystemPath::Path { path } => {
                let native_path = path.to_abs_path().map_err(|err| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "sandbox permission path URI `{path}` is not valid on this exec-server host: {err}"
                        ),
                    )
                })?;
                Ok(Self::Path { path: native_path })
            }
            ExecServerFileSystemPath::GlobPattern { pattern } => Ok(Self::GlobPattern { pattern }),
            ExecServerFileSystemPath::Special { value } => Ok(Self::Special {
                value: value.into(),
            }),
        }
    }
}

impl From<CoreFileSystemSandboxEntry> for ExecServerFileSystemSandboxEntry {
    fn from(value: CoreFileSystemSandboxEntry) -> Self {
        Self {
            path: value.path.into(),
            access: value.access.into(),
        }
    }
}

impl TryFrom<ExecServerFileSystemSandboxEntry> for CoreFileSystemSandboxEntry {
    type Error = io::Error;

    fn try_from(value: ExecServerFileSystemSandboxEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            path: value.path.try_into()?,
            access: value.access.into(),
        })
    }
}

impl From<CoreManagedFileSystemPermissions> for ExecServerManagedFileSystemPermissions {
    fn from(value: CoreManagedFileSystemPermissions) -> Self {
        match value {
            CoreManagedFileSystemPermissions::Restricted {
                entries,
                glob_scan_max_depth,
            } => Self::Restricted {
                entries: entries.into_iter().map(Into::into).collect(),
                glob_scan_max_depth,
            },
            CoreManagedFileSystemPermissions::Unrestricted => Self::Unrestricted,
        }
    }
}

impl TryFrom<ExecServerManagedFileSystemPermissions> for CoreManagedFileSystemPermissions {
    type Error = io::Error;

    fn try_from(value: ExecServerManagedFileSystemPermissions) -> Result<Self, Self::Error> {
        match value {
            ExecServerManagedFileSystemPermissions::Restricted {
                entries,
                glob_scan_max_depth,
            } => Ok(Self::Restricted {
                entries: entries
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<_, _>>()?,
                glob_scan_max_depth,
            }),
            ExecServerManagedFileSystemPermissions::Unrestricted => Ok(Self::Unrestricted),
        }
    }
}

impl From<CorePermissionProfile> for ExecServerPermissionProfile {
    fn from(value: CorePermissionProfile) -> Self {
        match value {
            CorePermissionProfile::Managed {
                file_system,
                network,
            } => Self::Managed {
                file_system: file_system.into(),
                network: network.into(),
            },
            CorePermissionProfile::Disabled => Self::Disabled,
            CorePermissionProfile::External { network } => Self::External {
                network: network.into(),
            },
        }
    }
}

impl TryFrom<ExecServerPermissionProfile> for CorePermissionProfile {
    type Error = io::Error;

    fn try_from(value: ExecServerPermissionProfile) -> Result<Self, Self::Error> {
        match value {
            ExecServerPermissionProfile::Managed {
                file_system,
                network,
            } => Ok(Self::Managed {
                file_system: file_system.try_into()?,
                network: network.into(),
            }),
            ExecServerPermissionProfile::Disabled => Ok(Self::Disabled),
            ExecServerPermissionProfile::External { network } => Ok(Self::External {
                network: network.into(),
            }),
        }
    }
}

impl From<FileSystemSandboxContext> for ExecServerFileSystemSandboxContext {
    fn from(value: FileSystemSandboxContext) -> Self {
        Self {
            permissions: value.permissions.into(),
            cwd: value.cwd,
            windows_sandbox_level: value.windows_sandbox_level.into(),
            windows_sandbox_private_desktop: value.windows_sandbox_private_desktop,
            use_legacy_landlock: value.use_legacy_landlock,
        }
    }
}

impl TryFrom<ExecServerFileSystemSandboxContext> for FileSystemSandboxContext {
    type Error = io::Error;

    fn try_from(value: ExecServerFileSystemSandboxContext) -> Result<Self, Self::Error> {
        Ok(Self {
            permissions: value.permissions.try_into()?,
            cwd: value.cwd,
            windows_sandbox_level: value.windows_sandbox_level.into(),
            windows_sandbox_private_desktop: value.windows_sandbox_private_desktop,
            use_legacy_landlock: value.use_legacy_landlock,
        })
    }
}

#[cfg(test)]
#[path = "sandbox_permissions_tests.rs"]
mod tests;
