use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_protocol::permissions::file_system_root;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use std::num::NonZeroUsize;

pub fn normalize_additional_permissions(
    additional_permissions: AdditionalPermissionProfile,
) -> Result<AdditionalPermissionProfile, String> {
    let network = additional_permissions
        .network
        .filter(|network| !network.is_empty());
    let file_system = match additional_permissions.file_system {
        Some(file_system) => {
            let mut entries = Vec::with_capacity(file_system.entries.len());
            let glob_scan_max_depth = file_system.glob_scan_max_depth;
            for entry in file_system.entries {
                if matches!(&entry.path, FileSystemPath::GlobPattern { .. })
                    && entry.access != FileSystemAccessMode::Deny
                {
                    return Err(
                        "glob file system permissions only support deny-read entries".to_string(),
                    );
                }
                if !entries.contains(&entry) {
                    entries.push(entry);
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
    Ok(AdditionalPermissionProfile {
        network,
        file_system,
    })
}

pub fn normalize_additional_permissions_with_context(
    additional_permissions: AdditionalPermissionProfile,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> Result<AdditionalPermissionProfile, String> {
    let normalized = normalize_additional_permissions(additional_permissions)?;
    if let Some(file_system) = normalized.file_system.as_ref() {
        for entry in &file_system.entries {
            let FileSystemPath::Path { path } = &entry.path else {
                continue;
            };
            if path.infer_path_convention().is_none()
                || path.infer_path_convention() != context.cwd.infer_path_convention()
                || path.join(".").is_err()
            {
                return Err(format!(
                    "permission path `{path}` does not match executor cwd `{}`",
                    context.cwd
                ));
            }
        }
    }
    Ok(normalized)
}

/// Resolves cwd-dependent permission entries without filtering their authority.
///
/// Unlike intersection, this preserves narrower grants beneath denied paths.
pub fn materialize_additional_permissions_with_context(
    mut additional_permissions: AdditionalPermissionProfile,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> Result<AdditionalPermissionProfile, String> {
    if let Some(file_system) = additional_permissions.file_system.as_mut() {
        file_system.entries = materialize_context_dependent_entries(&file_system.entries, context)
            .ok_or_else(|| format!("unable to resolve permission path in `{}`", context.cwd))?;
    }
    normalize_additional_permissions_with_context(additional_permissions, context)
}

pub fn merge_permission_profiles(
    base: Option<&AdditionalPermissionProfile>,
    permissions: Option<&AdditionalPermissionProfile>,
) -> Option<AdditionalPermissionProfile> {
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
                    glob_scan_max_depth: merge_glob_scan_max_depth(
                        &base.entries,
                        base.glob_scan_max_depth.map(usize::from),
                        &permissions.entries,
                        permissions.glob_scan_max_depth.map(usize::from),
                    )
                    .and_then(NonZeroUsize::new),
                })
                .filter(|file_system| !file_system.is_empty()),
                (Some(base), None) => Some(base.clone()),
                (None, Some(permissions)) => Some(permissions.clone()),
                (None, None) => None,
            };

            Some(AdditionalPermissionProfile {
                network,
                file_system,
            })
            .filter(|permissions| !permissions.is_empty())
        }
        None => Some(permissions.clone()).filter(|permissions| !permissions.is_empty()),
    }
}

pub fn intersect_permission_profiles_with_context(
    requested: AdditionalPermissionProfile,
    granted: AdditionalPermissionProfile,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> AdditionalPermissionProfile {
    let file_system = requested
        .file_system
        .and_then(|requested_file_system| {
            let granted_file_system = granted.file_system.unwrap_or_default();
            let requested_entries =
                materialize_context_dependent_entries(&requested_file_system.entries, context)?;
            let granted_entries =
                materialize_context_dependent_entries(&granted_file_system.entries, context)?;
            let requested_policy = FileSystemSandboxPolicy::restricted(requested_entries.clone());
            let requested_read_deny_matcher =
                ReadDenyMatcher::from_context(&requested_policy, context);
            let mut accepted_entries = Vec::new();
            for entry in granted_entries.iter().filter(|entry| {
                granted_file_system_entry_within_request(
                    &requested_file_system,
                    &requested_policy,
                    requested_read_deny_matcher.as_ref(),
                    entry,
                    context,
                )
            }) {
                if !accepted_entries.contains(entry) {
                    accepted_entries.push(entry.clone());
                }
            }
            let mut entries = accepted_entries.clone();
            let requested_retained_deny_entries = retain_constraining_deny_entries(
                &requested_entries,
                &accepted_entries,
                context,
                &mut entries,
            );
            let granted_retained_deny_entries = retain_constraining_deny_entries(
                &granted_entries,
                &accepted_entries,
                context,
                &mut entries,
            );
            Some(FileSystemPermissions {
                glob_scan_max_depth: merge_glob_scan_max_depth(
                    &requested_retained_deny_entries,
                    requested_file_system.glob_scan_max_depth.map(usize::from),
                    &granted_retained_deny_entries,
                    granted_file_system.glob_scan_max_depth.map(usize::from),
                )
                .and_then(NonZeroUsize::new),
                entries,
            })
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

    AdditionalPermissionProfile {
        network,
        file_system,
    }
}

fn merge_glob_scan_max_depth(
    left_entries: &[FileSystemSandboxEntry],
    left_depth: Option<usize>,
    right_entries: &[FileSystemSandboxEntry],
    right_depth: Option<usize>,
) -> Option<usize> {
    let left_depth = effective_glob_scan_depth(left_entries, left_depth);
    let right_depth = effective_glob_scan_depth(right_entries, right_depth);

    match (left_depth, right_depth) {
        (Some(GlobScanDepth::Unbounded), _) | (_, Some(GlobScanDepth::Unbounded)) => None,
        (Some(GlobScanDepth::Bounded(left)), Some(GlobScanDepth::Bounded(right))) => {
            Some(left.max(right))
        }
        (Some(GlobScanDepth::Bounded(depth)), None)
        | (None, Some(GlobScanDepth::Bounded(depth))) => Some(depth),
        (None, None) => None,
    }
}

fn effective_glob_scan_depth(
    entries: &[FileSystemSandboxEntry],
    depth: Option<usize>,
) -> Option<GlobScanDepth> {
    entries
        .iter()
        .any(|entry| {
            entry.access == FileSystemAccessMode::Deny
                && matches!(&entry.path, FileSystemPath::GlobPattern { .. })
        })
        .then_some(match depth {
            Some(depth) => GlobScanDepth::Bounded(depth),
            None => GlobScanDepth::Unbounded,
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GlobScanDepth {
    Bounded(usize),
    Unbounded,
}

fn granted_file_system_entry_within_request(
    requested: &FileSystemPermissions,
    requested_policy: &FileSystemSandboxPolicy,
    requested_read_deny_matcher: Option<&ReadDenyMatcher>,
    granted_entry: &FileSystemSandboxEntry,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> bool {
    if !granted_entry.access.can_read()
        || matches!(
            &granted_entry.path,
            FileSystemPath::Special {
                value: FileSystemSpecialPath::SlashTmp,
            } if context.cwd.infer_path_convention() != Some(PathConvention::Posix)
        )
    {
        return false;
    }
    if context.cwd.infer_path_convention() == Some(PathConvention::Windows)
        && is_root_entry(granted_entry)
        && !requested.entries.iter().any(|requested_entry| {
            is_root_entry(requested_entry)
                && access_covers(requested_entry.access, granted_entry.access)
        })
    {
        return false;
    }

    if let Some(path) = resolve_permission_path(&granted_entry.path, context) {
        if path.infer_path_convention() != context.cwd.infer_path_convention()
            || requested_read_deny_matcher
                .is_some_and(|matcher| matcher.is_read_denied_uri(&path, context))
        {
            return false;
        }
        return access_covers(
            requested_policy.resolve_access(&path, context),
            granted_entry.access,
        );
    }

    requested.entries.iter().any(|requested_entry| {
        access_covers(requested_entry.access, granted_entry.access)
            && requested_entry.path == granted_entry.path
    })
}

fn retain_constraining_deny_entries(
    source_entries: &[FileSystemSandboxEntry],
    accepted_entries: &[FileSystemSandboxEntry],
    context: &FileSystemSandboxPolicyContext<'_>,
    output_entries: &mut Vec<FileSystemSandboxEntry>,
) -> Vec<FileSystemSandboxEntry> {
    let mut retained_entries = Vec::new();
    for entry in source_entries
        .iter()
        .filter(|entry| entry.access == FileSystemAccessMode::Deny)
    {
        if !deny_entry_constrains_accepted_grant(entry, accepted_entries, context) {
            continue;
        }
        if !output_entries.contains(entry) {
            output_entries.push(entry.clone());
        }
        retained_entries.push(entry.clone());
    }
    retained_entries
}

fn deny_entry_constrains_accepted_grant(
    deny_entry: &FileSystemSandboxEntry,
    accepted_entries: &[FileSystemSandboxEntry],
    context: &FileSystemSandboxPolicyContext<'_>,
) -> bool {
    accepted_entries
        .iter()
        .filter(|entry| entry.access.can_read())
        .any(|entry| {
            if is_root_entry(entry) {
                return true;
            }
            let Some(grant_path) = resolve_permission_path(&entry.path, context) else {
                return true;
            };
            match &deny_entry.path {
                FileSystemPath::GlobPattern { pattern } => {
                    glob_static_prefix_path(pattern, context)
                        .is_none_or(|prefix| paths_overlap(&prefix, &grant_path))
                }
                FileSystemPath::Path { .. } | FileSystemPath::Special { .. } => {
                    resolve_permission_path(&deny_entry.path, context)
                        .is_none_or(|deny_path| paths_overlap(&deny_path, &grant_path))
                }
            }
        })
}

fn glob_static_prefix_path(
    pattern: &str,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> Option<PathUri> {
    let is_windows = context.cwd.infer_path_convention() == Some(PathConvention::Windows);
    let (prefix, wildcard_in_segment) = match pattern.find(['*', '?', '[', ']']) {
        Some(0) => return None,
        Some(index) => {
            let prefix = &pattern[..index];
            (
                prefix,
                !(prefix.ends_with('/') || is_windows && prefix.ends_with('\\')),
            )
        }
        None => (pattern, false),
    };
    let prefix = context.cwd.join(prefix).ok()?;
    if wildcard_in_segment {
        prefix.parent()
    } else {
        Some(prefix)
    }
}

fn paths_overlap(left: &PathUri, right: &PathUri) -> bool {
    left.overlaps(right).unwrap_or(true)
}

fn access_covers(requested: FileSystemAccessMode, granted: FileSystemAccessMode) -> bool {
    match granted {
        FileSystemAccessMode::Read => requested.can_read(),
        FileSystemAccessMode::Write => requested.can_write(),
        FileSystemAccessMode::Deny => false,
    }
}

fn is_root_entry(entry: &FileSystemSandboxEntry) -> bool {
    matches!(
        &entry.path,
        FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        }
    )
}

fn materialize_cwd_dependent_entry(
    entry: &FileSystemSandboxEntry,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> Option<FileSystemSandboxEntry> {
    match &entry.path {
        FileSystemPath::GlobPattern { pattern } => {
            let is_windows = context.cwd.infer_path_convention() == Some(PathConvention::Windows);
            let home_relative = pattern
                .strip_prefix("~/")
                .or_else(|| (pattern == "~").then_some(""))
                .or_else(|| is_windows.then(|| pattern.strip_prefix("~\\")).flatten());
            let (root, pattern) = match home_relative {
                Some(suffix) => (
                    context.user_home_dir?,
                    suffix.trim_start_matches(|separator| {
                        separator == '/' || is_windows && separator == '\\'
                    }),
                ),
                None => (context.cwd, pattern.as_str()),
            };
            let path = root.join(pattern).ok()?;
            let path = FileSystemPath::GlobPattern {
                pattern: path.inferred_native_path_string(),
            };
            Some(FileSystemSandboxEntry {
                path,
                access: entry.access,
                missing_path_behavior: entry.missing_path_behavior,
            })
        }
        FileSystemPath::Path { .. } | FileSystemPath::Special { .. } => Some(entry.clone()),
    }
}

fn resolve_permission_path(
    path: &FileSystemPath,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> Option<PathUri> {
    match path {
        FileSystemPath::Path { path } => Some(path.clone()),
        FileSystemPath::GlobPattern { .. } => None,
        FileSystemPath::Special { value } => match value {
            FileSystemSpecialPath::Root => file_system_root(context),
            FileSystemSpecialPath::ProjectRoots { subpath } => {
                let root = context.workspace_roots.first()?;
                match subpath {
                    Some(subpath) => root.join(subpath).ok(),
                    None => Some(root.clone()),
                }
            }
            FileSystemSpecialPath::Tmpdir => context.temporary_directories?.first().cloned(),
            FileSystemSpecialPath::SlashTmp
                if context.cwd.infer_path_convention() == Some(PathConvention::Posix) =>
            {
                context.cwd.join("/tmp").ok()
            }
            FileSystemSpecialPath::SlashTmp
            | FileSystemSpecialPath::Minimal
            | FileSystemSpecialPath::Unknown { .. } => None,
        },
    }
}

fn materialize_context_dependent_entries(
    entries: &[FileSystemSandboxEntry],
    context: &FileSystemSandboxPolicyContext<'_>,
) -> Option<Vec<FileSystemSandboxEntry>> {
    let mut materialized = Vec::new();
    for entry in entries {
        match &entry.path {
            FileSystemPath::Special {
                value: FileSystemSpecialPath::ProjectRoots { .. },
            } => {
                let mut resolved = Vec::new();
                for root in context.workspace_roots {
                    let mut root_context = *context;
                    root_context.workspace_roots = std::slice::from_ref(root);
                    let Some(path) = resolve_permission_path(&entry.path, &root_context) else {
                        if entry.access == FileSystemAccessMode::Deny {
                            return None;
                        }
                        continue;
                    };
                    resolved.push(materialized_path_entry(entry, path));
                }
                if entry.access == FileSystemAccessMode::Deny && resolved.is_empty() {
                    return None;
                }
                materialized.extend(resolved);
            }
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            } => {
                let Some(temporary_directories) = context.temporary_directories else {
                    if entry.access == FileSystemAccessMode::Deny {
                        return None;
                    }
                    materialized.push(entry.clone());
                    continue;
                };
                materialized.extend(
                    temporary_directories
                        .iter()
                        .cloned()
                        .map(|path| materialized_path_entry(entry, path)),
                );
            }
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            } => {
                resolve_permission_path(&entry.path, context)?;
                materialized.push(entry.clone());
            }
            _ => {
                let Some(entry) = materialize_cwd_dependent_entry(entry, context) else {
                    if entry.access == FileSystemAccessMode::Deny {
                        return None;
                    }
                    continue;
                };
                materialized.push(entry);
            }
        }
    }
    Some(materialized)
}

fn materialized_path_entry(
    entry: &FileSystemSandboxEntry,
    path: PathUri,
) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry {
        path: FileSystemPath::Path { path },
        access: entry.access,
        missing_path_behavior: entry.missing_path_behavior,
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
            merged_policy.glob_scan_max_depth = merge_glob_scan_max_depth(
                &file_system_policy.entries,
                file_system_policy.glob_scan_max_depth,
                &additional_permissions.entries,
                additional_permissions.glob_scan_max_depth.map(usize::from),
            );
            merged_policy
        }
        FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => {
            file_system_policy.clone()
        }
    }
}

pub fn effective_file_system_sandbox_policy(
    file_system_policy: &FileSystemSandboxPolicy,
    additional_permissions: Option<&AdditionalPermissionProfile>,
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

fn merge_network_access(
    base_network_access: bool,
    additional_permissions: &AdditionalPermissionProfile,
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
    additional_permissions: Option<&AdditionalPermissionProfile>,
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

pub fn effective_permission_profile(
    permission_profile: &PermissionProfile,
    additional_permissions: Option<&AdditionalPermissionProfile>,
) -> PermissionProfile {
    let (file_system_policy, network_policy) = permission_profile.to_runtime_permissions();
    let effective_file_system_policy =
        effective_file_system_sandbox_policy(&file_system_policy, additional_permissions);
    let effective_network_policy =
        effective_network_sandbox_policy(network_policy, additional_permissions);
    PermissionProfile::from_runtime_permissions_with_enforcement(
        permission_profile.enforcement(),
        &effective_file_system_policy,
        effective_network_policy,
    )
}

pub fn should_require_platform_sandbox(
    file_system_policy: &FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    has_managed_network_requirements: bool,
) -> bool {
    should_require_platform_sandbox_for_convention(
        file_system_policy,
        network_policy,
        has_managed_network_requirements,
        PathConvention::native(),
    )
}

pub(crate) fn should_require_platform_sandbox_for_convention(
    file_system_policy: &FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    has_managed_network_requirements: bool,
    convention: PathConvention,
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
        FileSystemSandboxKind::Restricted => {
            !file_system_policy.has_full_disk_write_access_for_convention(Some(convention))
        }
        FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => false,
    }
}

#[cfg(test)]
#[path = "policy_transforms_tests.rs"]
mod tests;
