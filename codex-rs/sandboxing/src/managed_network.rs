use crate::manager::SandboxTransformError;
use codex_network_proxy::NetworkProxyChildEnvSnapshot;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;

pub(crate) fn with_managed_mitm_ca_proxy_dirs_denied(
    permission_profile: PermissionProfile,
    managed_mitm_ca_trust_bundle_paths: &[AbsolutePathBuf],
    sandbox_policy_cwd: &Path,
) -> Result<PermissionProfile, SandboxTransformError> {
    if managed_mitm_ca_trust_bundle_paths.is_empty() {
        return Ok(permission_profile);
    }
    let (mut file_system_sandbox_policy, network_sandbox_policy) =
        permission_profile.to_runtime_permissions();

    // Seatbelt and bubblewrap can apply the parent deny plus file carveback per
    // invocation. Windows read grants persist; other profile kinds stay unchanged.
    if !cfg!(any(target_os = "linux", target_os = "macos"))
        || file_system_sandbox_policy.kind != FileSystemSandboxKind::Restricted
    {
        return Ok(permission_profile);
    }

    // Hide the entire proxy directory before custom CA materialization. This
    // prevents a later command from asking the unsandboxed host to copy an
    // earlier command's generated bundle (or the MITM private key) into its
    // own active bundle.
    let mut managed_mitm_ca_dirs = managed_mitm_ca_trust_bundle_paths
        .iter()
        .filter_map(|path| path.as_path().parent())
        .filter_map(|path| AbsolutePathBuf::from_absolute_path(path).ok())
        .collect::<Vec<_>>();
    managed_mitm_ca_dirs.sort();
    managed_mitm_ca_dirs.dedup();
    if managed_mitm_ca_dirs.iter().any(|path| {
        managed_mitm_ca_dir_overlaps_writable_path(
            &file_system_sandbox_policy,
            path.as_path(),
            sandbox_policy_cwd,
        )
    }) {
        return Err(SandboxTransformError::ManagedMitmCaPathUnderWritableRoot);
    }
    for path in managed_mitm_ca_dirs {
        let entry = FileSystemSandboxEntry {
            path: FileSystemPath::Path { path },
            access: FileSystemAccessMode::Deny,
        };
        if !file_system_sandbox_policy.entries.contains(&entry) {
            file_system_sandbox_policy.entries.push(entry);
        }
    }
    Ok(
        PermissionProfile::from_runtime_permissions_with_enforcement(
            permission_profile.enforcement(),
            &file_system_sandbox_policy,
            network_sandbox_policy,
        ),
    )
}

fn managed_mitm_ca_dir_overlaps_writable_path(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    managed_mitm_ca_dir: &Path,
    sandbox_policy_cwd: &Path,
) -> bool {
    if file_system_sandbox_policy.has_full_disk_write_access() {
        return true;
    }
    let managed_mitm_ca_dir_canonical = managed_mitm_ca_dir.canonicalize().ok();
    let has_explicit_writable_overlap = file_system_sandbox_policy
        .get_writable_roots_with_cwd(sandbox_policy_cwd)
        .into_iter()
        .any(|root| {
            let root = root.root;
            managed_mitm_ca_dir.starts_with(root.as_path())
                || root.as_path().starts_with(managed_mitm_ca_dir)
                || managed_mitm_ca_dir_canonical
                    .as_ref()
                    .is_some_and(|managed_dir| {
                        root.as_path().canonicalize().is_ok_and(|root| {
                            managed_dir.starts_with(&root) || root.starts_with(managed_dir)
                        })
                    })
        });
    if has_explicit_writable_overlap {
        return true;
    }

    #[cfg(target_os = "macos")]
    if file_system_sandbox_policy.include_platform_defaults() {
        return ["/tmp", "/private/tmp", "/var/tmp", "/private/var/tmp"]
            .into_iter()
            .map(Path::new)
            .any(|root| {
                managed_mitm_ca_dir.starts_with(root)
                    || managed_mitm_ca_dir_canonical
                        .as_ref()
                        .is_some_and(|managed_dir| {
                            root.canonicalize()
                                .is_ok_and(|root| managed_dir.starts_with(root))
                        })
            });
    }

    false
}

pub(crate) fn with_managed_mitm_ca_readable_roots(
    permission_profile: PermissionProfile,
    managed_mitm_ca_trust_bundle_paths: &[AbsolutePathBuf],
    sandbox_policy_cwd: &Path,
) -> PermissionProfile {
    let (file_system_sandbox_policy, network_sandbox_policy) =
        permission_profile.to_runtime_permissions();
    let file_system_sandbox_policy = file_system_sandbox_policy
        .with_additional_readable_roots(sandbox_policy_cwd, managed_mitm_ca_trust_bundle_paths);
    PermissionProfile::from_runtime_permissions_with_enforcement(
        permission_profile.enforcement(),
        &file_system_sandbox_policy,
        network_sandbox_policy,
    )
}

pub fn prepare_managed_network_child(
    network: Option<&NetworkProxyChildEnvSnapshot>,
    env: &mut HashMap<String, String>,
    command_cwd: &Path,
    permission_profile: PermissionProfile,
    sandbox_policy_cwd: &Path,
    persistent_windows_sandbox: bool,
) -> Result<PermissionProfile, SandboxTransformError> {
    let managed_mitm_ca_trust_bundle_paths = network
        .and_then(NetworkProxyChildEnvSnapshot::managed_mitm_ca_trust_bundle_path)
        .into_iter()
        .collect::<Vec<_>>();
    let permission_profile = with_managed_mitm_ca_proxy_dirs_denied(
        permission_profile,
        &managed_mitm_ca_trust_bundle_paths,
        sandbox_policy_cwd,
    )?;
    if persistent_windows_sandbox
        && network.is_some_and(|network| network.requires_child_specific_mitm_ca_bundle(env))
    {
        return Err(SandboxTransformError::ManagedMitmCustomCaUnsupportedOnWindows);
    }
    let active_mitm_ca_trust_bundle_paths = network.map_or_else(Vec::new, |network| {
        if persistent_windows_sandbox {
            return network.prepare_persistent_sandbox_child_env(env);
        }
        let file_system_sandbox_policy = permission_profile.file_system_sandbox_policy();
        let read_deny_glob_matcher =
            read_deny_glob_matcher(&file_system_sandbox_policy, sandbox_policy_cwd);
        network.prepare_child_env(env, command_cwd, |path| {
            can_read_path_with_policy(
                &file_system_sandbox_policy,
                read_deny_glob_matcher.as_ref(),
                path,
                sandbox_policy_cwd,
            )
        })
    });
    Ok(with_managed_mitm_ca_readable_roots(
        permission_profile,
        &active_mitm_ca_trust_bundle_paths,
        sandbox_policy_cwd,
    ))
}

pub(crate) fn read_deny_glob_matcher(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> Option<ReadDenyMatcher> {
    // Exact deny roots participate in normal path-specificity resolution, so
    // a narrower explicit read entry can validly reopen one CA file below a
    // denied parent. Globs are enforced separately and must still fail closed.
    let mut deny_glob_policy = file_system_sandbox_policy.clone();
    deny_glob_policy.entries.retain(|entry| {
        entry.access == FileSystemAccessMode::Deny
            && matches!(entry.path, FileSystemPath::GlobPattern { .. })
    });
    ReadDenyMatcher::new(&deny_glob_policy, cwd)
}

pub(crate) fn can_read_path_with_policy(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    read_deny_glob_matcher: Option<&ReadDenyMatcher>,
    path: &Path,
    cwd: &Path,
) -> bool {
    file_system_sandbox_policy.can_read_path_with_cwd(path, cwd)
        && path.canonicalize().is_ok_and(|canonical_path| {
            file_system_sandbox_policy.can_read_path_with_cwd(&canonical_path, cwd)
        })
        && !read_deny_glob_matcher.is_some_and(|matcher| matcher.is_read_denied(path))
}
