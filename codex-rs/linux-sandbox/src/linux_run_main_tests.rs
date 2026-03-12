#[cfg(test)]
use super::*;
#[cfg(test)]
use codex_protocol::protocol::FileSystemSandboxPolicy;
#[cfg(test)]
use codex_protocol::protocol::NetworkSandboxPolicy;
#[cfg(test)]
use codex_protocol::protocol::ReadOnlyAccess;
#[cfg(test)]
use codex_protocol::protocol::SandboxPolicy;
#[cfg(test)]
use codex_utils_absolute_path::AbsolutePathBuf;
#[cfg(test)]
use pretty_assertions::assert_eq;

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
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let argv = build_bwrap_argv(
        vec!["/bin/true".to_string()],
        &FileSystemSandboxPolicy::from(&sandbox_policy),
        Path::new("/"),
        BwrapOptions {
            mount_proc: true,
            network_mode: BwrapNetworkMode::FullAccess,
        },
    )
    .args;
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
fn inserts_unshare_net_when_network_isolation_requested() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let argv = build_bwrap_argv(
        vec!["/bin/true".to_string()],
        &FileSystemSandboxPolicy::from(&sandbox_policy),
        Path::new("/"),
        BwrapOptions {
            mount_proc: true,
            network_mode: BwrapNetworkMode::Isolated,
        },
    )
    .args;
    assert!(argv.contains(&"--unshare-net".to_string()));
}

#[test]
fn inserts_unshare_net_when_proxy_only_network_mode_requested() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let argv = build_bwrap_argv(
        vec!["/bin/true".to_string()],
        &FileSystemSandboxPolicy::from(&sandbox_policy),
        Path::new("/"),
        BwrapOptions {
            mount_proc: true,
            network_mode: BwrapNetworkMode::ProxyOnly,
        },
    )
    .args;
    assert!(argv.contains(&"--unshare-net".to_string()));
}

#[test]
fn proxy_only_mode_takes_precedence_over_full_network_policy() {
    let mode = bwrap_network_mode(NetworkSandboxPolicy::Enabled, true);
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
                value: codex_protocol::permissions::FileSystemSpecialPath::CurrentWorkingDirectory,
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
    let mode = bwrap_network_mode(NetworkSandboxPolicy::Enabled, true);
    let argv = build_preflight_bwrap_argv(
        Path::new("/"),
        &FileSystemSandboxPolicy::from(&SandboxPolicy::DangerFullAccess),
        mode,
    )
    .args;
    assert!(argv.iter().any(|arg| arg == "--"));
}

#[test]
fn managed_proxy_inner_command_includes_route_spec() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let args = build_inner_seccomp_command(InnerSeccompCommandArgs {
        sandbox_policy_cwd: Path::new("/tmp"),
        sandbox_policy: &sandbox_policy,
        file_system_sandbox_policy: &FileSystemSandboxPolicy::from(&sandbox_policy),
        network_sandbox_policy: NetworkSandboxPolicy::Restricted,
        allow_network_for_proxy: true,
        proxy_route_spec: Some("{\"routes\":[]}".to_string()),
        command: vec!["/bin/true".to_string()],
    });

    assert!(args.iter().any(|arg| arg == "--proxy-route-spec"));
    assert!(args.iter().any(|arg| arg == "{\"routes\":[]}"));
}

#[test]
fn inner_command_includes_split_policy_flags() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let args = build_inner_seccomp_command(InnerSeccompCommandArgs {
        sandbox_policy_cwd: Path::new("/tmp"),
        sandbox_policy: &sandbox_policy,
        file_system_sandbox_policy: &FileSystemSandboxPolicy::from(&sandbox_policy),
        network_sandbox_policy: NetworkSandboxPolicy::Restricted,
        allow_network_for_proxy: false,
        proxy_route_spec: None,
        command: vec!["/bin/true".to_string()],
    });

    assert!(args.iter().any(|arg| arg == "--file-system-sandbox-policy"));
    assert!(args.iter().any(|arg| arg == "--network-sandbox-policy"));
}

#[test]
fn non_managed_inner_command_omits_route_spec() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let args = build_inner_seccomp_command(InnerSeccompCommandArgs {
        sandbox_policy_cwd: Path::new("/tmp"),
        sandbox_policy: &sandbox_policy,
        file_system_sandbox_policy: &FileSystemSandboxPolicy::from(&sandbox_policy),
        network_sandbox_policy: NetworkSandboxPolicy::Restricted,
        allow_network_for_proxy: false,
        proxy_route_spec: None,
        command: vec!["/bin/true".to_string()],
    });

    assert!(!args.iter().any(|arg| arg == "--proxy-route-spec"));
}

#[test]
fn managed_proxy_inner_command_requires_route_spec() {
    let result = std::panic::catch_unwind(|| {
        let sandbox_policy = SandboxPolicy::new_read_only_policy();
        build_inner_seccomp_command(InnerSeccompCommandArgs {
            sandbox_policy_cwd: Path::new("/tmp"),
            sandbox_policy: &sandbox_policy,
            file_system_sandbox_policy: &FileSystemSandboxPolicy::from(&sandbox_policy),
            network_sandbox_policy: NetworkSandboxPolicy::Restricted,
            allow_network_for_proxy: true,
            proxy_route_spec: None,
            command: vec!["/bin/true".to_string()],
        })
    });
    assert!(result.is_err());
}

#[test]
fn resolve_sandbox_policies_derives_split_policies_from_legacy_policy() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();

    let resolved =
        resolve_sandbox_policies(Path::new("/tmp"), Some(sandbox_policy.clone()), None, None)
            .expect("legacy policy should resolve");

    assert_eq!(resolved.sandbox_policy, sandbox_policy);
    assert_eq!(
        resolved.file_system_sandbox_policy,
        FileSystemSandboxPolicy::from(&sandbox_policy)
    );
    assert_eq!(
        resolved.network_sandbox_policy,
        NetworkSandboxPolicy::from(&sandbox_policy)
    );
}

#[test]
fn resolve_sandbox_policies_derives_legacy_policy_from_split_policies() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let file_system_sandbox_policy = FileSystemSandboxPolicy::from(&sandbox_policy);
    let network_sandbox_policy = NetworkSandboxPolicy::from(&sandbox_policy);

    let resolved = resolve_sandbox_policies(
        Path::new("/tmp"),
        None,
        Some(file_system_sandbox_policy.clone()),
        Some(network_sandbox_policy),
    )
    .expect("split policies should resolve");

    assert_eq!(resolved.sandbox_policy, sandbox_policy);
    assert_eq!(
        resolved.file_system_sandbox_policy,
        file_system_sandbox_policy
    );
    assert_eq!(resolved.network_sandbox_policy, network_sandbox_policy);
}

#[test]
fn resolve_sandbox_policies_rejects_partial_split_policies() {
    let err = resolve_sandbox_policies(
        Path::new("/tmp"),
        Some(SandboxPolicy::new_read_only_policy()),
        Some(FileSystemSandboxPolicy::default()),
        None,
    )
    .expect_err("partial split policies should fail");

    assert_eq!(err, ResolveSandboxPoliciesError::PartialSplitPolicies);
}

#[test]
fn resolve_sandbox_policies_rejects_mismatched_legacy_and_split_inputs() {
    let err = resolve_sandbox_policies(
        Path::new("/tmp"),
        Some(SandboxPolicy::new_read_only_policy()),
        Some(FileSystemSandboxPolicy::unrestricted()),
        Some(NetworkSandboxPolicy::Enabled),
    )
    .expect_err("mismatched legacy and split policies should fail");
    assert!(
        matches!(
            err,
            ResolveSandboxPoliciesError::MismatchedLegacyPolicy { .. }
        ),
        "{err}"
    );
}

#[test]
fn resolve_sandbox_policies_accepts_split_policies_requiring_direct_runtime_enforcement() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let docs = temp_dir.path().join("docs");
    std::fs::create_dir_all(&docs).expect("create docs");
    let docs = AbsolutePathBuf::from_absolute_path(&docs).expect("absolute docs");
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
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

    let resolved = resolve_sandbox_policies(
        temp_dir.path(),
        Some(sandbox_policy.clone()),
        Some(file_system_sandbox_policy.clone()),
        Some(NetworkSandboxPolicy::Restricted),
    )
    .expect("split-only policy should preserve provided legacy fallback");

    assert_eq!(resolved.sandbox_policy, sandbox_policy);
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
fn resolve_sandbox_policies_accepts_semantically_equivalent_workspace_write_inputs() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let workspace = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let workspace = AbsolutePathBuf::from_absolute_path(&workspace).expect("absolute workspace");
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![workspace],
        read_only_access: ReadOnlyAccess::FullAccess,
        network_access: false,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    };
    let file_system_sandbox_policy =
        FileSystemSandboxPolicy::from(&SandboxPolicy::new_workspace_write_policy());

    let resolved = resolve_sandbox_policies(
        temp_dir.path().join("workspace").as_path(),
        Some(sandbox_policy.clone()),
        Some(file_system_sandbox_policy.clone()),
        Some(NetworkSandboxPolicy::Restricted),
    )
    .expect("semantically equivalent legacy workspace-write policy should resolve");

    assert_eq!(resolved.sandbox_policy, sandbox_policy);
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
fn apply_seccomp_then_exec_with_legacy_landlock_panics() {
    let result = std::panic::catch_unwind(|| ensure_inner_stage_mode_is_valid(true, true));
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
            true,
            &policy,
            NetworkSandboxPolicy::Restricted,
            temp_dir.path(),
        );
    });

    assert!(result.is_err());
}

#[test]
fn valid_inner_stage_modes_do_not_panic() {
    ensure_inner_stage_mode_is_valid(false, false);
    ensure_inner_stage_mode_is_valid(false, true);
    ensure_inner_stage_mode_is_valid(true, false);
}
