use super::*;
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::sandboxing::ExecOptions;
use crate::shell::ShellType;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::managed_network_for_sandbox_permissions;
#[cfg(target_os = "macos")]
use codex_network_proxy::CODEX_PROXY_GIT_SSH_COMMAND_MARKER;
use codex_network_proxy::CUSTOM_CA_ENV_KEYS;
use codex_network_proxy::ConfigReloader;
use codex_network_proxy::ConfigReloaderFuture;
use codex_network_proxy::ConfigState;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::NetworkProxyConstraints;
use codex_network_proxy::NetworkProxyState;
use codex_network_proxy::PROXY_ACTIVE_ENV_KEY;
use codex_network_proxy::PROXY_ENV_KEYS;
#[cfg(target_os = "macos")]
use codex_network_proxy::PROXY_GIT_SSH_COMMAND_ENV_KEY;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxType;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use pretty_assertions::assert_eq;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;

struct StaticReloader;

impl ConfigReloader for StaticReloader {
    fn source_label(&self) -> String {
        "test config state".to_string()
    }

    fn maybe_reload(&self) -> ConfigReloaderFuture<'_, Option<ConfigState>> {
        Box::pin(async { Ok(None) })
    }

    fn reload_now(&self) -> ConfigReloaderFuture<'_, ConfigState> {
        Box::pin(async { Err(anyhow::anyhow!("force reload is not supported in tests")) })
    }
}

fn shell_with_snapshot(
    shell_type: ShellType,
    shell_path: &str,
    snapshot_path: AbsolutePathBuf,
) -> (Shell, AbsolutePathBuf) {
    (
        Shell {
            shell_type,
            shell_path: PathBuf::from(shell_path),
        },
        snapshot_path,
    )
}

async fn test_network_proxy() -> anyhow::Result<NetworkProxy> {
    test_network_proxy_with_config(NetworkProxyConfig::default()).await
}

pub(super) async fn test_credential_broker_network_proxy() -> anyhow::Result<NetworkProxy> {
    let mut config = NetworkProxyConfig::default();
    config.set_credential_broker_enabled(/*enabled*/ true);
    test_network_proxy_with_config(config).await
}

async fn test_network_proxy_with_config(
    config: NetworkProxyConfig,
) -> anyhow::Result<NetworkProxy> {
    let state =
        codex_network_proxy::build_config_state(config, NetworkProxyConstraints::default())?;
    NetworkProxy::builder()
        .state(Arc::new(NetworkProxyState::with_reloader(
            state,
            Arc::new(StaticReloader),
        )))
        .managed_by_codex(/*managed_by_codex*/ false)
        .http_addr("127.0.0.1:43128".parse()?)
        .socks_addr("127.0.0.1:48081".parse()?)
        .build()
        .await
}

#[tokio::test]
async fn explicit_escalation_prepares_exec_without_managed_network() -> anyhow::Result<()> {
    let proxy = test_network_proxy().await?;
    let dir = tempdir().expect("create temp dir");
    let command_cwd = dir.path().join("command").abs();
    let native_sandbox_policy_cwd = dir.path().join("sandbox-policy").abs();
    let mut env = HashMap::from([("CUSTOM_ENV".to_string(), "kept".to_string())]);
    proxy.apply_to_env(&mut env);

    let command = codex_sandboxing::SandboxCommand {
        program: "/bin/echo".into(),
        args: vec!["ok".to_string()],
        cwd: PathUri::from_abs_path(&command_cwd),
        env: exec_env_for_sandbox_permissions(&env, SandboxPermissions::RequireEscalated),
        managed_network: None,
        additional_permissions: None,
    };
    assert_eq!(command.cwd, PathUri::from_abs_path(&command_cwd));
    let sandbox_policy_cwd = PathUri::from_abs_path(&native_sandbox_policy_cwd);
    let options = ExecOptions {
        expiration: ExecExpiration::DefaultTimeout,
        capture_policy: ExecCapturePolicy::ShellTool,
    };
    let permissions = PermissionProfile::Disabled;
    let manager = SandboxManager::new();
    let attempt = SandboxAttempt {
        sandbox: SandboxType::None,
        sandbox_requested: false,
        permissions: &permissions,
        exec_server_permissions: &permissions,
        enforce_managed_network: false,
        manager: &manager,
        sandbox_cwd: &sandbox_policy_cwd,
        workspace_roots: std::slice::from_ref(&sandbox_policy_cwd),
        sandbox_exe: None,
        use_legacy_landlock: false,
        windows_sandbox_type: SandboxType::None,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        network_denial_cancellation_token: None,
        network_proxy: None,
    };

    let exec_request = attempt
        .env_for(
            command,
            options,
            managed_network_for_sandbox_permissions(
                Some(&proxy),
                SandboxPermissions::RequireEscalated,
            ),
            /*environment_id*/ None,
        )
        .expect("prepare exec request");

    assert_eq!(exec_request.cwd, PathUri::from_abs_path(&command_cwd));
    assert_eq!(
        exec_request.windows_sandbox_policy_cwd,
        PathUri::from_abs_path(&native_sandbox_policy_cwd)
    );
    assert_eq!(exec_request.network, None);
    for key in PROXY_ENV_KEYS {
        assert_eq!(exec_request.env.get(*key), None, "{key} should be unset");
    }
    for key in CUSTOM_CA_ENV_KEYS {
        assert_eq!(exec_request.env.get(key), None, "{key} should be unset");
    }
    #[cfg(target_os = "macos")]
    assert_eq!(exec_request.env.get(PROXY_GIT_SSH_COMMAND_ENV_KEY), None);
    assert_eq!(
        exec_request.env.get("CUSTOM_ENV"),
        Some(&"kept".to_string())
    );

    Ok(())
}

#[test]
fn explicit_escalation_preserves_user_ca_env() {
    let env = HashMap::from([
        (PROXY_ACTIVE_ENV_KEY.to_string(), "1".to_string()),
        (
            "SSL_CERT_FILE".to_string(),
            "/tmp/custom-ca.pem".to_string(),
        ),
    ]);

    let env = exec_env_for_sandbox_permissions(&env, SandboxPermissions::RequireEscalated);

    assert_eq!(
        env.get("SSL_CERT_FILE"),
        Some(&"/tmp/custom-ca.pem".to_string())
    );
}

#[cfg(unix)]
#[test]
fn runtime_path_prepends_records_runtime_path_prepend() {
    let mut env = HashMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]);
    let mut runtime_path_prepends = RuntimePathPrepends::default();

    runtime_path_prepends.prepend(&mut env, PathBuf::from("/package/codex-path").as_path());

    assert_eq!(
        env.get("PATH").map(String::as_str),
        Some("/package/codex-path:/usr/bin:/bin"),
        "runtime PATH prepend should update the live exec environment"
    );
    assert_eq!(
        runtime_path_prepends.entries,
        vec!["/package/codex-path"],
        "runtime PATH prepend should be recorded for snapshot replay"
    );
}

#[cfg(unix)]
#[test]
fn runtime_path_prepends_drops_empty_path_entries() {
    let mut env = HashMap::from([(
        "PATH".to_string(),
        ":/usr/bin:/package/codex-path::/bin:".to_string(),
    )]);
    let mut runtime_path_prepends = RuntimePathPrepends::default();

    runtime_path_prepends.prepend(&mut env, PathBuf::from("/package/codex-path").as_path());

    assert_eq!(
        env.get("PATH").map(String::as_str),
        Some("/package/codex-path:/usr/bin:/bin"),
        "empty PATH entries should be dropped instead of preserving current-directory lookup"
    );
    assert_eq!(
        runtime_path_prepends.entries,
        vec!["/package/codex-path"],
        "deduped runtime PATH prepend should still be recorded once"
    );
}

#[cfg(unix)]
#[test]
fn runtime_path_prepends_ignores_empty_path_entry() {
    let mut env = HashMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]);
    let mut runtime_path_prepends = RuntimePathPrepends::default();

    runtime_path_prepends.prepend(&mut env, PathBuf::new().as_path());

    assert_eq!(
        env.get("PATH").map(String::as_str),
        Some("/usr/bin:/bin"),
        "empty runtime PATH prepend should leave PATH unchanged"
    );
    assert_eq!(
        runtime_path_prepends,
        RuntimePathPrepends::default(),
        "empty runtime PATH prepend should not be recorded for snapshot replay"
    );
}

#[cfg(unix)]
#[test]
fn apply_zsh_fork_path_prepend_uses_shell_parent() {
    let mut env = HashMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]);
    let mut runtime_path_prepends = RuntimePathPrepends::default();

    apply_zsh_fork_path_prepend(
        &mut env,
        &mut runtime_path_prepends,
        PathBuf::from("/package/codex-resources/zsh/bin/zsh").as_path(),
    );

    let expected = "/package/codex-resources/zsh/bin:/usr/bin:/bin";
    assert_eq!(env.get("PATH").map(String::as_str), Some(expected));
    assert_eq!(
        runtime_path_prepends,
        RuntimePathPrepends {
            entries: vec!["/package/codex-resources/zsh/bin".to_string()]
        }
    );
}

#[cfg(unix)]
#[test]
fn apply_zsh_fork_path_prepend_moves_existing_shell_parent_to_front() {
    let mut env = HashMap::from([(
        "PATH".to_string(),
        "/usr/bin:/package/codex-resources/zsh/bin:/bin:/package/codex-resources/zsh/bin"
            .to_string(),
    )]);
    let mut runtime_path_prepends = RuntimePathPrepends::default();

    apply_zsh_fork_path_prepend(
        &mut env,
        &mut runtime_path_prepends,
        PathBuf::from("/package/codex-resources/zsh/bin/zsh").as_path(),
    );

    assert_eq!(
        env.get("PATH").map(String::as_str),
        Some("/package/codex-resources/zsh/bin:/usr/bin:/bin")
    );
    assert_eq!(
        runtime_path_prepends,
        RuntimePathPrepends {
            entries: vec!["/package/codex-resources/zsh/bin".to_string()]
        }
    );
}

#[test]
fn explicit_escalation_keeps_user_proxy_env_without_codex_marker() {
    let env = HashMap::from([
        (
            "HTTP_PROXY".to_string(),
            "http://user.proxy:8080".to_string(),
        ),
        ("CUSTOM_ENV".to_string(), "kept".to_string()),
    ]);

    let env = exec_env_for_sandbox_permissions(&env, SandboxPermissions::RequireEscalated);

    assert_eq!(
        env.get("HTTP_PROXY"),
        Some(&"http://user.proxy:8080".to_string())
    );
    assert_eq!(env.get("CUSTOM_ENV"), Some(&"kept".to_string()));
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_bootstraps_in_user_shell() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, "# Snapshot file\n").expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Zsh, "/bin/zsh", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "echo hello".to_string(),
    ];

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );

    assert_eq!(rewritten[0], "/bin/zsh");
    assert_eq!(rewritten[1], "-c");
    assert!(rewritten[2].contains("if . '"));
    assert!(rewritten[2].contains("exec '/bin/bash' -c 'echo hello'"));
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_escapes_single_quotes() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, "# Snapshot file\n").expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Zsh, "/bin/zsh", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "echo 'hello'".to_string(),
    ];

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );

    assert!(rewritten[2].contains(r#"exec '/bin/bash' -c 'echo '"'"'hello'"'"''"#));
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_uses_bash_bootstrap_shell() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, "# Snapshot file\n").expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/zsh".to_string(),
        "-lc".to_string(),
        "echo hello".to_string(),
    ];

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );

    assert_eq!(rewritten[0], "/bin/bash");
    assert_eq!(rewritten[1], "-c");
    assert!(rewritten[2].contains("if . '"));
    assert!(rewritten[2].contains("exec '/bin/zsh' -c 'echo hello'"));
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_uses_sh_bootstrap_shell() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, "# Snapshot file\n").expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Sh, "/bin/sh", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "echo hello".to_string(),
    ];

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );

    assert_eq!(rewritten[0], "/bin/sh");
    assert_eq!(rewritten[1], "-c");
    assert!(rewritten[2].contains("if . '"));
    assert!(rewritten[2].contains("exec '/bin/bash' -c 'echo hello'"));
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_preserves_trailing_args() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, "# Snapshot file\n").expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Zsh, "/bin/zsh", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s %s' \"$0\" \"$1\"".to_string(),
        "arg0".to_string(),
        "arg1".to_string(),
    ];

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );

    assert!(
        rewritten[2]
            .contains(r#"exec '/bin/bash' -c 'printf '"'"'%s %s'"'"' "$0" "$1"' 'arg0' 'arg1'"#)
    );
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_reuses_brokered_session_zsh() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, "# Snapshot file\n").expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Zsh, "/bin/zsh", snapshot_path.abs());
    let copy_key = format!("{SNAPSHOT_BROKERED_VALUE_ENV_PREFIX}OPENAI_API_KEY");
    let env = HashMap::from([
        (
            CREDENTIAL_BROKER_ACTIVE_ENV_KEY.to_string(),
            "1".to_string(),
        ),
        (copy_key.clone(), "dummy-openai-key".to_string()),
    ]);
    for (flag, expected_flag) in [("-c", "-fc"), ("-lc", "-lfc")] {
        let command = vec![
            "/bin/zsh".to_string(),
            flag.to_string(),
            "printf '%s %s' \"$0\" \"$1\"".to_string(),
            "arg0".to_string(),
            "arg1".to_string(),
        ];

        let rewritten = maybe_wrap_shell_lc_with_snapshot(
            &command,
            &session_shell,
            Some(&shell_snapshot),
            &HashMap::new(),
            &env,
            &RuntimePathPrepends::default(),
        );

        assert_eq!(rewritten[0], "/bin/zsh");
        assert_eq!(rewritten[1], expected_flag);
        assert!(rewritten[2].contains(&format!(
            "if . '{}' >/dev/null 2>&1; then :; fi",
            shell_single_quote(&shell_snapshot.to_string_lossy())
        )));
        assert!(rewritten[2].ends_with(&command[2]));
        assert!(rewritten[2].contains(&format!("unset {copy_key} || exit 1")));
        assert!(!rewritten[2].contains("exec '/bin/zsh'"));
        assert_eq!(rewritten[3..], command[3..]);
    }
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_restores_explicit_override_precedence() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport TEST_ENV_SNAPSHOT=global\nexport SNAPSHOT_ONLY=from_snapshot\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s|%s' \"$TEST_ENV_SNAPSHOT\" \"${SNAPSHOT_ONLY-unset}\"".to_string(),
    ];
    let explicit_env_overrides =
        HashMap::from([("TEST_ENV_SNAPSHOT".to_string(), "worktree".to_string())]);
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &explicit_env_overrides,
        &HashMap::from([("TEST_ENV_SNAPSHOT".to_string(), "worktree".to_string())]),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env("TEST_ENV_SNAPSHOT", "worktree")
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "worktree|from_snapshot"
    );
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_restores_codex_metadata_from_env() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport CODEX_THREAD_ID='parent-thread'\nexport CODEX_VERSION='old-version'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s|%s' \"$CODEX_THREAD_ID\" \"$CODEX_VERSION\"".to_string(),
    ];
    let env = HashMap::from([
        ("CODEX_THREAD_ID".to_string(), "nested-thread".to_string()),
        (
            "CODEX_VERSION".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ),
    ]);
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &env,
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .envs(env)
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        concat!("nested-thread|", env!("CARGO_PKG_VERSION")),
    );
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_restores_permission_profile_from_env() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport CODEX_PERMISSION_PROFILE='parent-profile'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printenv CODEX_PERMISSION_PROFILE".to_string(),
    ];
    let env = HashMap::from([(
        CODEX_PERMISSION_PROFILE_ENV_VAR.to_string(),
        "current-profile".to_string(),
    )]);
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &env,
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env(CODEX_PERMISSION_PROFILE_ENV_VAR, "current-profile")
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "current-profile\n");
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_unsets_absent_permission_profile() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport CODEX_PERMISSION_PROFILE='stale-profile'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printenv CODEX_PERMISSION_PROFILE".to_string(),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env_remove(CODEX_PERMISSION_PROFILE_ENV_VAR)
        .output()
        .expect("run rewritten command");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"");
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_restores_apply_patch_rollout_state() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS='stale'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printenv CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS".to_string(),
    ];
    let env = HashMap::from([(
        CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR.to_string(),
        "1".to_string(),
    )]);
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &env,
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env(CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR, "1")
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env_remove(CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR)
        .output()
        .expect("run rewritten command");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"");
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_restores_reserved_metrics_output_env() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport CODEX_PLUGIN_METRICS_OUTPUT='/stale/path'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"${CODEX_PLUGIN_METRICS_OUTPUT-unset}\"".to_string(),
    ];

    for (live_value, expected) in [(None, "unset"), (Some("/private/path"), "/private/path")] {
        let env = live_value
            .map(|value| {
                HashMap::from([(PLUGIN_METRICS_OUTPUT_ENV_VAR.to_string(), value.to_string())])
            })
            .unwrap_or_default();
        let rewritten = maybe_wrap_shell_lc_with_snapshot(
            &command,
            &session_shell,
            Some(&shell_snapshot),
            &HashMap::new(),
            &env,
            &RuntimePathPrepends::default(),
        );
        let mut process = Command::new(&rewritten[0]);
        process.args(&rewritten[1..]);
        match live_value {
            Some(value) => process.env(PLUGIN_METRICS_OUTPUT_ENV_VAR, value),
            None => process.env_remove(PLUGIN_METRICS_OUTPUT_ENV_VAR),
        };
        let output = process.output().expect("run rewritten command");

        assert!(output.status.success(), "command failed: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    }
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_restores_proxy_env_from_process_env() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\n\
         export PIP_PROXY='http://127.0.0.1:8080'\n\
         export HTTP_PROXY='http://127.0.0.1:8080'\n\
         export http_proxy='http://127.0.0.1:8080'\n\
         export GIT_SSH_COMMAND='ssh -o ProxyCommand=stale'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s\\n%s\\n%s\\n%s' \"$PIP_PROXY\" \"$HTTP_PROXY\" \"$http_proxy\" \"$GIT_SSH_COMMAND\""
            .to_string(),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env(PROXY_ACTIVE_ENV_KEY, "1")
        .env("PIP_PROXY", "http://127.0.0.1:4321")
        .env("HTTP_PROXY", "http://127.0.0.1:4321")
        .env("http_proxy", "http://127.0.0.1:4321")
        .env("GIT_SSH_COMMAND", "ssh -o ProxyCommand=fresh")
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "http://127.0.0.1:4321\n\
         http://127.0.0.1:4321\n\
         http://127.0.0.1:4321\n\
         ssh -o ProxyCommand=stale"
    );
}

#[tokio::test]
async fn snapshot_wrapper_preserves_readonly_dummy_credentials() -> anyhow::Result<()> {
    let proxy = test_credential_broker_network_proxy().await?;
    let dir = tempdir()?;
    let snapshot = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot, "# Snapshot file\ntypeset +x GH_TOKEN\n")?;
    let mut env = HashMap::from([(
        "GH_TOKEN".to_string(),
        "ghp_abcdefghijklmnopqrstuvwxyz0123456789".to_string(),
    )]);
    proxy.apply_to_env(&mut env);
    let dummy = env["GH_TOKEN"].clone();
    assert_ne!(dummy, "ghp_abcdefghijklmnopqrstuvwxyz0123456789");
    for (shell_type, path) in [(ShellType::Bash, "/bin/bash"), (ShellType::Zsh, "/bin/zsh")] {
        if !std::path::Path::new(path).exists() {
            continue;
        }
        let (shell, snapshot) = shell_with_snapshot(shell_type, path, snapshot.abs());
        let command = vec![
            path.to_string(),
            "-c".to_string(),
            "exec /bin/sh -c 'printf %s \"$GH_TOKEN\"'".to_string(),
        ];
        let mut rewritten = maybe_wrap_shell_lc_with_snapshot(
            &command,
            &shell,
            Some(&snapshot),
            &HashMap::new(),
            &env,
            &RuntimePathPrepends::default(),
        );
        // Global startup can mark an inherited dummy readonly before the wrapper runs.
        rewritten[2].insert_str(0, "readonly GH_TOKEN\n");
        let output = Command::new(&rewritten[0])
            .args(&rewritten[1..])
            .env_clear()
            .envs(&env)
            .output()?;
        assert!(output.status.success(), "{path}: {output:?}");
        assert_eq!(String::from_utf8(output.stdout)?, dummy);
    }
    Ok(())
}

#[tokio::test]
async fn snapshot_wrapper_replays_dummy_and_preserves_unbrokered_credentials() -> anyhow::Result<()>
{
    let proxy = test_credential_broker_network_proxy().await?;
    let dir = tempdir()?;
    let startup = dir.path().join("startup.sh");
    std::fs::write(&startup, "export OPENAI_API_KEY='sk-startup-secret'\n")?;
    let snapshot_dir = dir.path().join("snapshots");
    std::fs::create_dir(&snapshot_dir)?;
    let snapshot = snapshot_dir.join("snapshot.sh");
    std::fs::write(
        &snapshot,
        format!(
            "# Snapshot file\ncorp_auth() {{ printf '%s\\n%s\\n%s' \"$OPENAI_API_KEY\" \"${{GITHUB_TOKEN-unset}}\" \"$1\"; }}\nexport OPENAI_API_KEY='stale'\nexport GITHUB_TOKEN='ghp_snapshot_only'\nexport GH_HOST='github.example.com'\nexport BASH_ENV='{}'\nexport ENV='{}'\nexport ZDOTDIR='{}'\n",
            startup.display(),
            startup.display(),
            dir.path().display()
        ),
    )?;
    let (shell, snapshot) = shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot.abs());
    let mut env = HashMap::from([
        (
            "OPENAI_API_KEY".to_string(),
            "sk-proj-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_".to_string(),
        ),
        (
            "AUTH_HEADER".to_string(),
            "Bearer sk-proj-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_"
                .to_string(),
        ),
        ("GITHUB_TOKEN".to_string(), String::new()),
        ("BASH_ENV".to_string(), startup.display().to_string()),
        ("ENV".to_string(), startup.display().to_string()),
    ]);
    proxy.apply_to_env(&mut env);
    let dummy = env["OPENAI_API_KEY"].clone();
    let dummy_alias = env["AUTH_HEADER"].clone();
    let mut alias_only_env = env.clone();
    alias_only_env.remove("OPENAI_API_KEY");
    let mut empty_canonical_env = env.clone();
    empty_canonical_env.insert("OPENAI_API_KEY".to_string(), String::new());
    let mut absent_alias_env = env.clone();
    absent_alias_env.remove("AUTH_HEADER");
    prepare_brokered_shell_snapshot_env(&mut env, Some(&snapshot), &shell);
    assert!(!env.contains_key("BASH_ENV"));
    assert_eq!(env.get("ENV"), Some(&startup.display().to_string()));
    let command = vec![
        "/bin/bash".to_string(),
        "-c".to_string(),
        "corp_auth \"$1\"".to_string(),
        "brokered-shell".to_string(),
        "github.example.com".to_string(),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &shell,
        Some(&snapshot),
        &HashMap::new(),
        &env,
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env_clear()
        .envs(&env)
        .output()?;

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{dummy}\nghp_snapshot_only\ngithub.example.com")
    );

    let (posix_shell, _) = shell_with_snapshot(ShellType::Sh, "/bin/sh", snapshot.clone());
    let no_prepends = RuntimePathPrepends::default();
    let wrap_brokered_snapshot = |shell: &Shell,
                                  snapshot: &AbsolutePathBuf,
                                  env: &HashMap<String, String>,
                                  prepends: &RuntimePathPrepends| {
        maybe_wrap_shell_lc_with_snapshot(
            &[
                shell.shell_path.to_string_lossy().into_owned(),
                if shell.shell_type == ShellType::Sh {
                    "-lc"
                } else {
                    "-c"
                }
                .to_string(),
                "/bin/sh -ic 'printf \"%s|%s\" \"$OPENAI_API_KEY\" \"${ENV-unset}\"'".to_string(),
            ],
            shell,
            Some(snapshot),
            &HashMap::new(),
            env,
            prepends,
        )
    };
    let wrap_snapshot = |snapshot: &AbsolutePathBuf, env: &HashMap<String, String>| {
        wrap_brokered_snapshot(&posix_shell, snapshot, env, &no_prepends)
    };
    let nested = wrap_snapshot(&snapshot, &env);
    let run_nested =
        |command: &[String], env: &HashMap<String, String>| -> anyhow::Result<String> {
            let output = Command::new(&command[0])
                .args(&command[1..])
                .current_dir(dir.path())
                .env_clear()
                .envs(env)
                .output()?;
            anyhow::ensure!(output.status.success(), "nested shell failed: {output:?}");
            Ok(String::from_utf8(output.stdout)?)
        };
    let posix_startup = dir.path().join("posix-startup.sh");
    std::fs::write(&posix_startup, "export OPENAI_API_KEY='sk-posix-secret'\n")?;
    let posix_snapshot = dir.path().join("posix-startup-snapshot.sh").abs();
    std::fs::write(
        &posix_snapshot,
        format!("export ENV='{}'\n", posix_startup.display()),
    )?;
    let mut posix_startup_env = env.clone();
    posix_startup_env.insert(
        SNAPSHOT_ORIGINAL_POSIX_ENV_ENV_KEY.to_string(),
        posix_startup.display().to_string(),
    );
    let distinct = wrap_snapshot(&posix_snapshot, &posix_startup_env);
    assert_eq!(
        run_nested(&distinct, &posix_startup_env)?,
        format!("{dummy}|unset")
    );
    posix_startup_env.remove(SNAPSHOT_ORIGINAL_BASH_ENV_ENV_KEY);
    posix_startup_env.insert("ENV".to_string(), posix_startup.display().to_string());
    let inherited = wrap_snapshot(&posix_snapshot, &posix_startup_env);
    assert_eq!(
        run_nested(&inherited, &posix_startup_env)?,
        format!("{dummy}|unset")
    );

    let wrap_bash_snapshot = |snapshot: &AbsolutePathBuf, env: &HashMap<String, String>| {
        wrap_brokered_snapshot(&shell, snapshot, env, &no_prepends)
    };
    let readonly_snapshot = dir.path().join("readonly-env-snapshot.sh").abs();
    for initial_env in [None, Some("production"), Some(startup.to_str().unwrap())] {
        let mut case_env = env.clone();
        case_env.remove("ENV");
        if let Some(value) = initial_env {
            case_env.insert("ENV".to_string(), value.to_string());
        }
        for (value, succeeds) in [(startup.to_str().unwrap(), false), ("production", true)] {
            std::fs::write(
                &readonly_snapshot,
                format!("export ENV='{value}'\nreadonly ENV\n"),
            )?;
            for command_shell in [&shell, &posix_shell] {
                let command = wrap_brokered_snapshot(
                    command_shell,
                    &readonly_snapshot,
                    &case_env,
                    &no_prepends,
                );
                let output = Command::new(&command[0])
                    .args(&command[1..])
                    .current_dir(dir.path())
                    .env_clear()
                    .envs(&case_env)
                    .output()?;
                assert_eq!(output.status.success(), succeeds, "{output:?}");
                assert_eq!(
                    String::from_utf8(output.stdout)?,
                    if succeeds {
                        format!("{dummy}|production")
                    } else {
                        String::new()
                    }
                );
            }
        }
    }
    let prepended_path_snapshot = dir.path().join("prepended-path-snapshot.sh").abs();
    std::fs::write(
        &prepended_path_snapshot,
        "export ENV='${PATH%%:*}/startup.sh'\nexport PATH='/safe'\n",
    )?;
    let mut prepended_env = env.clone();
    prepended_env.insert("PATH".to_string(), "/safe".to_string());
    prepended_env.insert("ENV".to_string(), "${PATH%%:*}/startup.sh".to_string());
    let mut prepends = RuntimePathPrepends::default();
    prepends.prepend(&mut prepended_env, dir.path());
    let command =
        wrap_brokered_snapshot(&shell, &prepended_path_snapshot, &prepended_env, &prepends);
    assert_eq!(
        run_nested(&command, &prepended_env)?,
        format!("{dummy}|unset")
    );
    let relative_startup_snapshot = dir.path().join("relative-startup-snapshot.sh").abs();
    let startup_path = startup.display().to_string();
    let mut expanded_startup_env = env.clone();
    expanded_startup_env.extend([
        ("SNAPSHOT_STARTUP_FILE".to_string(), startup_path.clone()),
        ("ORIGINAL_STARTUP_FILE".to_string(), startup_path.clone()),
        (
            "SNAPSHOT_APPLICATION_FILE".to_string(),
            "/safe-application.env".to_string(),
        ),
        (
            SNAPSHOT_ORIGINAL_BASH_ENV_ENV_KEY.to_string(),
            "$SNAPSHOT_STARTUP_FILE".to_string(),
        ),
    ]);
    let application_script =
        format!("export ENV='{startup_path}'\nexport SNAPSHOT_APPLICATION_FILE='{startup_path}'\n");
    let changed_startup_script = "export ENV='$ORIGINAL_STARTUP_FILE'\nexport SNAPSHOT_STARTUP_FILE='/different-startup.sh'\n";
    let strict_script = format!("set -eu\nexport ENV='{startup_path}'\n");
    for (original_env, script, expected_env) in [
        (
            startup_path.as_str(),
            "export ENV='./startup.sh'\n",
            "unset",
        ),
        (
            startup_path.as_str(),
            "export ENV='$SNAPSHOT_STARTUP_FILE'\n",
            "unset",
        ),
        (
            "$SNAPSHOT_APPLICATION_FILE",
            application_script.as_str(),
            "unset",
        ),
        (
            "${SNAPSHOT_APPLICATION_FILE}",
            application_script.as_str(),
            "unset",
        ),
        (startup_path.as_str(), changed_startup_script, "unset"),
        (
            "$OPTIONAL_APPLICATION_FILE",
            strict_script.as_str(),
            "$OPTIONAL_APPLICATION_FILE",
        ),
    ] {
        let mut case_env = expanded_startup_env.clone();
        case_env.insert("ENV".to_string(), original_env.to_string());
        std::fs::write(&relative_startup_snapshot, script)?;
        let command = wrap_bash_snapshot(&relative_startup_snapshot, &case_env);
        assert_eq!(
            run_nested(&command, &case_env)?,
            format!("{dummy}|{expected_env}")
        );
    }

    for bash_env in ["", "$OPTIONAL_STARTUP"] {
        let mut case_env = env.clone();
        case_env.insert("BASH_ENV".to_string(), bash_env.to_string());
        case_env.insert("ENV".to_string(), String::new());
        prepare_brokered_shell_snapshot_env(&mut case_env, Some(&snapshot), &shell);
        let command = wrap_bash_snapshot(&snapshot, &case_env);
        assert_eq!(run_nested(&command, &case_env)?, format!("{dummy}|"));
    }

    let mut expanded_env = env.clone();
    expanded_env.insert("ENV".to_string(), "$SNAPSHOT_STARTUP_FILE".to_string());
    expanded_env.insert(
        "SNAPSHOT_STARTUP_FILE".to_string(),
        startup.display().to_string(),
    );
    assert_eq!(
        run_nested(&nested, &expanded_env)?,
        format!("{dummy}|unset")
    );
    expanded_env.insert("ENV".to_string(), "${SNAPSHOT_STARTUP_FILE}".to_string());
    assert_eq!(
        run_nested(&nested, &expanded_env)?,
        format!("{dummy}|unset")
    );

    let injection = "\"; printf CODEX_ENV_INJECTED; __codex_env_file=\"";
    expanded_env.insert("ENV".to_string(), injection.to_string());
    let startup_value_snapshot = dir.path().join("startup-value-snapshot.sh").abs();
    std::fs::write(
        &startup_value_snapshot,
        format!("export ENV='{injection}'\n"),
    )?;
    let expanded_command = wrap_snapshot(&startup_value_snapshot, &expanded_env);
    assert_eq!(
        run_nested(&expanded_command, &expanded_env)?,
        format!("{dummy}|{injection}")
    );

    std::fs::write(&startup_value_snapshot, "export ENV='~/startup.sh'\n")?;
    let bash_posix_path = dir.path().join("sh");
    std::os::unix::fs::symlink("/bin/bash", &bash_posix_path)?;
    let bash_posix_path = bash_posix_path.display().to_string();
    let (bash_posix_shell, _) =
        shell_with_snapshot(ShellType::Sh, &bash_posix_path, snapshot.clone());
    let bash_posix_command = vec![
        bash_posix_path.clone(),
        "-c".to_string(),
        format!(
            "'{bash_posix_path}' -ic 'printf \"%s|%s\" \"$OPENAI_API_KEY\" \"${{ENV-unset}}\"'"
        ),
    ];
    let mut tilde_env = env.clone();
    tilde_env.insert("ENV".to_string(), "~/startup.sh".to_string());
    tilde_env.insert("HOME".to_string(), dir.path().display().to_string());
    let bash_posix_command = maybe_wrap_shell_lc_with_snapshot(
        &bash_posix_command,
        &bash_posix_shell,
        Some(&startup_value_snapshot),
        &HashMap::new(),
        &tilde_env,
        &RuntimePathPrepends::default(),
    );
    assert_eq!(
        run_nested(&bash_posix_command, &tilde_env)?,
        format!("{dummy}|unset")
    );

    #[cfg(target_os = "macos")]
    {
        expanded_env.insert("ENV".to_string(), "~/startup.sh".to_string());
        expanded_env.insert("HOME".to_string(), dir.path().display().to_string());
        assert_eq!(
            run_nested(&nested, &expanded_env)?,
            format!("{dummy}|unset")
        );
    }

    std::fs::create_dir(dir.path().join("production"))?;
    let mut ordinary_env = env.clone();
    ordinary_env.insert("ENV".to_string(), "production".to_string());
    assert_eq!(
        run_nested(&nested, &ordinary_env)?,
        format!("{dummy}|production")
    );

    let application_env_file = dir.path().join(".env");
    std::fs::write(&application_env_file, "APPLICATION_SETTING=production\n")?;
    ordinary_env.insert(
        "ENV".to_string(),
        application_env_file.display().to_string(),
    );
    let application_command = vec![
        "/bin/bash".to_string(),
        "-c".to_string(),
        "printf '%s' \"$ENV\"".to_string(),
    ];
    let application_command = maybe_wrap_shell_lc_with_snapshot(
        &application_command,
        &shell,
        Some(&snapshot),
        &HashMap::new(),
        &ordinary_env,
        &RuntimePathPrepends::default(),
    );
    assert_eq!(
        run_nested(&application_command, &ordinary_env)?,
        application_env_file.display().to_string()
    );
    for flag in ["-c", "-lc"] {
        let posix_application_command = maybe_wrap_shell_lc_with_snapshot(
            &[
                "/bin/sh".to_string(),
                flag.to_string(),
                "printf '%s' \"$ENV\"".to_string(),
            ],
            &posix_shell,
            Some(&snapshot),
            &HashMap::new(),
            &ordinary_env,
            &RuntimePathPrepends::default(),
        );
        assert_eq!(
            run_nested(&posix_application_command, &ordinary_env)?,
            application_env_file.display().to_string()
        );
    }

    let changed_application_snapshot = dir.path().join("changed-application-snapshot.sh").abs();
    std::fs::write(
        &changed_application_snapshot,
        format!("export ENV='{}'\n", application_env_file.display()),
    )?;
    let bash_nested_command = wrap_bash_snapshot(&changed_application_snapshot, &env);
    assert_eq!(
        run_nested(&bash_nested_command, &env)?,
        format!("{dummy}|{}", application_env_file.display())
    );
    let posix_nested_command = wrap_snapshot(&changed_application_snapshot, &env);
    assert_eq!(
        run_nested(&posix_nested_command, &env)?,
        format!("{dummy}|{}", application_env_file.display())
    );
    let application_snapshot = dir.path().join("application-snapshot.sh");
    std::fs::write(&application_snapshot, "export ENV='production'\n")?;
    let application_snapshot = application_snapshot.abs();
    let application_command = wrap_snapshot(&application_snapshot, &env);
    assert_eq!(
        run_nested(&application_command, &env)?,
        format!("{dummy}|production")
    );
    let mut snapshot_env = env.clone();
    snapshot_env.remove("ENV");
    let application_command = wrap_snapshot(&application_snapshot, &snapshot_env);
    assert_eq!(
        run_nested(&application_command, &snapshot_env)?,
        format!("{dummy}|production")
    );

    if std::path::Path::new("/bin/zsh").exists() {
        std::fs::write(
            dir.path().join(".zshenv"),
            "export OPENAI_API_KEY='sk-startup-secret'\n",
        )?;
        let renamed_zsh = dir.path().join("zsh.exe");
        std::os::unix::fs::symlink("/bin/zsh", &renamed_zsh)?;
        env.insert("ZDOTDIR".to_string(), dir.path().display().to_string());
        let (zsh, _) = shell_with_snapshot(ShellType::Zsh, "/bin/zsh", snapshot.clone());
        prepare_brokered_shell_snapshot_env(&mut env, Some(&snapshot), &zsh);
        let command = vec![
            renamed_zsh.display().to_string(),
            "-lc".to_string(),
            "printf '%s\\n%s' \"$OPENAI_API_KEY\" \"$ZDOTDIR\"".to_string(),
        ];
        let rewritten = maybe_wrap_shell_lc_with_snapshot(
            &command,
            &zsh,
            Some(&snapshot),
            &HashMap::new(),
            &env,
            &RuntimePathPrepends::default(),
        );
        assert!(!rewritten[2].contains(&dummy));
        assert!(!rewritten[2].contains(&dummy_alias));
        assert_eq!(rewritten[1], "-lfc");
        let output = Command::new(&rewritten[0])
            .args(&rewritten[1..])
            .env_clear()
            .envs(&env)
            .output()?;
        assert!(output.status.success(), "zsh command failed: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("{dummy}\n{}", dir.path().display())
        );

        let system_startup_dir = dir.path().join("system-startup");
        std::fs::create_dir(&system_startup_dir)?;
        let system_startup_zsh = system_startup_dir.join("zsh");
        std::fs::write(
            &system_startup_zsh,
            "#!/bin/sh\nexport OPENAI_API_KEY='sk-system-startup-secret'\nexport AUTH_HEADER='Bearer sk-system-startup-secret'\nexec /bin/zsh -ax \"$@\"\n",
        )?;
        let mut permissions = std::fs::metadata(&system_startup_zsh)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&system_startup_zsh, permissions)?;
        let (system_zsh, _) = shell_with_snapshot(
            ShellType::Zsh,
            system_startup_zsh.to_string_lossy().as_ref(),
            snapshot.clone(),
        );
        for (mut startup_env, expected_canonical, expected_alias) in [
            (alias_only_env, "unset", dummy_alias.as_str()),
            (empty_canonical_env, "", dummy_alias.as_str()),
            (absent_alias_env, dummy.as_str(), "unset"),
        ] {
            prepare_brokered_shell_snapshot_env(&mut startup_env, Some(&snapshot), &system_zsh);
            let command = vec![
                system_startup_zsh.display().to_string(),
                "-c".to_string(),
                format!(
                    "set +x\nprintf '%s\\n%s\\n%s\\n%s\\n%s\\n' \"${{OPENAI_API_KEY-unset}}\" \"${{AUTH_HEADER-unset}}\" \"${{{SNAPSHOT_BROKERED_VALUE_ENV_PREFIX}AUTH_HEADER-unset}}\" \"${{{SNAPSHOT_BROKERED_VALUE_ENV_PREFIX}OPENAI_API_KEY-unset}}\" \"${{{SNAPSHOT_BROKERED_UNSET_ENV_PREFIX}OPENAI_API_KEY-unset}}\"\nenv -0"
                ),
            ];
            let system_rewritten = maybe_wrap_shell_lc_with_snapshot(
                &command,
                &system_zsh,
                Some(&snapshot),
                &HashMap::new(),
                &startup_env,
                &RuntimePathPrepends::default(),
            );
            let output = Command::new(&system_rewritten[0])
                .args(&system_rewritten[1..])
                .env_clear()
                .envs(&startup_env)
                .output()?;
            assert!(
                output.status.success(),
                "system startup command failed: {output:?}"
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            let exported_env = stdout
                .strip_prefix(&format!(
                    "{expected_canonical}\n{expected_alias}\nunset\nunset\nunset\n"
                ))
                .unwrap_or_else(|| panic!("unexpected system startup output: {stdout}"));
            assert!(!stdout.contains("sk-system-startup-secret"));
            assert!(
                !exported_env
                    .split('\0')
                    .any(|entry| entry.starts_with("__CODEX_SNAPSHOT_PROXY_OVERRIDE_"))
            );
            assert!(!String::from_utf8_lossy(&output.stderr).contains("sk-system-startup-secret"));
        }

        for (bash_env, posix_env, zdotdir) in [
            (
                "$ZDOTDIR/startup.sh",
                "$ZDOTDIR/startup.sh",
                Some(dir.path()),
            ),
            ("${ZDOTDIR:-$HOME}/startup.sh", "$HOME/startup.sh", None),
        ] {
            let mut nested_zsh_env = env.clone();
            nested_zsh_env.remove(SNAPSHOT_ORIGINAL_ZDOTDIR_ENV_KEY);
            nested_zsh_env.insert("HOME".to_string(), dir.path().display().to_string());
            nested_zsh_env.insert("BASH_ENV".to_string(), bash_env.to_string());
            nested_zsh_env.insert("ENV".to_string(), posix_env.to_string());
            if let Some(zdotdir) = zdotdir {
                nested_zsh_env.insert("ZDOTDIR".to_string(), zdotdir.display().to_string());
            } else {
                nested_zsh_env.remove("ZDOTDIR");
            }
            prepare_brokered_shell_snapshot_env(&mut nested_zsh_env, Some(&snapshot), &zsh);
            let command = wrap_brokered_snapshot(&zsh, &snapshot, &nested_zsh_env, &no_prepends);
            assert_eq!(
                run_nested(&command, &nested_zsh_env)?,
                format!("{dummy}|unset")
            );
        }

        env.remove("ZDOTDIR");
        env.remove(SNAPSHOT_ORIGINAL_ZDOTDIR_ENV_KEY);
        prepare_brokered_shell_snapshot_env(&mut env, Some(&snapshot), &zsh);
        let output = Command::new(&rewritten[0])
            .args(&rewritten[1..])
            .env_clear()
            .envs(env)
            .output()?;
        assert!(output.status.success(), "zsh command failed: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("{dummy}\n{}", dir.path().display())
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn maybe_wrap_shell_lc_with_snapshot_refreshes_codex_proxy_git_ssh_command() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    let stale_command = format!(
        "{CODEX_PROXY_GIT_SSH_COMMAND_MARKER}ssh -o ProxyCommand='nc -X 5 -x 127.0.0.1:8081 %h %p'"
    );
    let fresh_command = format!(
        "{CODEX_PROXY_GIT_SSH_COMMAND_MARKER}ssh -o ProxyCommand='nc -X 5 -x 127.0.0.1:48081 %h %p'"
    );
    std::fs::write(
        &snapshot_path,
        format!(
            "# Snapshot file\nexport {PROXY_GIT_SSH_COMMAND_ENV_KEY}='{}'\n",
            shell_single_quote(&stale_command)
        ),
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        format!("printf '%s' \"${PROXY_GIT_SSH_COMMAND_ENV_KEY}\""),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env(PROXY_GIT_SSH_COMMAND_ENV_KEY, &fresh_command)
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), fresh_command);
}

#[cfg(target_os = "macos")]
#[test]
fn maybe_wrap_shell_lc_with_snapshot_restores_custom_git_ssh_command() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    let stale_command = format!(
        "{CODEX_PROXY_GIT_SSH_COMMAND_MARKER}ssh -o ProxyCommand='nc -X 5 -x 127.0.0.1:8081 %h %p'"
    );
    let custom_command = "ssh -o ProxyCommand='tsh proxy ssh --cluster=dev %r@%h:%p'";
    std::fs::write(
        &snapshot_path,
        format!(
            "# Snapshot file\nexport {PROXY_GIT_SSH_COMMAND_ENV_KEY}='{}'\n",
            shell_single_quote(&stale_command)
        ),
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        format!("printf '%s' \"${PROXY_GIT_SSH_COMMAND_ENV_KEY}\""),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env(PROXY_GIT_SSH_COMMAND_ENV_KEY, custom_command)
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), custom_command);
}

#[cfg(target_os = "macos")]
#[test]
fn maybe_wrap_shell_lc_with_snapshot_clears_stale_codex_git_ssh_command_without_live_command() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    let stale_command = format!(
        "{CODEX_PROXY_GIT_SSH_COMMAND_MARKER}ssh -o ProxyCommand='nc -X 5 -x 127.0.0.1:8081 %h %p'"
    );
    std::fs::write(
        &snapshot_path,
        format!(
            "# Snapshot file\nexport {PROXY_GIT_SSH_COMMAND_ENV_KEY}='{}'\n",
            shell_single_quote(&stale_command)
        ),
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        format!(
            "if [ \"${{{PROXY_GIT_SSH_COMMAND_ENV_KEY}+x}}\" = x ]; then printf 'set'; else printf 'unset'; fi"
        ),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env_remove(PROXY_GIT_SSH_COMMAND_ENV_KEY)
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "unset");
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_keeps_user_proxy_env_when_proxy_inactive() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport HTTP_PROXY='http://user.proxy:8080'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$HTTP_PROXY\"".to_string(),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );
    let mut command = Command::new(&rewritten[0]);
    command.args(&rewritten[1..]);
    for key in PROXY_ENV_KEYS {
        command.env_remove(key);
    }
    let output = command.output().expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "http://user.proxy:8080"
    );
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_restores_live_env_when_snapshot_proxy_active() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        format!(
            "# Snapshot file\n\
             export {PROXY_ACTIVE_ENV_KEY}='1'\n\
             export PIP_PROXY='http://127.0.0.1:8080'\n\
             export HTTP_PROXY='http://127.0.0.1:8080'\n"
        ),
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        format!(
            "if [ \"${{PIP_PROXY+x}}\" = x ]; then printf 'pip:%s\\n' \"$PIP_PROXY\"; else printf 'pip:unset\\n'; fi; \
             printf 'http:%s\\n' \"$HTTP_PROXY\"; \
             if [ \"${{{PROXY_ACTIVE_ENV_KEY}+x}}\" = x ]; then printf 'active:%s' \"${PROXY_ACTIVE_ENV_KEY}\"; else printf 'active:unset'; fi"
        ),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::from([(
            "HTTP_PROXY".to_string(),
            "http://user.proxy:8080".to_string(),
        )]),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env("HTTP_PROXY", "http://user.proxy:8080")
        .env_remove("PIP_PROXY")
        .env_remove(PROXY_ACTIVE_ENV_KEY)
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "pip:unset\nhttp:http://user.proxy:8080\nactive:unset"
    );
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_keeps_snapshot_path_without_override() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport PATH='/snapshot/bin'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$PATH\"".to_string(),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "/snapshot/bin");
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_applies_explicit_path_override() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport PATH='/snapshot/bin'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$PATH\"".to_string(),
    ];
    let explicit_env_overrides = HashMap::from([("PATH".to_string(), "/worktree/bin".to_string())]);
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &explicit_env_overrides,
        &HashMap::from([("PATH".to_string(), "/worktree/bin".to_string())]),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env("PATH", "/worktree/bin")
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "/worktree/bin");
}

#[cfg(unix)]
#[test]
fn maybe_wrap_shell_lc_with_snapshot_preserves_package_path_prepend() -> anyhow::Result<()> {
    let (stdout, package_path_dir) =
        run_snapshot_path_probe_with_runtime_path_prepend(HashMap::new())?;

    assert_eq!(
        stdout,
        format!("{}:/snapshot/bin", package_path_dir.display()),
        "package path prepend should replay ahead of snapshot PATH"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn maybe_wrap_shell_lc_with_snapshot_applies_runtime_path_prepend_after_explicit_path_override()
-> anyhow::Result<()> {
    let (stdout, package_path_dir) = run_snapshot_path_probe_with_runtime_path_prepend(
        HashMap::from([("PATH".to_string(), "/worktree/bin".to_string())]),
    )?;

    assert_eq!(
        stdout,
        format!("{}:/worktree/bin", package_path_dir.display()),
        "explicit PATH override should suppress snapshot PATH while preserving runtime prepend"
    );
    Ok(())
}

#[cfg(unix)]
fn run_snapshot_path_probe_with_runtime_path_prepend(
    explicit_env_overrides: HashMap<String, String>,
) -> anyhow::Result<(String, PathBuf)> {
    let dir = tempdir()?;
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport PATH='/snapshot/bin'\n",
    )?;
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$PATH\"".to_string(),
    ];
    let package_path_dir = dir.path().join("codex-path");
    let mut env = HashMap::from([("PATH".to_string(), "/worktree/bin".to_string())]);
    let mut runtime_path_prepends = RuntimePathPrepends::default();
    runtime_path_prepends.prepend(&mut env, package_path_dir.as_path());
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &explicit_env_overrides,
        &env,
        &runtime_path_prepends,
    );
    let path = env
        .get("PATH")
        .ok_or_else(|| anyhow::anyhow!("PATH should be set"))?;
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env("PATH", path)
        .output()?;

    assert!(output.status.success(), "command failed: {output:?}");
    Ok((
        String::from_utf8_lossy(&output.stdout).into_owned(),
        package_path_dir,
    ))
}

#[cfg(unix)]
#[test]
fn maybe_wrap_shell_lc_with_snapshot_preserves_zsh_fork_path_prepend() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport PATH='/snapshot/bin'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$PATH\"".to_string(),
    ];
    let zsh_path = dir
        .path()
        .join("codex-resources")
        .join("zsh")
        .join("bin")
        .join("zsh");
    let zsh_bin_dir = zsh_path.parent().expect("zsh path should have parent");
    let mut env = HashMap::from([("PATH".to_string(), "/worktree/bin".to_string())]);
    let explicit_env_overrides = HashMap::new();
    let mut runtime_path_prepends = RuntimePathPrepends::default();
    apply_zsh_fork_path_prepend(&mut env, &mut runtime_path_prepends, zsh_path.as_path());
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &explicit_env_overrides,
        &env,
        &runtime_path_prepends,
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env("PATH", env.get("PATH").expect("PATH should be set"))
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}:/snapshot/bin", zsh_bin_dir.display()),
        "zsh fork path prepend should replay ahead of snapshot PATH"
    );
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_does_not_embed_override_values_in_argv() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport OPENAI_API_KEY='snapshot-value'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$OPENAI_API_KEY\"".to_string(),
    ];
    let explicit_env_overrides = HashMap::from([
        (
            "OPENAI_API_KEY".to_string(),
            "super-secret-value".to_string(),
        ),
        (
            "openai_identity_token_file".to_string(),
            "/run/identity-token".to_string(),
        ),
    ]);
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &explicit_env_overrides,
        &HashMap::from([(
            "OPENAI_API_KEY".to_string(),
            "super-secret-value".to_string(),
        )]),
        &RuntimePathPrepends::default(),
    );

    assert!(!rewritten[2].contains("super-secret-value"));
    assert!(!rewritten[2].contains("openai_identity_token_file"));
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env("OPENAI_API_KEY", "super-secret-value")
        .output()
        .expect("run rewritten command");
    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "super-secret-value"
    );
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_preserves_unset_override_variables() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport CODEX_TEST_UNSET_OVERRIDE='snapshot-value'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "if [ \"${CODEX_TEST_UNSET_OVERRIDE+x}\" = x ]; then printf 'set:%s' \"$CODEX_TEST_UNSET_OVERRIDE\"; else printf 'unset'; fi".to_string(),
        ];
    let explicit_env_overrides = HashMap::from([(
        "CODEX_TEST_UNSET_OVERRIDE".to_string(),
        "worktree-value".to_string(),
    )]);
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &explicit_env_overrides,
        &HashMap::new(),
        &RuntimePathPrepends::default(),
    );

    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env_remove("CODEX_TEST_UNSET_OVERRIDE")
        .output()
        .expect("run rewritten command");
    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "unset");
}

#[test]
fn brokered_credential_exports_stop_when_startup_makes_credential_readonly() {
    let copy_key = format!("{SNAPSHOT_BROKERED_VALUE_ENV_PREFIX}OPENAI_API_KEY");
    let env = HashMap::from([(copy_key.clone(), "dummy-openai-key".to_string())]);
    let restore = build_brokered_credential_exports(&env, /*remove_copies*/ true);

    let unset_output = Command::new("/bin/bash")
        .args([
            "-c",
            &format!(
                "unset OPENAI_API_KEY\n{restore}\nprintf '%s|%s' \"${{OPENAI_API_KEY-unset}}\" \"${{{copy_key}-unset}}\""
            ),
        ])
        .env(&copy_key, "dummy-openai-key")
        .output()
        .expect("run unset credential probe");
    assert!(
        unset_output.status.success(),
        "unset credential probe failed: {unset_output:?}"
    );
    assert_eq!(String::from_utf8_lossy(&unset_output.stdout), "unset|unset");

    let empty_output = Command::new("/bin/bash")
        .args([
            "-c",
            &format!(
                "OPENAI_API_KEY=\n{restore}\nprintf '%s|%s' \"$OPENAI_API_KEY\" \"${{{copy_key}-unset}}\""
            ),
        ])
        .env(&copy_key, "dummy-openai-key")
        .output()
        .expect("run empty credential probe");
    assert!(
        empty_output.status.success(),
        "empty credential probe failed: {empty_output:?}"
    );
    assert_eq!(String::from_utf8_lossy(&empty_output.stdout), "|unset");

    let unchanged_output = Command::new("/bin/bash")
        .args([
            "-c",
            &format!(
                "readonly OPENAI_API_KEY=dummy-openai-key\n{restore}\nprintf '%s|%s' \"$OPENAI_API_KEY\" \"${{{copy_key}-unset}}\""
            ),
        ])
        .env(&copy_key, "dummy-openai-key")
        .output()
        .expect("run unchanged readonly credential probe");
    assert!(
        unchanged_output.status.success(),
        "unchanged readonly credential probe failed: {unchanged_output:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&unchanged_output.stdout),
        "dummy-openai-key|unset"
    );

    let xtrace_output = Command::new("/bin/bash")
        .args([
            "-c",
            &format!(
                "OPENAI_API_KEY=real-openai-key\nset -x\n{restore}\nset +x\nprintf '%s|%s' \"$OPENAI_API_KEY\" \"${{{copy_key}-unset}}\""
            ),
        ])
        .env(&copy_key, "dummy-openai-key")
        .output()
        .expect("run xtrace credential probe");
    assert!(
        xtrace_output.status.success(),
        "xtrace credential probe failed: {xtrace_output:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&xtrace_output.stdout),
        "dummy-openai-key|unset"
    );
    assert!(!String::from_utf8_lossy(&xtrace_output.stderr).contains("real-openai-key"));

    let unset_marker_key = format!("{SNAPSHOT_BROKERED_UNSET_ENV_PREFIX}OPENAI_API_KEY");
    let unset_env = HashMap::from([(unset_marker_key.clone(), "1".to_string())]);
    let unset_restore = build_brokered_credential_exports(&unset_env, /*remove_copies*/ true);
    let readonly_empty_output = Command::new("/bin/bash")
        .args([
            "-c",
            &format!(
                "readonly OPENAI_API_KEY=\n{unset_restore}\nprintf '%s|%s' \"$OPENAI_API_KEY\" \"${{{unset_marker_key}-unset}}\""
            ),
        ])
        .env(&unset_marker_key, "1")
        .output()
        .expect("run readonly empty credential probe");
    assert!(
        readonly_empty_output.status.success(),
        "readonly empty credential probe failed: {readonly_empty_output:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&readonly_empty_output.stdout),
        "|unset"
    );

    let output = Command::new("/bin/bash")
        .args([
            "-c",
            &format!("readonly OPENAI_API_KEY=real-openai-key\n{restore}\nprintf should-not-run"),
        ])
        .env(copy_key, "dummy-openai-key")
        .output()
        .expect("run readonly credential probe");

    assert!(!output.status.success(), "command unexpectedly succeeded");
    assert!(
        output.stdout.is_empty(),
        "protected command unexpectedly ran"
    );
}
