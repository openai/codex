use std::num::NonZeroUsize;

use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Deserializer;

use super::ExecServerFileSystemAccessMode;
use super::ExecServerFileSystemPath;
use super::ExecServerFileSystemSandboxEntry;
use super::ExecServerManagedFileSystemPermissions;
use super::ExecServerNetworkSandboxPolicy;
use super::ExecServerPermissionProfile;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TaggedExecServerPermissionProfile {
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

impl From<TaggedExecServerPermissionProfile> for ExecServerPermissionProfile {
    fn from(value: TaggedExecServerPermissionProfile) -> Self {
        match value {
            TaggedExecServerPermissionProfile::Managed {
                file_system,
                network,
            } => Self::Managed {
                file_system,
                network,
            },
            TaggedExecServerPermissionProfile::Disabled => Self::Disabled,
            TaggedExecServerPermissionProfile::External { network } => Self::External { network },
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyExecServerPermissionProfile {
    network: Option<LegacyNetworkPermissions>,
    file_system: Option<LegacyFileSystemPermissions>,
}

impl From<LegacyExecServerPermissionProfile> for ExecServerPermissionProfile {
    fn from(value: LegacyExecServerPermissionProfile) -> Self {
        let file_system = value.file_system.unwrap_or_default();
        let network = if value
            .network
            .and_then(|network| network.enabled)
            .unwrap_or(false)
        {
            ExecServerNetworkSandboxPolicy::Enabled
        } else {
            ExecServerNetworkSandboxPolicy::Restricted
        };
        Self::Managed {
            file_system: ExecServerManagedFileSystemPermissions::Restricted {
                entries: file_system.entries,
                glob_scan_max_depth: file_system.glob_scan_max_depth,
            },
            network,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LegacyNetworkPermissions {
    enabled: Option<bool>,
}

#[derive(Debug, Clone, Default)]
struct LegacyFileSystemPermissions {
    entries: Vec<ExecServerFileSystemSandboxEntry>,
    glob_scan_max_depth: Option<NonZeroUsize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct CanonicalLegacyFileSystemPermissions {
    #[serde(default)]
    entries: Vec<ExecServerFileSystemSandboxEntry>,
    #[serde(default)]
    glob_scan_max_depth: Option<NonZeroUsize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadWriteLegacyFileSystemPermissions {
    #[serde(default)]
    read: Option<Vec<PathUri>>,
    #[serde(default)]
    write: Option<Vec<PathUri>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum LegacyFileSystemPermissionsDe {
    Canonical(CanonicalLegacyFileSystemPermissions),
    ReadWrite(ReadWriteLegacyFileSystemPermissions),
}

impl<'de> Deserialize<'de> for LegacyFileSystemPermissions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            match LegacyFileSystemPermissionsDe::deserialize(deserializer)? {
                LegacyFileSystemPermissionsDe::Canonical(value) => Self {
                    entries: value.entries,
                    glob_scan_max_depth: value.glob_scan_max_depth,
                },
                LegacyFileSystemPermissionsDe::ReadWrite(value) => {
                    let mut entries = Vec::new();
                    if let Some(paths) = value.read {
                        entries.extend(paths.into_iter().map(|path| {
                            ExecServerFileSystemSandboxEntry {
                                path: ExecServerFileSystemPath::Path { path },
                                access: ExecServerFileSystemAccessMode::Read,
                            }
                        }));
                    }
                    if let Some(paths) = value.write {
                        entries.extend(paths.into_iter().map(|path| {
                            ExecServerFileSystemSandboxEntry {
                                path: ExecServerFileSystemPath::Path { path },
                                access: ExecServerFileSystemAccessMode::Write,
                            }
                        }));
                    }
                    Self {
                        entries,
                        glob_scan_max_depth: None,
                    }
                }
            },
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ExecServerPermissionProfileDe {
    Tagged(TaggedExecServerPermissionProfile),
    Legacy(LegacyExecServerPermissionProfile),
}

impl<'de> Deserialize<'de> for ExecServerPermissionProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            match ExecServerPermissionProfileDe::deserialize(deserializer)? {
                ExecServerPermissionProfileDe::Tagged(value) => value.into(),
                ExecServerPermissionProfileDe::Legacy(value) => value.into(),
            },
        )
    }
}
