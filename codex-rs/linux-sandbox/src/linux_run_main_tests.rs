#[cfg(test)]
use super::*;
#[cfg(test)]
use codex_protocol::models::PermissionProfile;
#[cfg(test)]
use codex_protocol::protocol::FileSystemSandboxPolicy;
#[cfg(test)]
use codex_protocol::protocol::NetworkSandboxPolicy;
#[cfg(test)]
use codex_utils_absolute_path::AbsolutePathBuf;
#[cfg(test)]
use pretty_assertions::assert_eq;

fn read_only_permission_profile() -> PermissionProfile {
    PermissionProfile::read_only()
}

fn read_only_file_system_policy() -> FileSystemSandboxPolicy {
    read_only_permission_profile().file_system_sandbox_policy()
}

#[test]
fn detects_proc_mount_invalid_argument_failure() {
    let stderr = "bwrap: Can't mount proc on /newroot/proc: Invalid argument";
    assert!(is_proc_mount_failure(stderr));
}

#[test]
fn detects_proc_mount_operation_not_permitted_failure() {
    let stderr = "bwrap: Can't mount proc on /newroot/proc: Operation not permitted";
    assert!(is_proc_mount_failure(stderr));
}

#[test]
fn detects_proc_mount_permission_denied_failure() {
    let stderr = "bwrap: Can't mount proc on /newroot/proc: Permission denied";
    assert!(is_proc_mount_failure(stderr));
}

#[test]
fn ignores_non_proc_mount_errors() {
    let stderr = "bwrap: Can't bind mount /dev/null: Operation not permitted";
    assert!(!is_proc_mount_failure(stderr));
}

#[test]
fn inserts_bwrap_argv0_before_command_separator() {
    let file_system_sandbox_policy = read_only_file_system_policy();
    let mut argv = build_bwrap_argv(
        vec!["/bin/true".to_string()],
        &file_system_sandbox_policy,
        Path::new("/"),
        Path::new("/"),
        BwrapOptions {
            mount_proc: true,
            network_mode: BwrapNetworkMode::FullAccess,
            ..Default::default()
        },
    )
    .args;
    apply_inner_command_argv0_for_launcher(
        &mut argv,
        /*supports_argv0*/ true,
        "/tmp/codex-arg0-session/codex-linux-sandbox".to_string(),
    );
    assert_eq!(
        argv,
        vec![
            "bwrap".to_string(),
            "--new-session".to_string(),
            "--die-with-parent".to_string(),
            "--ro-bind".to_string(),
            "/".to_string(),
            "/".to_string(),
            "--dev".to_string(),
            "/dev".to_string(),
            "--unshare-user".to_string(),
            "--unshare-pid".to_string(),
            "--proc".to_string(),
            "/proc".to_string(),
            "--argv0".to_string(),
            "codex-linux-sandbox".to_string(),
            "--".to_string(),
            "/bin/true".to_string(),
        ]
    );
}

#[test]
fn rewrites_inner_command_path_when_bwrap_lacks_argv0() {
    let file_system_sandbox_policy = read_only_file_system_policy();
    let mut argv = build_bwrap_argv(
        vec!["/bin/true".to_string()],
        &file_system_sandbox_policy,
        Path::new("/"),
        Path::new("/"),
        BwrapOptions {
            mount_proc: true,
            network_mode: BwrapNetworkMode::FullAccess,
            ..Default::default()
        },
    )
    .args;
    apply_inner_command_argv0_for_launcher(
        &mut argv,
        /*supports_argv0*/ false,
        "/tmp/codex-arg0-session/codex-linux-sandbox".to_string(),
    );

    assert!(!argv.iter().any(|arg| arg == "--argv0"));
    assert!(
        argv.windows(2)
            .any(|window| { window == ["--", "/tmp/codex-arg0-session/codex-linux-sandbox"] })
    );
}

#[test]
fn rewrites_bwrap_helper_command_not_nested_user_command_when_current_exe_appears_later() {
    let nested_current_exe = std::env::current_exe()
        .expect("current exe")
        .to_string_lossy()
        .into_owned();
    let mut argv = vec![
        "bwrap".to_string(),
        "--".to_string(),
        "/tmp/helper-symlink".to_string(),
        "--sandbox-policy-cwd".to_string(),
        "/tmp/cwd".to_string(),
        "--".to_string(),
        nested_current_exe.clone(),
        "--codex-run-as-apply-patch".to_string(),
        "patch".to_string(),
    ];

    apply_inner_command_argv0_for_launcher(
        &mut argv,
        /*supports_argv0*/ false,
        "/tmp/argv0-fallback-helper".to_string(),
    );

    assert_eq!(
        argv,
        vec![
            "bwrap".to_string(),
            "--".to_string(),
            "/tmp/argv0-fallback-helper".to_string(),
            "--sandbox-policy-cwd".to_string(),
            "/tmp/cwd".to_string(),
            "--".to_string(),
            nested_current_exe,
            "--codex-run-as-apply-patch".to_string(),
            "patch".to_string(),
        ]
    );
}

#[test]
fn inserts_unshare_net_when_network_isolation_requested() {
    let file_system_sandbox_policy = read_only_file_system_policy();
    let argv = build_bwrap_argv(
        vec!["/bin/true".to_string()],
        &file_system_sandbox_policy,
        Path::new("/"),
        Path::new("/"),
        BwrapOptions {
            mount_proc: true,
            network_mode: BwrapNetworkMode::Isolated,
            ..Default::default()
        },
    )
    .args;
    assert!(argv.contains(&"--unshare-net".to_string()));
}

#[test]
fn inserts_unshare_net_when_proxy_only_network_mode_requested() {
    let file_system_sandbox_policy = read_only_file_system_policy();
    let argv = build_bwrap_argv(
        vec!["/bin/true".to_string()],
        &file_system_sandbox_policy,
        Path::new("/"),
        Path::new("/"),
        BwrapOptions {
            mount_proc: true,
            network_mode: BwrapNetworkMode::ProxyOnly,
            ..Default::default()
        },
    )
    .args;
    assert!(argv.contains(&"--unshare-net".to_string()));
}

#[test]
fn proxy_only_mode_takes_precedence_over_full_network_policy() {
    let mode = bwrap_network_mode(
        NetworkSandboxPolicy::Enabled,
        /*allow_network_for_proxy*/ true,
    );
    assert_eq!(mode, BwrapNetworkMode::ProxyOnly);
}

#[test]
fn split_only_filesystem_policy_requires_direct_runtime_enforcement() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let docs = temp_dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("create docs");
    let docs = AbsolutePathBuf::from_absolute_path(&docs).expect("absolute docs");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        codex_protocol::permissions::FileSystemSandboxEntry {
            path: codex_protocol::permissions::FileSystemPath::Special {
                value: codex_protocol::permissions::FileSystemSpecialPath::project_roots(
                    /*subpath*/ None,
                ),
            },
            access: codex_protocol::permissions::FileSystemAccessMode::Write,
        },
        codex_protocol::permissions::FileSystemSandboxEntry {
            path: codex_protocol::permissions::FileSystemPath::Path { path: docs },
            access: codex_protocol::permissions::FileSystemAccessMode::Read,
        },
    ]);

    assert!(
        policy.needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, temp_dir.path(),)
    );
}

#[test]
fn root_write_read_only_carveout_requires_direct_runtime_enforcement() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let docs = temp_dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("create docs");
    let docs = AbsolutePathBuf::from_absolute_path(&docs).expect("absolute docs");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        codex_protocol::permissions::FileSystemSandboxEntry {
            path: codex_protocol::permissions::FileSystemPath::Special {
                value: codex_protocol::permissions::FileSystemSpecialPath::Root,
            },
            access: codex_protocol::permissions::FileSystemAccessMode::Write,
        },
        codex_protocol::permissions::FileSystemSandboxEntry {
            path: codex_protocol::permissions::FileSystemPath::Path { path: docs },
            access: codex_protocol::permissions::FileSystemAccessMode::Read,
        },
    ]);

    assert!(
        policy.needs_direct_runtime_enforcement(NetworkSandboxPolicy::Restricted, temp_dir.path(),)
    );
}

#[test]
fn managed_proxy_preflight_argv_is_wrapped_for_full_access_policy() {
    let mode = bwrap_network_mode(
        NetworkSandboxPolicy::Enabled,
        /*allow_network_for_proxy*/ true,
    );
    let argv = build_preflight_bwrap_argv(
        Path::new("/"),
        Path::new("/"),
        &FileSystemSandboxPolicy::unrestricted(),
        mode,
    )
    .args;
    assert!(argv.iter().any(|arg| arg == "--"));
}

#[test]
fn managed_proxy_inner_command_includes_route_spec() {
    let permission_profile = read_only_permission_profile();
    let args = build_inner_seccomp_command(InnerSeccompCommandArgs {
        sandbox_policy_cwd: Path::new("/tmp"),
        command_cwd: Some(Path::new("/tmp/link")),
        permission_profile: &permission_profile,
        allow_network_for_proxy: true,
        proxy_route_spec: Some("{\"routes\":[]}".to_string()),
        command: vec!["/bin/true".to_string()],
    });

    assert!(args.iter().any(|arg| arg == "--proxy-route-spec"));
    assert!(args.iter().any(|arg| arg == "{\"routes\":[]}"));
}

#[test]
fn inner_command_includes_permission_profile_flag() {
    let permission_profile = read_only_permission_profile();
    let args = build_inner_seccomp_command(InnerSeccompCommandArgs {
        sandbox_policy_cwd: Path::new("/tmp"),
        command_cwd: Some(Path::new("/tmp/link")),
        permission_profile: &permission_profile,
        allow_network_for_proxy: false,
        proxy_route_spec: None,
        command: vec!["/bin/true".to_string()],
    });

    assert!(args.iter().any(|arg| arg == "--permission-profile"));
    assert!(
        args.windows(2)
            .any(|window| { window == ["--command-cwd", "/tmp/link"] })
    );
}

#[test]
fn non_managed_inner_command_omits_route_spec() {
    let permission_profile = read_only_permission_profile();
    let args = build_inner_seccomp_command(InnerSeccompCommandArgs {
        sandbox_policy_cwd: Path::new("/tmp"),
        command_cwd: Some(Path::new("/tmp/link")),
        permission_profile: &permission_profile,
        allow_network_for_proxy: false,
        proxy_route_spec: None,
        command: vec!["/bin/true".to_string()],
    });

    assert!(!args.iter().any(|arg| arg == "--proxy-route-spec"));
}

#[test]
fn managed_proxy_inner_command_requires_route_spec() {
    let result = std::panic::catch_unwind(|| {
        let permission_profile = read_only_permission_profile();
        build_inner_seccomp_command(InnerSeccompCommandArgs {
            sandbox_policy_cwd: Path::new("/tmp"),
            command_cwd: Some(Path::new("/tmp/link")),
            permission_profile: &permission_profile,
            allow_network_for_proxy: true,
            proxy_route_spec: None,
            command: vec!["/bin/true".to_string()],
        })
    });
    assert!(result.is_err());
}

#[test]
fn resolve_permission_profile_derives_runtime_policies() {
    let permission_profile = read_only_permission_profile();
    let resolved = resolve_permission_profile(Some(permission_profile.clone()))
        .expect("profile should resolve");

    assert_eq!(resolved.permission_profile, permission_profile);
    assert_eq!(
        resolved.file_system_sandbox_policy,
        read_only_file_system_policy()
    );
    assert_eq!(
        resolved.network_sandbox_policy,
        NetworkSandboxPolicy::Restricted
    );
}

#[test]
fn resolve_permission_profile_preserves_direct_runtime_profile() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let docs = temp_dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("create docs");
    let docs = AbsolutePathBuf::from_absolute_path(&docs).expect("absolute docs");
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        codex_protocol::permissions::FileSystemSandboxEntry {
            path: codex_protocol::permissions::FileSystemPath::Special {
                value: codex_protocol::permissions::FileSystemSpecialPath::Root,
            },
            access: codex_protocol::permissions::FileSystemAccessMode::Read,
        },
        codex_protocol::permissions::FileSystemSandboxEntry {
            path: codex_protocol::permissions::FileSystemPath::Path { path: docs },
            access: codex_protocol::permissions::FileSystemAccessMode::Write,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );
    let resolved = resolve_permission_profile(Some(permission_profile.clone()))
        .expect("profile should resolve");

    assert_eq!(resolved.permission_profile, permission_profile);
    assert_eq!(
        resolved.file_system_sandbox_policy,
        file_system_sandbox_policy
    );
    assert_eq!(
        resolved.network_sandbox_policy,
        NetworkSandboxPolicy::Restricted
    );
}

#[test]
fn resolve_permission_profile_rejects_missing_configuration() {
    let err = resolve_permission_profile(/*permission_profile*/ None)
        .expect_err("missing profile should fail");

    assert_eq!(err, ResolvePermissionProfileError::MissingConfiguration);
}

#[test]
fn apply_seccomp_then_exec_with_legacy_landlock_panics() {
    let result = std::panic::catch_unwind(|| {
        ensure_inner_stage_mode_is_valid(
            /*apply_seccomp_then_exec*/ true, /*use_legacy_landlock*/ true,
        )
    });
    assert!(result.is_err());
}

#[test]
fn legacy_landlock_rejects_split_only_filesystem_policies() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let docs = temp_dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("create docs");
    let docs = AbsolutePathBuf::from_absolute_path(&docs).expect("absolute docs");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        codex_protocol::permissions::FileSystemSandboxEntry {
            path: codex_protocol::permissions::FileSystemPath::Special {
                value: codex_protocol::permissions::FileSystemSpecialPath::Root,
            },
            access: codex_protocol::permissions::FileSystemAccessMode::Read,
        },
        codex_protocol::permissions::FileSystemSandboxEntry {
            path: codex_protocol::permissions::FileSystemPath::Path { path: docs },
            access: codex_protocol::permissions::FileSystemAccessMode::Write,
        },
    ]);

    let result = std::panic::catch_unwind(|| {
        ensure_legacy_landlock_mode_supports_policy(
            /*use_legacy_landlock*/ true,
            &policy,
            NetworkSandboxPolicy::Restricted,
            temp_dir.path(),
        );
    });

    assert!(result.is_err());
}

#[test]
fn valid_inner_stage_modes_do_not_panic() {
    ensure_inner_stage_mode_is_valid(
        /*apply_seccomp_then_exec*/ false, /*use_legacy_landlock*/ false,
    );
    ensure_inner_stage_mode_is_valid(
        /*apply_seccomp_then_exec*/ false, /*use_legacy_landlock*/ true,
    );
    ensure_inner_stage_mode_is_valid(
        /*apply_seccomp_then_exec*/ true, /*use_legacy_landlock*/ false,
    );
}
