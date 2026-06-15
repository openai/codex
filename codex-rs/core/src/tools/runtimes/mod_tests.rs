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
    let state = codex_network_proxy::build_config_state(
        NetworkProxyConfig::default(),
        NetworkProxyConstraints::default(),
    )?;
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

    let command = vec!["/bin/echo".to_string(), "ok".to_string()];
    let command = build_sandbox_command(
        &command,
        &command_cwd,
        &exec_env_for_sandbox_permissions(&env, SandboxPermissions::RequireEscalated),
        /*additional_permissions*/ None,
    )
    .expect("build sandbox command");
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
        permissions: &permissions,
        enforce_managed_network: false,
        manager: &manager,
        sandbox_cwd: &sandbox_policy_cwd,
        workspace_roots: std::slice::from_ref(&native_sandbox_policy_cwd),
        codex_linux_sandbox_exe: None,
        use_legacy_landlock: false,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        network_denial_cancellation_token: None,
    };

    let exec_request = attempt
        .env_for(
            command,
            options,
            managed_network_for_sandbox_permissions(
                Some(&proxy),
                SandboxPermissions::RequireEscalated,
            ),
        )
        .expect("prepare exec request");

    assert_eq!(exec_request.cwd, command_cwd);
    assert_eq!(
        exec_request.windows_sandbox_policy_cwd,
        native_sandbox_policy_cwd
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
fn prepend_zsh_fork_bin_to_path_ignores_empty_parent() {
    let mut env = HashMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]);

    let result = prepend_zsh_fork_bin_to_path(&mut env, PathBuf::from("zsh").as_path());

    assert_eq!(
        result, None,
        "zsh fork helper should not report a PATH update for an empty parent"
    );
    assert_eq!(
        env.get("PATH").map(String::as_str),
        Some("/usr/bin:/bin"),
        "zsh fork helper should leave PATH unchanged when the parent is empty"
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
fn maybe_wrap_shell_lc_with_snapshot_restores_codex_thread_id_from_env() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport CODEX_THREAD_ID='parent-thread'\n",
    )
    .expect("write snapshot");
    let (session_shell, shell_snapshot) =
        shell_with_snapshot(ShellType::Bash, "/bin/bash", snapshot_path.abs());
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$CODEX_THREAD_ID\"".to_string(),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        Some(&shell_snapshot),
        &HashMap::new(),
        &HashMap::from([("CODEX_THREAD_ID".to_string(), "nested-thread".to_string())]),
        &RuntimePathPrepends::default(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env("CODEX_THREAD_ID", "nested-thread")
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "nested-thread");
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
    let explicit_env_overrides = HashMap::from([(
        "OPENAI_API_KEY".to_string(),
        "super-secret-value".to_string(),
    )]);
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
