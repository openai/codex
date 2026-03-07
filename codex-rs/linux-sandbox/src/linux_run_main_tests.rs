#[cfg(test)]
use super::*;
#[cfg(test)]
use codex_protocol::protocol::FileSystemSandboxPolicy;
#[cfg(test)]
use codex_protocol::protocol::NetworkSandboxPolicy;
#[cfg(test)]
use codex_protocol::protocol::SandboxPolicy;
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
    );
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
    );
    assert!(argv.contains(&"--unshare-net".to_string()));
}

#[test]
fn proxy_only_mode_takes_precedence_over_full_network_policy() {
    let mode = bwrap_network_mode(NetworkSandboxPolicy::Enabled, true);
    assert_eq!(mode, BwrapNetworkMode::ProxyOnly);
}

#[test]
fn managed_proxy_preflight_argv_is_wrapped_for_full_access_policy() {
    let mode = bwrap_network_mode(NetworkSandboxPolicy::Enabled, true);
    let argv = build_preflight_bwrap_argv(
        Path::new("/"),
        &FileSystemSandboxPolicy::from(&SandboxPolicy::DangerFullAccess),
        mode,
    );
    assert!(argv.iter().any(|arg| arg == "--"));
}

#[test]
fn managed_proxy_inner_command_includes_route_spec() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let args = build_inner_seccomp_command(
        Path::new("/tmp"),
        &sandbox_policy,
        &FileSystemSandboxPolicy::from(&sandbox_policy),
        NetworkSandboxPolicy::Restricted,
        true,
        true,
        Some("{\"routes\":[]}".to_string()),
        vec!["/bin/true".to_string()],
    );

    assert!(args.iter().any(|arg| arg == "--proxy-route-spec"));
    assert!(args.iter().any(|arg| arg == "{\"routes\":[]}"));
}

#[test]
fn inner_command_includes_split_policy_flags() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let args = build_inner_seccomp_command(
        Path::new("/tmp"),
        &sandbox_policy,
        &FileSystemSandboxPolicy::from(&sandbox_policy),
        NetworkSandboxPolicy::Restricted,
        true,
        false,
        None,
        vec!["/bin/true".to_string()],
    );

    assert!(args.iter().any(|arg| arg == "--file-system-sandbox-policy"));
    assert!(args.iter().any(|arg| arg == "--network-sandbox-policy"));
}

#[test]
fn non_managed_inner_command_omits_route_spec() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let args = build_inner_seccomp_command(
        Path::new("/tmp"),
        &sandbox_policy,
        &FileSystemSandboxPolicy::from(&sandbox_policy),
        NetworkSandboxPolicy::Restricted,
        true,
        false,
        None,
        vec!["/bin/true".to_string()],
    );

    assert!(!args.iter().any(|arg| arg == "--proxy-route-spec"));
}

#[test]
fn managed_proxy_inner_command_requires_route_spec() {
    let result = std::panic::catch_unwind(|| {
        let sandbox_policy = SandboxPolicy::new_read_only_policy();
        build_inner_seccomp_command(
            Path::new("/tmp"),
            &sandbox_policy,
            &FileSystemSandboxPolicy::from(&sandbox_policy),
            NetworkSandboxPolicy::Restricted,
            true,
            true,
            None,
            vec!["/bin/true".to_string()],
        )
    });
    assert!(result.is_err());
}

#[test]
fn resolve_sandbox_policies_derives_split_policies_from_legacy_policy() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();

    let resolved =
        resolve_sandbox_policies(Path::new("/tmp"), Some(sandbox_policy.clone()), None, None);

    assert_eq!(resolved.sandbox_policy, sandbox_policy.clone());
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
    );

    assert_eq!(resolved.sandbox_policy, sandbox_policy);
    assert_eq!(
        resolved.file_system_sandbox_policy,
        file_system_sandbox_policy
    );
    assert_eq!(resolved.network_sandbox_policy, network_sandbox_policy);
}

#[test]
fn resolve_sandbox_policies_rejects_partial_split_policies() {
    let result = std::panic::catch_unwind(|| {
        resolve_sandbox_policies(
            Path::new("/tmp"),
            Some(SandboxPolicy::new_read_only_policy()),
            Some(FileSystemSandboxPolicy::default()),
            None,
        )
    });

    assert!(result.is_err());
}

#[test]
fn apply_seccomp_then_exec_without_bwrap_panics() {
    let result = std::panic::catch_unwind(|| ensure_inner_stage_mode_is_valid(true, false));
    assert!(result.is_err());
}

#[test]
fn valid_inner_stage_modes_do_not_panic() {
    ensure_inner_stage_mode_is_valid(false, false);
    ensure_inner_stage_mode_is_valid(false, true);
    ensure_inner_stage_mode_is_valid(true, true);
}
