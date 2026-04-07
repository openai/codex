use super::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
use super::MACOS_SEATBELT_BASE_POLICY;
use super::ProxyPolicyInputs;
use super::UnixDomainSocketPolicy;
use super::create_seatbelt_command_args;
use super::create_seatbelt_command_args_for_policies;
use super::dynamic_network_policy;
use super::macos_dir_params;
use super::normalize_path_for_sandbox;
use super::unix_socket_dir_params;
use super::unix_socket_policy;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn assert_seatbelt_denied(stderr: &[u8], path: &Path) {
    let stderr = String::from_utf8_lossy(stderr);
    let expected = format!("bash: {}: Operation not permitted\n", path.display());
    assert!(
        stderr == expected
            || stderr.contains("sandbox-exec: sandbox_apply: Operation not permitted"),
        "unexpected stderr: {stderr}"
    );
}

fn absolute_path(path: &str) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(Path::new(path)).expect("absolute path")
}

fn seatbelt_policy_arg(args: &[String]) -> &str {
    let policy_index = args
        .iter()
        .position(|arg| arg == "-p")
        .expect("seatbelt args should include -p");
    args.get(policy_index + 1)
        .expect("seatbelt args should include policy text")
}

#[test]
fn base_policy_allows_node_cpu_sysctls() {
    assert!(
        MACOS_SEATBELT_BASE_POLICY.contains("(sysctl-name \"machdep.cpu.brand_string\")"),
        "base policy must allow CPU brand lookup for os.cpus()"
    );
    assert!(
        MACOS_SEATBELT_BASE_POLICY.contains("(sysctl-name \"hw.model\")"),
        "base policy must allow hardware model lookup for os.cpus()"
    );
}

#[test]
fn base_policy_allows_kmp_registration_shm_read_create_and_unlink() {
    let expected = r##"(allow ipc-posix-shm-read-data
  ipc-posix-shm-write-create
  ipc-posix-shm-write-unlink
  (ipc-posix-name-regex #"^/__KMP_REGISTERED_LIB_[0-9]+$"))"##;

    assert!(
        MACOS_SEATBELT_BASE_POLICY.contains(expected),
        "base policy must allow only KMP registration shm read/create/unlink:\n{MACOS_SEATBELT_BASE_POLICY}"
    );
}

#[test]
fn create_seatbelt_args_routes_network_through_proxy_ports() {
    let policy = dynamic_network_policy(
        &SandboxPolicy::new_read_only_policy(),
        /*enforce_managed_network*/ false,
        &ProxyPolicyInputs {
            ports: vec![43128, 48081],
            has_proxy_config: true,
            allow_local_binding: false,
            ..ProxyPolicyInputs::default()
        },
    );

    assert!(
        policy.contains("(allow network-outbound (remote ip \"localhost:43128\"))"),
        "expected HTTP proxy port allow rule in policy:\n{policy}"
    );
    assert!(
        policy.contains("(allow network-outbound (remote ip \"localhost:48081\"))"),
        "expected SOCKS proxy port allow rule in policy:\n{policy}"
    );
    assert!(
        !policy.contains("\n(allow network-outbound)\n"),
        "policy should not include blanket outbound allowance when proxy ports are present:\n{policy}"
    );
    assert!(
        !policy.contains("(allow network-bind (local ip \"localhost:*\"))"),
        "policy should not allow loopback binding unless explicitly enabled:\n{policy}"
    );
    assert!(
        !policy.contains("(allow network-inbound (local ip \"localhost:*\"))"),
        "policy should not allow loopback inbound unless explicitly enabled:\n{policy}"
    );
}

#[test]
fn explicit_unreadable_paths_are_excluded_from_full_disk_read_and_write_access() {
    let unreadable = absolute_path("/tmp/codex-unreadable");
    let file_system_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: unreadable },
            access: FileSystemAccessMode::None,
        },
    ]);

    let args = create_seatbelt_command_args_for_policies(
        vec!["/bin/true".to_string()],
        &file_system_policy,
        NetworkSandboxPolicy::Restricted,
        Path::new("/"),
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );

    let policy = seatbelt_policy_arg(&args);
    let unreadable_roots = file_system_policy.get_unreadable_roots_with_cwd(Path::new("/"));
    let unreadable_root = unreadable_roots.first().expect("expected unreadable root");
    assert!(
        policy.contains("(require-not (literal (param \"READABLE_ROOT_0_EXCLUDED_0\")))"),
        "expected exact read carveout in policy:\n{policy}"
    );
    assert!(
        policy.contains("(require-not (subpath (param \"READABLE_ROOT_0_EXCLUDED_0\")))"),
        "expected read carveout in policy:\n{policy}"
    );
    assert!(
        policy.contains("(require-not (literal (param \"WRITABLE_ROOT_0_EXCLUDED_0\")))"),
        "expected exact write carveout in policy:\n{policy}"
    );
    assert!(
        policy.contains("(require-not (subpath (param \"WRITABLE_ROOT_0_EXCLUDED_0\")))"),
        "expected write carveout in policy:\n{policy}"
    );
    assert!(
        args.iter().any(
            |arg| arg == &format!("-DREADABLE_ROOT_0_EXCLUDED_0={}", unreadable_root.display())
        ),
        "expected read carveout parameter in args: {args:#?}"
    );
    let writable_definitions: Vec<String> = args
        .iter()
        .filter(|arg| arg.starts_with("-DWRITABLE_ROOT_"))
        .cloned()
        .collect();
    assert_eq!(
        writable_definitions,
        vec![
            "-DWRITABLE_ROOT_0=/".to_string(),
            "-DWRITABLE_ROOT_0_EXCLUDED_0=/.codex".to_string(),
            format!("-DWRITABLE_ROOT_0_EXCLUDED_1={}", unreadable_root.display()),
        ],
        "unexpected write carveout parameters in args: {args:#?}"
    );
}

#[test]
fn explicit_unreadable_paths_are_excluded_from_readable_roots() {
    let root = absolute_path("/tmp/codex-readable");
    let unreadable = absolute_path("/tmp/codex-readable/private");
    let file_system_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: root },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: unreadable },
            access: FileSystemAccessMode::None,
        },
    ]);

    let args = create_seatbelt_command_args_for_policies(
        vec!["/bin/true".to_string()],
        &file_system_policy,
        NetworkSandboxPolicy::Restricted,
        Path::new("/"),
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );

    let policy = seatbelt_policy_arg(&args);
    let readable_roots = file_system_policy.get_readable_roots_with_cwd(Path::new("/"));
    let readable_root = readable_roots.first().expect("expected readable root");
    let unreadable_roots = file_system_policy.get_unreadable_roots_with_cwd(Path::new("/"));
    let unreadable_root = unreadable_roots.first().expect("expected unreadable root");
    assert!(
        policy.contains("(require-not (literal (param \"READABLE_ROOT_0_EXCLUDED_0\")))"),
        "expected exact read carveout in policy:\n{policy}"
    );
    assert!(
        policy.contains("(require-not (subpath (param \"READABLE_ROOT_0_EXCLUDED_0\")))"),
        "expected read carveout in policy:\n{policy}"
    );
    assert!(
        args.iter()
            .any(|arg| arg == &format!("-DREADABLE_ROOT_0={}", readable_root.display())),
        "expected readable root parameter in args: {args:#?}"
    );
    assert!(
        args.iter().any(
            |arg| arg == &format!("-DREADABLE_ROOT_0_EXCLUDED_0={}", unreadable_root.display())
        ),
        "expected read carveout parameter in args: {args:#?}"
    );
}

#[test]
fn seatbelt_args_without_extension_profile_keep_legacy_preferences_read_access() {
    let cwd = std::env::temp_dir();
    let args = create_seatbelt_command_args(
        vec!["echo".to_string(), "ok".to_string()],
        &SandboxPolicy::new_read_only_policy(),
        cwd.as_path(),
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );
    let policy = &args[1];
    assert!(policy.contains("(allow user-preference-read)"));
    assert!(!policy.contains("(allow user-preference-write)"));
}

#[test]
fn seatbelt_legacy_workspace_write_nested_readable_root_stays_writable() {
    let tmp = TempDir::new().expect("tempdir");
    let cwd = tmp.path().join("workspace");
    fs::create_dir_all(cwd.join("docs")).expect("create docs");
    let docs = AbsolutePathBuf::from_absolute_path(cwd.join("docs")).expect("absolute docs");
    let args = create_seatbelt_command_args(
        vec!["/bin/true".to_string()],
        &SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: vec![docs.clone()],
            },
            network_access: false,
            allow_limited_git_writes: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        },
        cwd.as_path(),
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );

    assert!(
        !args
            .iter()
            .any(|arg| arg.ends_with(&format!("={}", docs.as_path().display()))),
        "legacy workspace-write readable roots under cwd should not become seatbelt carveouts:\n{args:#?}",
    );
    assert!(
        args.iter()
            .any(|arg| arg.starts_with("-DWRITABLE_ROOT_0_EXCLUDED_")
                && arg.ends_with("/workspace/.codex")),
        "expected proactive .codex carveout for cwd root: {args:#?}",
    );
}

#[test]
fn create_seatbelt_args_allows_local_binding_when_explicitly_enabled() {
    let policy = dynamic_network_policy(
        &SandboxPolicy::new_read_only_policy(),
        /*enforce_managed_network*/ false,
        &ProxyPolicyInputs {
            ports: vec![43128],
            has_proxy_config: true,
            allow_local_binding: true,
            ..ProxyPolicyInputs::default()
        },
    );

    assert!(
        policy.contains("(allow network-bind (local ip \"localhost:*\"))"),
        "policy should allow loopback local binding when explicitly enabled:\n{policy}"
    );
    assert!(
        policy.contains("(allow network-inbound (local ip \"localhost:*\"))"),
        "policy should allow loopback inbound when explicitly enabled:\n{policy}"
    );
    assert!(
        policy.contains("(allow network-outbound (remote ip \"localhost:*\"))"),
        "policy should allow loopback outbound when explicitly enabled:\n{policy}"
    );
    assert!(
        !policy.contains("\n(allow network-outbound)\n"),
        "policy should keep proxy-routed behavior without blanket outbound allowance:\n{policy}"
    );
}

#[test]
fn dynamic_network_policy_preserves_restricted_policy_when_proxy_config_without_ports() {
    let policy = dynamic_network_policy(
        &SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: true,
            allow_limited_git_writes: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        },
        /*enforce_managed_network*/ false,
        &ProxyPolicyInputs {
            ports: vec![],
            has_proxy_config: true,
            allow_local_binding: false,
            ..ProxyPolicyInputs::default()
        },
    );

    assert!(
        policy.contains("(socket-domain AF_SYSTEM)"),
        "policy should keep the restricted network profile when proxy config is present without ports:\n{policy}"
    );
    assert!(
        !policy.contains("\n(allow network-outbound)\n"),
        "policy should not include blanket outbound allowance when proxy config is present without ports:\n{policy}"
    );
    assert!(
        !policy.contains("(allow network-outbound (remote ip \"localhost:"),
        "policy should not include proxy port allowance when proxy config is present without ports:\n{policy}"
    );
}

#[test]
fn dynamic_network_policy_preserves_restricted_policy_for_managed_network_without_proxy_config() {
    let policy = dynamic_network_policy(
        &SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: true,
            allow_limited_git_writes: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        },
        /*enforce_managed_network*/ true,
        &ProxyPolicyInputs {
            ports: vec![],
            has_proxy_config: false,
            allow_local_binding: false,
            ..ProxyPolicyInputs::default()
        },
    );

    assert!(
        policy.contains("(socket-domain AF_SYSTEM)"),
        "policy should keep the restricted network profile when managed network is active without proxy endpoints:\n{policy}"
    );
    assert!(
        !policy.contains("\n(allow network-outbound)\n"),
        "policy should not include blanket outbound allowance when managed network is active without proxy endpoints:\n{policy}"
    );
}

#[test]
fn create_seatbelt_args_allowlists_unix_socket_paths() {
    let policy = dynamic_network_policy(
        &SandboxPolicy::new_read_only_policy(),
        /*enforce_managed_network*/ false,
        &ProxyPolicyInputs {
            ports: vec![43128],
            has_proxy_config: true,
            allow_local_binding: false,
            unix_domain_socket_policy: UnixDomainSocketPolicy::Restricted {
                allowed: vec![absolute_path("/tmp/example.sock")],
            },
        },
    );

    assert!(
        policy.contains("(allow system-socket (socket-domain AF_UNIX))"),
        "policy should allow AF_UNIX socket creation for configured unix sockets:\n{policy}"
    );
    assert!(
        policy.contains(
            "(allow network-bind (local unix-socket (subpath (param \"UNIX_SOCKET_PATH_0\"))))"
        ),
        "policy should allow binding explicitly configured unix sockets:\n{policy}"
    );
    assert!(
        policy.contains(
            "(allow network-outbound (remote unix-socket (subpath (param \"UNIX_SOCKET_PATH_0\"))))"
        ),
        "policy should allow connecting to explicitly configured unix sockets:\n{policy}"
    );
    assert!(
        !policy.contains("(allow network* (subpath"),
        "policy should no longer use the generic subpath unix-socket rules:\n{policy}"
    );
}

#[test]
fn unix_socket_policy_non_empty_output_is_newline_terminated() {
    let allowlist_policy = unix_socket_policy(&ProxyPolicyInputs {
        unix_domain_socket_policy: UnixDomainSocketPolicy::Restricted {
            allowed: vec![absolute_path("/tmp/example.sock")],
        },
        ..ProxyPolicyInputs::default()
    });
    assert!(
        allowlist_policy.ends_with('\n'),
        "allowlist unix socket policy should end with a newline:\n{allowlist_policy}"
    );

    let allow_all_policy = unix_socket_policy(&ProxyPolicyInputs {
        unix_domain_socket_policy: UnixDomainSocketPolicy::AllowAll,
        ..ProxyPolicyInputs::default()
    });
    assert!(
        allow_all_policy.ends_with('\n'),
        "allow-all unix socket policy should end with a newline:\n{allow_all_policy}"
    );
}

#[test]
fn unix_socket_dir_params_use_stable_param_names() {
    let params = unix_socket_dir_params(&ProxyPolicyInputs {
        unix_domain_socket_policy: UnixDomainSocketPolicy::Restricted {
            allowed: vec![
                absolute_path("/tmp/b.sock"),
                absolute_path("/tmp/a.sock"),
                absolute_path("/tmp/a.sock"),
            ],
        },
        ..ProxyPolicyInputs::default()
    });

    assert_eq!(
        params,
        vec![
            (
                "UNIX_SOCKET_PATH_0".to_string(),
                PathBuf::from("/tmp/a.sock")
            ),
            (
                "UNIX_SOCKET_PATH_1".to_string(),
                PathBuf::from("/tmp/b.sock")
            ),
        ]
    );
}

#[test]
fn normalize_path_for_sandbox_rejects_relative_paths() {
    assert_eq!(normalize_path_for_sandbox(Path::new("relative.sock")), None);
}

#[test]
fn create_seatbelt_args_allows_all_unix_sockets_when_enabled() {
    let policy = dynamic_network_policy(
        &SandboxPolicy::new_read_only_policy(),
        /*enforce_managed_network*/ false,
        &ProxyPolicyInputs {
            ports: vec![43128],
            has_proxy_config: true,
            allow_local_binding: false,
            unix_domain_socket_policy: UnixDomainSocketPolicy::AllowAll,
        },
    );

    assert!(
        policy.contains("(allow system-socket (socket-domain AF_UNIX))"),
        "policy should allow AF_UNIX socket creation when unix sockets are enabled:\n{policy}"
    );
    assert!(
        policy.contains("(allow network-bind (local unix-socket))"),
        "policy should allow binding unix sockets when enabled:\n{policy}"
    );
    assert!(
        policy.contains("(allow network-outbound (remote unix-socket))"),
        "policy should allow connecting to unix sockets when enabled:\n{policy}"
    );
    assert!(
        !policy.contains("(allow network* (subpath"),
        "policy should no longer use the generic subpath unix-socket rules:\n{policy}"
    );
}

#[test]
fn create_seatbelt_args_full_network_with_proxy_is_still_proxy_only() {
    let policy = dynamic_network_policy(
        &SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: true,
            allow_limited_git_writes: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        },
        /*enforce_managed_network*/ false,
        &ProxyPolicyInputs {
            ports: vec![43128],
            has_proxy_config: true,
            allow_local_binding: false,
            ..ProxyPolicyInputs::default()
        },
    );

    assert!(
        policy.contains("(allow network-outbound (remote ip \"localhost:43128\"))"),
        "expected proxy endpoint allow rule in policy:\n{policy}"
    );
    assert!(
        !policy.contains("\n(allow network-outbound)\n"),
        "policy should not include blanket outbound allowance when proxy is configured:\n{policy}"
    );
    assert!(
        !policy.contains("\n(allow network-inbound)\n"),
        "policy should not include blanket inbound allowance when proxy is configured:\n{policy}"
    );
}

#[test]
fn create_seatbelt_args_with_read_only_git_and_codex_subpaths() {
    // Create a temporary workspace with two writable roots: one containing
    // top-level .git and .codex directories and one without them.
    let tmp = TempDir::new().expect("tempdir");
    let PopulatedTmp {
        vulnerable_root,
        vulnerable_root_canonical,
        dot_git_canonical,
        dot_codex_canonical,
        empty_root,
        empty_root_canonical,
    } = populate_tmpdir(tmp.path());
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("create cwd");

    // Build a policy that only includes the two test roots as writable and
    // does not automatically include defaults TMPDIR or /tmp.
    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![vulnerable_root, empty_root]
            .into_iter()
            .map(|p| p.try_into().unwrap())
            .collect(),
        read_only_access: Default::default(),
        network_access: false,
        allow_limited_git_writes: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    // Create the Seatbelt command to wrap a shell command that tries to
    // write to .codex/config.toml in the vulnerable root.
    let shell_command: Vec<String> = [
        "bash",
        "-c",
        "echo 'sandbox_mode = \"danger-full-access\"' > \"$1\"",
        "bash",
        dot_codex_canonical
            .join("config.toml")
            .to_string_lossy()
            .as_ref(),
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();
    let args = create_seatbelt_command_args(
        shell_command.clone(),
        &policy,
        &cwd,
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );

    let policy_text = seatbelt_policy_arg(&args);
    assert!(
        policy_text.contains("(require-all (subpath (param \"WRITABLE_ROOT_0\"))"),
        "expected cwd writable root to carry protected carveouts:\n{policy_text}",
    );
    assert!(
        policy_text.contains("WRITABLE_ROOT_0_EXCLUDED_0"),
        "expected cwd .codex carveout in policy:\n{policy_text}",
    );
    assert!(
        policy_text.contains("WRITABLE_ROOT_1_EXCLUDED_0")
            && policy_text.contains("WRITABLE_ROOT_1_EXCLUDED_1"),
        "expected explicit writable root .git/.codex carveouts in policy:\n{policy_text}",
    );
    assert!(
        policy_text.contains("(subpath (param \"WRITABLE_ROOT_2\"))"),
        "expected second explicit writable root grant in policy:\n{policy_text}",
    );

    let expected_definitions = [
        format!(
            "-DWRITABLE_ROOT_0={}",
            cwd.canonicalize()
                .expect("canonicalize cwd")
                .to_string_lossy()
        ),
        format!(
            "-DWRITABLE_ROOT_0_EXCLUDED_0={}",
            cwd.canonicalize()
                .expect("canonicalize cwd")
                .join(".codex")
                .display()
        ),
        format!(
            "-DWRITABLE_ROOT_1={}",
            vulnerable_root_canonical.to_string_lossy()
        ),
        format!(
            "-DWRITABLE_ROOT_1_EXCLUDED_0={}",
            dot_git_canonical.to_string_lossy()
        ),
        format!(
            "-DWRITABLE_ROOT_1_EXCLUDED_1={}",
            dot_codex_canonical.to_string_lossy()
        ),
        format!(
            "-DWRITABLE_ROOT_2={}",
            empty_root_canonical.to_string_lossy()
        ),
    ];
    let writable_definitions: Vec<String> = args
        .iter()
        .filter(|arg| arg.starts_with("-DWRITABLE_ROOT_"))
        .cloned()
        .collect();
    assert_eq!(
        writable_definitions, expected_definitions,
        "unexpected writable-root parameter definitions in {args:#?}"
    );
    for (key, value) in macos_dir_params() {
        let expected_definition = format!("-D{key}={}", value.to_string_lossy());
        assert!(
            args.contains(&expected_definition),
            "expected definition arg `{expected_definition}` in {args:#?}"
        );
    }

    let command_index = args
        .iter()
        .position(|arg| arg == "--")
        .expect("seatbelt args should include command separator");
    assert_eq!(args[command_index + 1..], shell_command);

    // Verify that .codex/config.toml cannot be modified under the generated
    // Seatbelt policy.
    let config_toml = dot_codex_canonical.join("config.toml");
    let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
        .args(&args)
        .current_dir(&cwd)
        .output()
        .expect("execute seatbelt command");
    assert_eq!(
        "sandbox_mode = \"read-only\"\n",
        String::from_utf8_lossy(&fs::read(&config_toml).expect("read config.toml")),
        "config.toml should contain its original contents because it should not have been modified"
    );
    assert!(
        !output.status.success(),
        "command to write {} should fail under seatbelt",
        &config_toml.display()
    );
    assert_seatbelt_denied(&output.stderr, &config_toml);

    // Create a similar Seatbelt command that tries to write to a file in
    // the .git folder, which should also be blocked.
    let pre_commit_hook = dot_git_canonical.join("hooks").join("pre-commit");
    let shell_command_git: Vec<String> = [
        "bash",
        "-c",
        "echo 'pwned!' > \"$1\"",
        "bash",
        pre_commit_hook.to_string_lossy().as_ref(),
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();
    let write_hooks_file_args = create_seatbelt_command_args(
        shell_command_git,
        &policy,
        &cwd,
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );
    let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
        .args(&write_hooks_file_args)
        .current_dir(&cwd)
        .output()
        .expect("execute seatbelt command");
    assert!(
        !fs::exists(&pre_commit_hook).expect("exists pre-commit hook"),
        "{} should not exist because it should not have been created",
        pre_commit_hook.display()
    );
    assert!(
        !output.status.success(),
        "command to write {} should fail under seatbelt",
        &pre_commit_hook.display()
    );
    assert_seatbelt_denied(&output.stderr, &pre_commit_hook);

    // Verify that writing a file to the folder containing .git and .codex is allowed.
    let allowed_file = vulnerable_root_canonical.join("allowed.txt");
    let shell_command_allowed: Vec<String> = [
        "bash",
        "-c",
        "echo 'this is allowed' > \"$1\"",
        "bash",
        allowed_file.to_string_lossy().as_ref(),
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();
    let write_allowed_file_args = create_seatbelt_command_args(
        shell_command_allowed,
        &policy,
        &cwd,
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );
    let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
        .args(&write_allowed_file_args)
        .current_dir(&cwd)
        .output()
        .expect("execute seatbelt command");
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success()
        && stderr.contains("sandbox-exec: sandbox_apply: Operation not permitted")
    {
        return;
    }
    assert!(
        output.status.success(),
        "command to write {} should succeed under seatbelt",
        &allowed_file.display()
    );
    assert_eq!(
        "this is allowed\n",
        String::from_utf8_lossy(&fs::read(&allowed_file).expect("read allowed.txt")),
        "{} should contain the written text",
        allowed_file.display()
    );
}

#[test]
fn create_seatbelt_args_with_allow_limited_git_writes_carves_out_config_and_hooks() {
    let tmp = TempDir::new().expect("tempdir");
    let PopulatedTmp {
        vulnerable_root,
        dot_git_canonical,
        empty_root,
        ..
    } = populate_tmpdir(tmp.path());
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("create cwd");

    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![vulnerable_root, empty_root]
            .into_iter()
            .map(|p| p.try_into().unwrap())
            .collect(),
        read_only_access: Default::default(),
        network_access: false,
        allow_limited_git_writes: true,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    let args = create_seatbelt_command_args(
        vec!["true".to_string()],
        &policy,
        &cwd,
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );
    let writable_definitions: Vec<String> = args
        .iter()
        .filter(|arg| arg.starts_with("-DWRITABLE_ROOT_"))
        .cloned()
        .collect();

    let dot_git_definition = format!("={}", dot_git_canonical.to_string_lossy());
    assert!(
        !writable_definitions
            .iter()
            .any(|arg| arg.ends_with(&dot_git_definition)),
        "allow_limited_git_writes should not carve out the entire .git directory in {writable_definitions:#?}"
    );

    for protected_path in [
        dot_git_canonical.join("config"),
        dot_git_canonical.join("hooks"),
    ] {
        let protected_definition = format!("={}", protected_path.to_string_lossy());
        assert!(
            writable_definitions
                .iter()
                .any(|arg| arg.ends_with(&protected_definition)),
            "expected protected Git path {protected_path:?} in {writable_definitions:#?}"
        );
    }
}

#[test]
fn create_seatbelt_args_block_first_time_dot_codex_creation_with_exact_and_descendant_carveouts() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).expect("create repo root");

    Command::new("git")
        .arg("init")
        .arg(".")
        .current_dir(&repo_root)
        .output()
        .expect("git init .");

    let dot_codex = repo_root.join(".codex");
    let config_toml = dot_codex.join("config.toml");
    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![repo_root.as_path().try_into().expect("absolute repo root")],
        read_only_access: Default::default(),
        network_access: false,
        allow_limited_git_writes: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    let shell_command: Vec<String> = [
        "bash",
        "-c",
        "mkdir -p \"$1\" && echo 'sandbox_mode = \"danger-full-access\"' > \"$2\"",
        "bash",
        dot_codex.to_string_lossy().as_ref(),
        config_toml.to_string_lossy().as_ref(),
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();
    let args = create_seatbelt_command_args(
        shell_command,
        &policy,
        repo_root.as_path(),
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );

    let policy_text = seatbelt_policy_arg(&args);
    assert!(
        policy_text.contains("(require-not (literal (param \"WRITABLE_ROOT_0_EXCLUDED_1\")))"),
        "expected exact .codex carveout in policy:\n{policy_text}"
    );
    assert!(
        policy_text.contains("(require-not (subpath (param \"WRITABLE_ROOT_0_EXCLUDED_1\")))"),
        "expected descendant .codex carveout in policy:\n{policy_text}"
    );
}

#[test]
fn create_seatbelt_args_with_read_only_git_pointer_file() {
    let tmp = TempDir::new().expect("tempdir");
    let worktree_root = tmp.path().join("worktree_root");
    fs::create_dir_all(&worktree_root).expect("create worktree_root");
    let gitdir = worktree_root.join("actual-gitdir");
    fs::create_dir_all(&gitdir).expect("create gitdir");
    let gitdir_config = gitdir.join("config");
    let gitdir_config_contents = "[core]\n";
    fs::write(&gitdir_config, gitdir_config_contents).expect("write gitdir config");

    let dot_git = worktree_root.join(".git");
    let dot_git_contents = format!("gitdir: {}\n", gitdir.to_string_lossy());
    fs::write(&dot_git, &dot_git_contents).expect("write .git pointer");

    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("create cwd");

    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![worktree_root.try_into().expect("worktree_root is absolute")],
        read_only_access: Default::default(),
        network_access: false,
        allow_limited_git_writes: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    let shell_command: Vec<String> = [
        "bash",
        "-c",
        "echo 'pwned!' > \"$1\"",
        "bash",
        dot_git.to_string_lossy().as_ref(),
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();
    let args = create_seatbelt_command_args(
        shell_command,
        &policy,
        &cwd,
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );

    let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
        .args(&args)
        .current_dir(&cwd)
        .output()
        .expect("execute seatbelt command");

    assert_eq!(
        dot_git_contents,
        String::from_utf8_lossy(&fs::read(&dot_git).expect("read .git pointer")),
        ".git pointer file should not be modified under seatbelt"
    );
    assert!(
        !output.status.success(),
        "command to write {} should fail under seatbelt",
        dot_git.display()
    );
    assert_seatbelt_denied(&output.stderr, &dot_git);

    let shell_command_gitdir: Vec<String> = [
        "bash",
        "-c",
        "echo 'pwned!' > \"$1\"",
        "bash",
        gitdir_config.to_string_lossy().as_ref(),
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();
    let gitdir_args = create_seatbelt_command_args(
        shell_command_gitdir,
        &policy,
        &cwd,
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );
    let output = Command::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
        .args(&gitdir_args)
        .current_dir(&cwd)
        .output()
        .expect("execute seatbelt command");

    assert_eq!(
        gitdir_config_contents,
        String::from_utf8_lossy(&fs::read(&gitdir_config).expect("read gitdir config")),
        "gitdir config should contain its original contents because it should not have been modified"
    );
    assert!(
        !output.status.success(),
        "command to write {} should fail under seatbelt",
        gitdir_config.display()
    );
    assert_seatbelt_denied(&output.stderr, &gitdir_config);
}

#[test]
fn create_seatbelt_args_for_cwd_as_git_repo() {
    // Create a temporary workspace with two writable roots: one containing
    // top-level .git and .codex directories and one without them.
    let tmp = TempDir::new().expect("tempdir");
    let PopulatedTmp {
        vulnerable_root,
        vulnerable_root_canonical,
        dot_git_canonical,
        dot_codex_canonical,
        ..
    } = populate_tmpdir(tmp.path());

    // Build a policy that does not specify any writable_roots, but does
    // use the default ones (cwd and TMPDIR) and verifies the `.git` and
    // `.codex` checks are done properly for cwd.
    let policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        allow_limited_git_writes: false,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    };

    let shell_command: Vec<String> = [
        "bash",
        "-c",
        "echo 'sandbox_mode = \"danger-full-access\"' > \"$1\"",
        "bash",
        dot_codex_canonical
            .join("config.toml")
            .to_string_lossy()
            .as_ref(),
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();
    let args = create_seatbelt_command_args(
        shell_command.clone(),
        &policy,
        vulnerable_root.as_path(),
        /*enforce_managed_network*/ false,
        /*network*/ None,
    );

    let tmpdir_env_var = std::env::var("TMPDIR")
        .ok()
        .map(PathBuf::from)
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().to_string());

    let tempdir_policy_entry = if tmpdir_env_var.is_some() {
        r#" (require-all (subpath (param "WRITABLE_ROOT_2")) (require-not (literal (param "WRITABLE_ROOT_2_EXCLUDED_0"))) (require-not (subpath (param "WRITABLE_ROOT_2_EXCLUDED_0"))) (require-not (literal (param "WRITABLE_ROOT_2_EXCLUDED_1"))) (require-not (subpath (param "WRITABLE_ROOT_2_EXCLUDED_1"))) )"#
    } else {
        ""
    };

    // Build the expected policy text using a raw string for readability.
    // Note that the policy includes:
    // - the base policy,
    // - read-only access to the filesystem,
    // - write access to WRITABLE_ROOT_0 (but not its .git or .codex), WRITABLE_ROOT_1, and cwd as WRITABLE_ROOT_2.
    let expected_policy = format!(
        r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (literal (param "WRITABLE_ROOT_0_EXCLUDED_0"))) (require-not (subpath (param "WRITABLE_ROOT_0_EXCLUDED_0"))) (require-not (literal (param "WRITABLE_ROOT_0_EXCLUDED_1"))) (require-not (subpath (param "WRITABLE_ROOT_0_EXCLUDED_1"))) ) (subpath (param "WRITABLE_ROOT_1")){tempdir_policy_entry}
)
"#,
    );

    let mut expected_args = vec![
        "-p".to_string(),
        expected_policy,
        format!(
            "-DWRITABLE_ROOT_0={}",
            vulnerable_root_canonical.to_string_lossy()
        ),
        format!(
            "-DWRITABLE_ROOT_0_EXCLUDED_0={}",
            dot_git_canonical.to_string_lossy()
        ),
        format!(
            "-DWRITABLE_ROOT_0_EXCLUDED_1={}",
            dot_codex_canonical.to_string_lossy()
        ),
        format!(
            "-DWRITABLE_ROOT_1={}",
            PathBuf::from("/tmp")
                .canonicalize()
                .expect("canonicalize /tmp")
                .to_string_lossy()
        ),
    ];

    if let Some(p) = tmpdir_env_var {
        expected_args.push(format!("-DWRITABLE_ROOT_2={p}"));
        expected_args.push(format!(
            "-DWRITABLE_ROOT_2_EXCLUDED_0={}",
            dot_git_canonical.to_string_lossy()
        ));
        expected_args.push(format!(
            "-DWRITABLE_ROOT_2_EXCLUDED_1={}",
            dot_codex_canonical.to_string_lossy()
        ));
    }

    expected_args.extend(
        macos_dir_params()
            .into_iter()
            .map(|(key, value)| format!("-D{key}={value}", value = value.to_string_lossy())),
    );

    expected_args.push("--".to_string());
    expected_args.extend(shell_command);

    assert_eq!(expected_args, args);
}

struct PopulatedTmp {
    /// Path containing a .git and .codex subfolder.
    /// For the purposes of this test, we consider this a "vulnerable" root
    /// because a bad actor could write to .git/hooks/pre-commit so an
    /// unsuspecting user would run code as privileged the next time they
    /// ran `git commit` themselves, or modified .codex/config.toml to
    /// contain `sandbox_mode = "danger-full-access"` so the agent would
    /// have full privileges the next time it ran in that repo.
    vulnerable_root: PathBuf,
    vulnerable_root_canonical: PathBuf,
    dot_git_canonical: PathBuf,
    dot_codex_canonical: PathBuf,

    /// Path without .git or .codex subfolders.
    empty_root: PathBuf,
    /// Canonicalized version of `empty_root`.
    empty_root_canonical: PathBuf,
}

fn populate_tmpdir(tmp: &Path) -> PopulatedTmp {
    let vulnerable_root = tmp.join("vulnerable_root");
    fs::create_dir_all(&vulnerable_root).expect("create vulnerable_root");

    // TODO(mbolin): Should also support the case where `.git` is a file
    // with a gitdir: ... line.
    Command::new("git")
        .arg("init")
        .arg(".")
        .current_dir(&vulnerable_root)
        .output()
        .expect("git init .");

    fs::create_dir_all(vulnerable_root.join(".codex")).expect("create .codex");
    fs::write(
        vulnerable_root.join(".codex").join("config.toml"),
        "sandbox_mode = \"read-only\"\n",
    )
    .expect("write .codex/config.toml");

    let empty_root = tmp.join("empty_root");
    fs::create_dir_all(&empty_root).expect("create empty_root");

    // Ensure we have canonical paths for -D parameter matching.
    let vulnerable_root_canonical = vulnerable_root
        .canonicalize()
        .expect("canonicalize vulnerable_root");
    let dot_git_canonical = vulnerable_root_canonical.join(".git");
    let dot_codex_canonical = vulnerable_root_canonical.join(".codex");
    let empty_root_canonical = empty_root.canonicalize().expect("canonicalize empty_root");
    PopulatedTmp {
        vulnerable_root,
        vulnerable_root_canonical,
        dot_git_canonical,
        dot_codex_canonical,
        empty_root,
        empty_root_canonical,
    }
}
