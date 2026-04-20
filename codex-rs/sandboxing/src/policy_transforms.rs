use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::NetworkAccess;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::canonicalize_preserving_symlinks;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSandboxPermissions {
    pub sandbox_policy: SandboxPolicy,
}

impl EffectiveSandboxPermissions {
    pub fn new(
        sandbox_policy: &SandboxPolicy,
        additional_permissions: Option<&PermissionProfile>,
    ) -> Self {
        let Some(additional_permissions) = additional_permissions else {
            return Self {
                sandbox_policy: sandbox_policy.clone(),
            };
        };

        Self {
            sandbox_policy: effective_sandbox_policy(sandbox_policy, Some(additional_permissions)),
        }
    }
}

pub fn normalize_additional_permissions(
    additional_permissions: PermissionProfile,
) -> Result<PermissionProfile, String> {
    let network = additional_permissions
        .network
        .filter(|network| !network.is_empty());
    let file_system = match additional_permissions.file_system {
        Some(file_system) => {
            let mut entries = Vec::with_capacity(file_system.entries.len());
            let glob_scan_max_depth = file_system.glob_scan_max_depth;
            for entry in file_system.entries {
                if matches!(&entry.path, FileSystemPath::GlobPattern { .. })
                    && entry.access != FileSystemAccessMode::None
                {
                    return Err(
                        "glob file system permissions only support deny-read entries".to_string(),
                    );
                }
                let path = match entry.path {
                    FileSystemPath::Path { path } => FileSystemPath::Path {
                        path: canonicalize_preserving_symlinks(path.as_path())
                            .ok()
                            .and_then(|path| AbsolutePathBuf::from_absolute_path(path).ok())
                            .unwrap_or(path),
                    },
                    FileSystemPath::GlobPattern { pattern } => {
                        FileSystemPath::GlobPattern { pattern }
                    }
                    FileSystemPath::Special { value } => FileSystemPath::Special { value },
                };
                let normalized_entry = FileSystemSandboxEntry {
                    path,
                    access: entry.access,
                };
                if !entries.contains(&normalized_entry) {
                    entries.push(normalized_entry);
                }
            }
            let file_system = FileSystemPermissions {
                entries,
                glob_scan_max_depth,
            };
            (!file_system.is_empty()).then_some(file_system)
        }
        None => None,
    };
    Ok(PermissionProfile {
        network,
        file_system,
    })
}

pub fn merge_permission_profiles(
    base: Option<&PermissionProfile>,
    permissions: Option<&PermissionProfile>,
) -> Option<PermissionProfile> {
    let Some(permissions) = permissions else {
        return base.cloned();
    };

    match base {
        Some(base) => {
            let network = match (base.network.as_ref(), permissions.network.as_ref()) {
                (
                    Some(NetworkPermissions {
                        enabled: Some(true),
                    }),
                    _,
                )
                | (
                    _,
                    Some(NetworkPermissions {
                        enabled: Some(true),
                    }),
                ) => Some(NetworkPermissions {
                    enabled: Some(true),
                }),
                _ => None,
            };
            let file_system = match (base.file_system.as_ref(), permissions.file_system.as_ref()) {
                (Some(base), Some(permissions)) => Some(FileSystemPermissions {
                    entries: merge_permission_entries(&base.entries, &permissions.entries),
                    glob_scan_max_depth: base
                        .glob_scan_max_depth
                        .max(permissions.glob_scan_max_depth),
                })
                .filter(|file_system| !file_system.is_empty()),
                (Some(base), None) => Some(base.clone()),
                (None, Some(permissions)) => Some(permissions.clone()),
                (None, None) => None,
            };

            Some(PermissionProfile {
                network,
                file_system,
            })
            .filter(|permissions| !permissions.is_empty())
        }
        None => Some(permissions.clone()).filter(|permissions| !permissions.is_empty()),
    }
}

pub fn intersect_permission_profiles(
    requested: PermissionProfile,
    granted: PermissionProfile,
) -> PermissionProfile {
    let file_system = requested
        .file_system
        .map(|requested_file_system| {
            let granted_file_system = granted.file_system.unwrap_or_default();
            let entries = requested_file_system
                .entries
                .into_iter()
                .filter(|entry| granted_file_system.entries.contains(entry))
                .collect();
            FileSystemPermissions {
                entries,
                glob_scan_max_depth: requested_file_system.glob_scan_max_depth,
            }
        })
        .filter(|file_system| !file_system.is_empty());
    let network = match (requested.network, granted.network) {
        (
            Some(NetworkPermissions {
                enabled: Some(true),
            }),
            Some(NetworkPermissions {
                enabled: Some(true),
            }),
        ) => Some(NetworkPermissions {
            enabled: Some(true),
        }),
        _ => None,
    };

    PermissionProfile {
        network,
        file_system,
    }
}

fn merge_permission_entries(
    base: &[FileSystemSandboxEntry],
    permissions: &[FileSystemSandboxEntry],
) -> Vec<FileSystemSandboxEntry> {
    let mut merged = Vec::with_capacity(base.len() + permissions.len());
    for entry in base.iter().chain(permissions.iter()) {
        if !merged.contains(entry) {
            merged.push(entry.clone());
        }
    }
    merged
}

fn dedup_absolute_paths(paths: Vec<AbsolutePathBuf>) -> Vec<AbsolutePathBuf> {
    let mut out = Vec::with_capacity(paths.len());
    let mut seen = HashSet::new();
    for path in paths {
        if seen.insert(path.to_path_buf()) {
            out.push(path);
        }
    }
    out
}

fn additional_permission_roots(
    additional_permissions: &PermissionProfile,
) -> (Vec<AbsolutePathBuf>, Vec<AbsolutePathBuf>) {
    (
        dedup_absolute_paths(
            additional_permissions
                .file_system
                .as_ref()
                .map(|file_system| {
                    file_system
                        .explicit_path_entries()
                        .filter_map(|(path, access)| access.can_read().then_some(path.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        ),
        dedup_absolute_paths(
            additional_permissions
                .file_system
                .as_ref()
                .map(|file_system| {
                    file_system
                        .explicit_path_entries()
                        .filter_map(|(path, access)| access.can_write().then_some(path.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        ),
    )
}

fn merge_file_system_policy_with_additional_permissions(
    file_system_policy: &FileSystemSandboxPolicy,
    additional_permissions: &FileSystemPermissions,
) -> FileSystemSandboxPolicy {
    match file_system_policy.kind {
        FileSystemSandboxKind::Restricted => {
            let mut merged_policy = file_system_policy.clone();
            for entry in &additional_permissions.entries {
                if !merged_policy.entries.contains(entry) {
                    merged_policy.entries.push(entry.clone());
                }
            }
            merged_policy
        }
        FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => {
            file_system_policy.clone()
        }
    }
}

pub fn effective_file_system_sandbox_policy(
    file_system_policy: &FileSystemSandboxPolicy,
    additional_permissions: Option<&PermissionProfile>,
) -> FileSystemSandboxPolicy {
    let Some(additional_permissions) = additional_permissions else {
        return file_system_policy.clone();
    };

    let Some(file_system_permissions) = additional_permissions.file_system.as_ref() else {
        return file_system_policy.clone();
    };
    if file_system_permissions.is_empty() {
        file_system_policy.clone()
    } else {
        merge_file_system_policy_with_additional_permissions(
            file_system_policy,
            file_system_permissions,
        )
    }
}

fn merge_read_only_access_with_additional_reads(
    read_only_access: &ReadOnlyAccess,
    extra_reads: Vec<AbsolutePathBuf>,
) -> ReadOnlyAccess {
    match read_only_access {
        ReadOnlyAccess::FullAccess => ReadOnlyAccess::FullAccess,
        ReadOnlyAccess::Restricted {
            include_platform_defaults,
            readable_roots,
        } => {
            let mut merged = readable_roots.clone();
            merged.extend(extra_reads);
            ReadOnlyAccess::Restricted {
                include_platform_defaults: *include_platform_defaults,
                readable_roots: dedup_absolute_paths(merged),
            }
        }
    }
}

fn merge_network_access(
    base_network_access: bool,
    additional_permissions: &PermissionProfile,
) -> bool {
    base_network_access
        || additional_permissions
            .network
            .as_ref()
            .and_then(|network| network.enabled)
            .unwrap_or(false)
}

pub fn effective_network_sandbox_policy(
    network_policy: NetworkSandboxPolicy,
    additional_permissions: Option<&PermissionProfile>,
) -> NetworkSandboxPolicy {
    if additional_permissions
        .is_some_and(|permissions| merge_network_access(network_policy.is_enabled(), permissions))
    {
        NetworkSandboxPolicy::Enabled
    } else if additional_permissions.is_some() {
        NetworkSandboxPolicy::Restricted
    } else {
        network_policy
    }
}

fn sandbox_policy_with_additional_permissions(
    sandbox_policy: &SandboxPolicy,
    additional_permissions: &PermissionProfile,
) -> SandboxPolicy {
    if additional_permissions.is_empty() {
        return sandbox_policy.clone();
    }

    let (extra_reads, extra_writes) = additional_permission_roots(additional_permissions);

    match sandbox_policy {
        SandboxPolicy::DangerFullAccess => SandboxPolicy::DangerFullAccess,
        SandboxPolicy::ExternalSandbox { network_access } => SandboxPolicy::ExternalSandbox {
            network_access: if merge_network_access(
                network_access.is_enabled(),
                additional_permissions,
            ) {
                NetworkAccess::Enabled
            } else {
                NetworkAccess::Restricted
            },
        },
        SandboxPolicy::WorkspaceWrite {
            writable_roots,
            read_only_access,
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        } => {
            let mut merged_writes = writable_roots.clone();
            merged_writes.extend(extra_writes);
            SandboxPolicy::WorkspaceWrite {
                writable_roots: dedup_absolute_paths(merged_writes),
                read_only_access: merge_read_only_access_with_additional_reads(
                    read_only_access,
                    extra_reads,
                ),
                network_access: merge_network_access(*network_access, additional_permissions),
                exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
                exclude_slash_tmp: *exclude_slash_tmp,
            }
        }
        SandboxPolicy::ReadOnly {
            access,
            network_access,
        } => {
            if extra_writes.is_empty() {
                SandboxPolicy::ReadOnly {
                    access: merge_read_only_access_with_additional_reads(access, extra_reads),
                    network_access: merge_network_access(*network_access, additional_permissions),
                }
            } else {
                // todo(dylan) - for now, this grants more access than the request. We should restrict this,
                // but we should add a new SandboxPolicy variant to handle this. While the feature is still
                // UnderDevelopment, it's a useful approximation of the desired behavior.
                SandboxPolicy::WorkspaceWrite {
                    writable_roots: dedup_absolute_paths(extra_writes),
                    read_only_access: merge_read_only_access_with_additional_reads(
                        access,
                        extra_reads,
                    ),
                    network_access: merge_network_access(*network_access, additional_permissions),
                    exclude_tmpdir_env_var: false,
                    exclude_slash_tmp: false,
                }
            }
        }
    }
}

fn effective_sandbox_policy(
    sandbox_policy: &SandboxPolicy,
    additional_permissions: Option<&PermissionProfile>,
) -> SandboxPolicy {
    additional_permissions.map_or_else(
        || sandbox_policy.clone(),
        |permissions| sandbox_policy_with_additional_permissions(sandbox_policy, permissions),
    )
}

pub fn should_require_platform_sandbox(
    file_system_policy: &FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    has_managed_network_requirements: bool,
) -> bool {
    if has_managed_network_requirements {
        return true;
    }

    if !network_policy.is_enabled() {
        return !matches!(
            file_system_policy.kind,
            FileSystemSandboxKind::ExternalSandbox
        );
    }

    match file_system_policy.kind {
        FileSystemSandboxKind::Restricted => !file_system_policy.has_full_disk_write_access(),
        FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => false,
    }
}

#[cfg(test)]
#[path = "policy_transforms_tests.rs"]
mod tests;
