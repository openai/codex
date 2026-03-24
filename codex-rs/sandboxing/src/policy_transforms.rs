use crate::macos_permissions::intersect_macos_seatbelt_profile_extensions;
use crate::macos_permissions::merge_macos_seatbelt_profile_extensions;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;
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
use dunce::canonicalize;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSandboxPermissions {
    pub sandbox_policy: SandboxPolicy,
    pub macos_seatbelt_profile_extensions: Option<MacOsSeatbeltProfileExtensions>,
}

impl EffectiveSandboxPermissions {
    pub fn new(
        sandbox_policy: &SandboxPolicy,
        macos_seatbelt_profile_extensions: Option<&MacOsSeatbeltProfileExtensions>,
        additional_permissions: Option<&PermissionProfile>,
    ) -> Self {
        let Some(additional_permissions) = additional_permissions else {
            return Self {
                sandbox_policy: sandbox_policy.clone(),
                macos_seatbelt_profile_extensions: macos_seatbelt_profile_extensions.cloned(),
            };
        };

        Self {
            sandbox_policy: effective_sandbox_policy(sandbox_policy, Some(additional_permissions)),
            macos_seatbelt_profile_extensions: merge_macos_seatbelt_profile_extensions(
                macos_seatbelt_profile_extensions,
                additional_permissions.macos.as_ref(),
            ),
        }
    }
}

pub fn normalize_additional_permissions(
    additional_permissions: PermissionProfile,
) -> Result<PermissionProfile, String> {
    let network = additional_permissions
        .network
        .filter(|network| !network.is_empty());
    let file_system = additional_permissions
        .file_system
        .map(|file_system| {
            let read = file_system
                .read
                .map(|paths| normalize_permission_paths(paths, "file_system.read"));
            let write = file_system
                .write
                .map(|paths| normalize_permission_paths(paths, "file_system.write"));
            FileSystemPermissions { read, write }
        })
        .filter(|file_system| !file_system.is_empty());
    let macos = additional_permissions.macos;

    Ok(PermissionProfile {
        network,
        file_system,
        macos,
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
                    read: merge_permission_paths(base.read.as_ref(), permissions.read.as_ref()),
                    write: merge_permission_paths(base.write.as_ref(), permissions.write.as_ref()),
                })
                .filter(|file_system| !file_system.is_empty()),
                (Some(base), None) => Some(base.clone()),
                (None, Some(permissions)) => Some(permissions.clone()),
                (None, None) => None,
            };
            let macos = merge_macos_seatbelt_profile_extensions(
                base.macos.as_ref(),
                permissions.macos.as_ref(),
            );

            Some(PermissionProfile {
                network,
                file_system,
                macos,
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
            let read = requested_file_system
                .read
                .map(|requested_read| {
                    let granted_read = granted_file_system.read.unwrap_or_default();
                    requested_read
                        .into_iter()
                        .filter(|path| granted_read.contains(path))
                        .collect()
                })
                .filter(|paths: &Vec<_>| !paths.is_empty());
            let write = requested_file_system
                .write
                .map(|requested_write| {
                    let granted_write = granted_file_system.write.unwrap_or_default();
                    requested_write
                        .into_iter()
                        .filter(|path| granted_write.contains(path))
                        .collect()
                })
                .filter(|paths: &Vec<_>| !paths.is_empty());
            FileSystemPermissions { read, write }
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

    let macos = intersect_macos_seatbelt_profile_extensions(requested.macos, granted.macos);

    PermissionProfile {
        network,
        file_system,
        macos,
    }
}

fn normalize_permission_paths(
    paths: Vec<AbsolutePathBuf>,
    _permission_kind: &str,
) -> Vec<AbsolutePathBuf> {
    let mut out = Vec::with_capacity(paths.len());
    let mut seen = HashSet::new();

    for path in paths {
        let canonicalized = canonicalize(path.as_path())
            .ok()
            .and_then(|path| AbsolutePathBuf::from_absolute_path(path).ok())
            .unwrap_or(path);
        if seen.insert(canonicalized.clone()) {
            out.push(canonicalized);
        }
    }

    out
}

fn merge_permission_paths(
    base: Option<&Vec<AbsolutePathBuf>>,
    permissions: Option<&Vec<AbsolutePathBuf>>,
) -> Option<Vec<AbsolutePathBuf>> {
    match (base, permissions) {
        (Some(base), Some(permissions)) => {
            let mut merged = Vec::with_capacity(base.len() + permissions.len());
            let mut seen = HashSet::with_capacity(base.len() + permissions.len());

            for path in base.iter().chain(permissions.iter()) {
                if seen.insert(path.clone()) {
                    merged.push(path.clone());
                }
            }

            Some(merged).filter(|paths| !paths.is_empty())
        }
        (Some(base), None) => Some(base.clone()),
        (None, Some(permissions)) => Some(permissions.clone()),
        (None, None) => None,
    }
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
                .and_then(|file_system| file_system.read.clone())
                .unwrap_or_default(),
        ),
        dedup_absolute_paths(
            additional_permissions
                .file_system
                .as_ref()
                .and_then(|file_system| file_system.write.clone())
                .unwrap_or_default(),
        ),
    )
}

fn merge_file_system_policy_with_additional_permissions(
    file_system_policy: &FileSystemSandboxPolicy,
    extra_reads: Vec<AbsolutePathBuf>,
    extra_writes: Vec<AbsolutePathBuf>,
) -> FileSystemSandboxPolicy {
    match file_system_policy.kind {
        FileSystemSandboxKind::Restricted => {
            let mut merged_policy = file_system_policy.clone();
            for path in extra_reads {
                let entry = FileSystemSandboxEntry {
                    path: FileSystemPath::Path { path },
                    access: FileSystemAccessMode::Read,
                };
                if !merged_policy.entries.contains(&entry) {
                    merged_policy.entries.push(entry);
                }
            }
            for path in extra_writes {
                let entry = FileSystemSandboxEntry {
                    path: FileSystemPath::Path { path },
                    access: FileSystemAccessMode::Write,
                };
                if !merged_policy.entries.contains(&entry) {
                    merged_policy.entries.push(entry);
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

    let (extra_reads, extra_writes) = additional_permission_roots(additional_permissions);
    if extra_reads.is_empty() && extra_writes.is_empty() {
        file_system_policy.clone()
    } else {
        merge_file_system_policy_with_additional_permissions(
            file_system_policy,
            extra_reads,
            extra_writes,
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
