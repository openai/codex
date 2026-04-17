use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::NetworkAccess;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::canonicalize_preserving_symlinks;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

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
    let file_system = additional_permissions
        .file_system
        .map(|file_system| {
            let mut entries = Vec::with_capacity(file_system.entries.len());
            for entry in file_system.entries {
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
            FileSystemPermissions { entries }
        })
        .filter(|file_system| !file_system.is_empty());
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
    cwd: &Path,
) -> PermissionProfile {
    let file_system = requested
        .file_system
        .map(|requested_file_system| {
            let requested_policy =
                FileSystemSandboxPolicy::restricted(requested_file_system.entries.clone());
            let entries = granted
                .file_system
                .unwrap_or_default()
                .entries
                .into_iter()
                .filter(|entry| {
                    granted_file_system_entry_within_request(
                        &requested_file_system,
                        &requested_policy,
                        entry,
                        cwd,
                    )
                })
                .collect();
            FileSystemPermissions { entries }
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

fn granted_file_system_entry_within_request(
    requested: &FileSystemPermissions,
    requested_policy: &FileSystemSandboxPolicy,
    granted_entry: &FileSystemSandboxEntry,
    cwd: &Path,
) -> bool {
    if !granted_entry.access.can_read() {
        return false;
    }

    if let Some(path) = resolve_permission_path(&granted_entry.path, cwd) {
        return access_covers(
            requested_policy.resolve_access_with_cwd(path.as_path(), cwd),
            granted_entry.access,
        );
    }

    requested.entries.iter().any(|requested_entry| {
        access_covers(requested_entry.access, granted_entry.access)
            && requested_entry.path == granted_entry.path
    })
}

fn access_covers(requested: FileSystemAccessMode, granted: FileSystemAccessMode) -> bool {
    match granted {
        FileSystemAccessMode::Read => requested.can_read(),
        FileSystemAccessMode::Write => requested.can_write(),
        FileSystemAccessMode::None => false,
    }
}

fn resolve_permission_path(path: &FileSystemPath, cwd: &Path) -> Option<AbsolutePathBuf> {
    match path {
        FileSystemPath::Path { path } => Some(path.clone()),
        FileSystemPath::GlobPattern { .. } => None,
        FileSystemPath::Special { value } => match value {
            FileSystemSpecialPath::Root => {
                let root = cwd.ancestors().last()?;
                AbsolutePathBuf::from_absolute_path(root).ok()
            }
            FileSystemSpecialPath::CurrentWorkingDirectory => {
                AbsolutePathBuf::from_absolute_path(cwd).ok()
            }
            FileSystemSpecialPath::ProjectRoots { subpath } => {
                let cwd = AbsolutePathBuf::from_absolute_path(cwd).ok()?;
                Some(match subpath {
                    Some(subpath) => {
                        AbsolutePathBuf::resolve_path_against_base(subpath, cwd.as_path())
                    }
                    None => cwd,
                })
            }
            FileSystemSpecialPath::Tmpdir => {
                let tmpdir = std::env::var_os("TMPDIR")?;
                if tmpdir.is_empty() {
                    None
                } else {
                    AbsolutePathBuf::from_absolute_path(PathBuf::from(tmpdir)).ok()
                }
            }
            FileSystemSpecialPath::SlashTmp => AbsolutePathBuf::from_absolute_path("/tmp")
                .ok()
                .filter(|path| path.as_path().is_dir()),
            FileSystemSpecialPath::Minimal | FileSystemSpecialPath::Unknown { .. } => None,
        },
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
