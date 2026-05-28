use super::EffectiveFilesystemPermissions;
use super::FilesystemPermissionsContext;
use super::FilesystemPermissionsError;
use super::FilesystemPermissionsMode;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_protocol::permissions::project_roots_glob_pattern;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;

fn absolute_path(path: &Path) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(path).expect("absolute path")
}

fn derive_effective(
    permission_profile: &PermissionProfile,
    cwd: &AbsolutePathBuf,
) -> super::EffectiveFilesystemPermissions {
    EffectiveFilesystemPermissions::from_profile(
        permission_profile,
        FilesystemPermissionsContext {
            policy_evaluation_cwd: cwd,
        },
    )
    .expect("valid effective filesystem permissions")
}

fn assert_projected_fields_match_policy(
    permission_profile: &PermissionProfile,
    cwd: &AbsolutePathBuf,
) -> super::EffectiveFilesystemPermissions {
    let effective = derive_effective(permission_profile, cwd);
    let policy = permission_profile.file_system_sandbox_policy();
    let expected_mode = match policy.kind {
        FileSystemSandboxKind::Restricted => FilesystemPermissionsMode::Restricted,
        FileSystemSandboxKind::Unrestricted => FilesystemPermissionsMode::Unrestricted,
        FileSystemSandboxKind::ExternalSandbox => FilesystemPermissionsMode::External,
    };

    assert_eq!(effective.mode, expected_mode);
    assert_eq!(
        effective.readable_roots,
        policy.get_readable_roots_with_cwd(cwd.as_path())
    );
    assert_eq!(
        effective.writable_roots,
        policy.get_writable_roots_with_cwd(cwd.as_path())
    );
    assert_eq!(
        effective.unreadable_roots,
        policy.get_unreadable_roots_with_cwd(cwd.as_path())
    );
    assert_eq!(
        effective
            .unreadable_globs
            .iter()
            .map(|glob| glob.pattern().to_string())
            .collect::<Vec<_>>(),
        policy.get_unreadable_globs_with_cwd(cwd.as_path())
    );
    assert_eq!(
        effective.include_platform_defaults,
        policy.include_platform_defaults()
    );
    assert_eq!(effective.glob_scan_max_depth, policy.glob_scan_max_depth);
    assert_eq!(
        effective.has_full_disk_read_access(),
        policy.has_full_disk_read_access()
    );
    assert_eq!(
        effective.has_full_disk_write_access(),
        policy.has_full_disk_write_access()
    );

    effective
}

#[test]
fn effective_access_modes_preserve_builtin_profile_semantics() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = absolute_path(temp_dir.path());

    let read_only = assert_projected_fields_match_policy(&PermissionProfile::read_only(), &cwd);
    assert_eq!(read_only.mode, FilesystemPermissionsMode::Restricted);
    assert_eq!(read_only.can_read(cwd.as_path()), true);
    assert_eq!(read_only.can_write(cwd.as_path()), false);

    let unrestricted = assert_projected_fields_match_policy(&PermissionProfile::Disabled, &cwd);
    assert_eq!(unrestricted.mode, FilesystemPermissionsMode::Unrestricted);
    assert_eq!(unrestricted.can_write(cwd.as_path()), true);

    let external = assert_projected_fields_match_policy(
        &PermissionProfile::External {
            network: NetworkSandboxPolicy::Restricted,
        },
        &cwd,
    );
    assert_eq!(external.mode, FilesystemPermissionsMode::External);
    assert_eq!(external.has_full_disk_write_access(), true);
}

#[test]
fn effective_workspace_permissions_reject_unmaterialized_runtime_workspace_roots() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = absolute_path(temp_dir.path());
    let unresolved_exact = PermissionProfile::workspace_write();
    let unresolved_glob_policy =
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: project_roots_glob_pattern(Path::new("**/*.env")),
            },
            access: FileSystemAccessMode::Deny,
        }]);
    let unresolved_glob = PermissionProfile::from_runtime_permissions(
        &unresolved_glob_policy,
        NetworkSandboxPolicy::Restricted,
    );

    for profile in [&unresolved_exact, &unresolved_glob] {
        let error = EffectiveFilesystemPermissions::from_profile(
            profile,
            FilesystemPermissionsContext {
                policy_evaluation_cwd: &cwd,
            },
        )
        .expect_err("unresolved runtime workspace roots should fail");
        assert_eq!(
            error,
            FilesystemPermissionsError::UnmaterializedWorkspaceRoots
        );
    }
}

#[test]
fn effective_workspace_permissions_preserve_materialized_workspace_roots() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = absolute_path(temp_dir.path());
    let first_root = cwd.join("first");
    let second_root = cwd.join("second");
    let permission_profile = PermissionProfile::workspace_write()
        .materialize_project_roots_with_workspace_roots(&[first_root.clone(), second_root.clone()]);
    let effective = assert_projected_fields_match_policy(&permission_profile, &cwd);

    assert_eq!(effective.can_write(first_root.join("src").as_path()), true);
    assert_eq!(effective.can_write(second_root.join("src").as_path()), true);
}

#[test]
fn effective_permissions_preserve_nested_carveouts_and_read_denies() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = absolute_path(temp_dir.path());
    let workspace = cwd.join("workspace");
    let read_only_child = workspace.join("generated");
    let denied_child = workspace.join("private");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: workspace.clone(),
            },
            access: FileSystemAccessMode::Write,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: read_only_child.clone(),
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: denied_child.clone(),
            },
            access: FileSystemAccessMode::Deny,
        },
    ]);
    let permission_profile =
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted);
    let effective = assert_projected_fields_match_policy(&permission_profile, &cwd);

    assert_eq!(
        effective.can_write(workspace.join("file.txt").as_path()),
        true
    );
    assert_eq!(
        effective.can_write(read_only_child.join("file.txt").as_path()),
        false
    );
    assert_eq!(effective.can_write(workspace.join(".git").as_path()), false);
    assert_eq!(
        effective.can_read(denied_child.join("secret.txt").as_path()),
        false
    );
}

#[cfg(unix)]
#[test]
fn effective_permissions_preserve_symlinked_writable_roots() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = absolute_path(temp_dir.path());
    let target = cwd.join("target");
    let link = cwd.join("linked-workspace");
    std::fs::create_dir_all(target.as_path()).expect("create target");
    std::os::unix::fs::symlink(target.as_path(), link.as_path()).expect("create symlink");
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Path { path: link.clone() },
        access: FileSystemAccessMode::Write,
    }]);
    let permission_profile =
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted);
    let effective = assert_projected_fields_match_policy(&permission_profile, &cwd);

    assert_eq!(
        effective.can_write(link.join("file.txt").as_path()),
        policy.can_write_path_with_cwd(link.join("file.txt").as_path(), cwd.as_path())
    );
}

#[test]
fn effective_permissions_preserve_accepted_deny_glob_matching() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = absolute_path(temp_dir.path());
    let pattern = cwd.join("secret[.txt").to_string_lossy().into_owned();
    let denied_path = cwd.join("secret[.txt");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: cwd.clone() },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern { pattern },
            access: FileSystemAccessMode::Deny,
        },
    ]);
    let permission_profile =
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted);
    let effective = assert_projected_fields_match_policy(&permission_profile, &cwd);
    let current_matcher = ReadDenyMatcher::try_new(&policy, cwd.as_path())
        .expect("accepted pattern")
        .expect("deny matcher");

    assert_eq!(effective.is_read_denied(denied_path.as_path()), true);
    assert_eq!(
        effective.is_read_denied(denied_path.as_path()),
        current_matcher.is_read_denied(denied_path.as_path())
    );
    assert_eq!(effective.can_read(denied_path.as_path()), false);
}

#[test]
fn effective_permissions_fail_closed_for_malformed_deny_globs() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = absolute_path(temp_dir.path());
    let readable_path = cwd.join("readable.txt");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: cwd.clone() },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: format!("{}/**/[z-a]", cwd.as_path().display()),
            },
            access: FileSystemAccessMode::Deny,
        },
    ]);
    let permission_profile =
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted);
    let effective = derive_effective(&permission_profile, &cwd);

    assert_eq!(effective.is_read_denied(readable_path.as_path()), true);
    assert_eq!(effective.can_read(readable_path.as_path()), false);
}
