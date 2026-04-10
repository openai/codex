use super::*;
use crate::shell::ShellType;
use crate::shell_snapshot::ShellSnapshot;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::watch;

fn shell_with_snapshot(
    shell_type: ShellType,
    shell_path: &str,
    snapshot_path: PathBuf,
    snapshot_cwd: PathBuf,
) -> Shell {
    let (_tx, shell_snapshot) = watch::channel(Some(Arc::new(ShellSnapshot {
        path: snapshot_path,
        cwd: snapshot_cwd,
    })));
    Shell {
        shell_type,
        shell_path: PathBuf::from(shell_path),
        shell_snapshot,
    }
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_bootstraps_in_user_shell() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, "# Snapshot file\n").expect("write snapshot");
    let session_shell = shell_with_snapshot(
        ShellType::Zsh,
        "/bin/zsh",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "echo hello".to_string(),
    ];

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        dir.path(),
        &HashMap::new(),
        &HashMap::new(),
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
    let session_shell = shell_with_snapshot(
        ShellType::Zsh,
        "/bin/zsh",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "echo 'hello'".to_string(),
    ];

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        dir.path(),
        &HashMap::new(),
        &HashMap::new(),
    );

    assert!(rewritten[2].contains(r#"exec '/bin/bash' -c 'echo '"'"'hello'"'"''"#));
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_uses_bash_bootstrap_shell() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, "# Snapshot file\n").expect("write snapshot");
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/zsh".to_string(),
        "-lc".to_string(),
        "echo hello".to_string(),
    ];

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        dir.path(),
        &HashMap::new(),
        &HashMap::new(),
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
    let session_shell = shell_with_snapshot(
        ShellType::Sh,
        "/bin/sh",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "echo hello".to_string(),
    ];

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        dir.path(),
        &HashMap::new(),
        &HashMap::new(),
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
    let session_shell = shell_with_snapshot(
        ShellType::Zsh,
        "/bin/zsh",
        snapshot_path,
        dir.path().to_path_buf(),
    );
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
        dir.path(),
        &HashMap::new(),
        &HashMap::new(),
    );

    assert!(
        rewritten[2]
            .contains(r#"exec '/bin/bash' -c 'printf '"'"'%s %s'"'"' "$0" "$1"' 'arg0' 'arg1'"#)
    );
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_skips_when_cwd_mismatch() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, "# Snapshot file\n").expect("write snapshot");
    let snapshot_cwd = dir.path().join("worktree-a");
    let command_cwd = dir.path().join("worktree-b");
    std::fs::create_dir_all(&snapshot_cwd).expect("create snapshot cwd");
    std::fs::create_dir_all(&command_cwd).expect("create command cwd");
    let session_shell =
        shell_with_snapshot(ShellType::Zsh, "/bin/zsh", snapshot_path, snapshot_cwd);
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "echo hello".to_string(),
    ];

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        &command_cwd,
        &HashMap::new(),
        &HashMap::new(),
    );

    assert_eq!(rewritten, command);
}

#[test]
fn maybe_wrap_shell_lc_with_snapshot_accepts_dot_alias_cwd() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, "# Snapshot file\n").expect("write snapshot");
    let session_shell = shell_with_snapshot(
        ShellType::Zsh,
        "/bin/zsh",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "echo hello".to_string(),
    ];
    let command_cwd = dir.path().join(".");

    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        &command_cwd,
        &HashMap::new(),
        &HashMap::new(),
    );

    assert_eq!(rewritten[0], "/bin/zsh");
    assert_eq!(rewritten[1], "-c");
    assert!(rewritten[2].contains("if . '"));
    assert!(rewritten[2].contains("exec '/bin/bash' -c 'echo hello'"));
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
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path,
        dir.path().to_path_buf(),
    );
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
        dir.path(),
        &explicit_env_overrides,
        &HashMap::from([("TEST_ENV_SNAPSHOT".to_string(), "worktree".to_string())]),
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
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$CODEX_THREAD_ID\"".to_string(),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        dir.path(),
        &HashMap::new(),
        &HashMap::from([("CODEX_THREAD_ID".to_string(), "nested-thread".to_string())]),
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
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s\\n%s\\n%s\\n%s' \"$PIP_PROXY\" \"$HTTP_PROXY\" \"$http_proxy\" \"$GIT_SSH_COMMAND\""
            .to_string(),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        dir.path(),
        &HashMap::new(),
        &HashMap::new(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
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

#[test]
fn maybe_wrap_shell_lc_with_snapshot_keeps_snapshot_proxy_env_without_process_proxy_env() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\n\
         export PIP_PROXY='http://127.0.0.1:8080'\n\
         export HTTP_PROXY='http://127.0.0.1:8080'\n",
    )
    .expect("write snapshot");
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s\\n%s' \"$PIP_PROXY\" \"$HTTP_PROXY\"".to_string(),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        dir.path(),
        &HashMap::new(),
        &HashMap::new(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env_remove("PIP_PROXY")
        .env_remove("HTTP_PROXY")
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "http://127.0.0.1:8080\n\
         http://127.0.0.1:8080"
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
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        format!("printf '%s' \"${PROXY_GIT_SSH_COMMAND_ENV_KEY}\""),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        dir.path(),
        &HashMap::new(),
        &HashMap::new(),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env(PROXY_GIT_SSH_COMMAND_ENV_KEY, &fresh_command)
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), fresh_command);
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
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$PATH\"".to_string(),
    ];
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        dir.path(),
        &HashMap::new(),
        &HashMap::new(),
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
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path,
        dir.path().to_path_buf(),
    );
    let command = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "printf '%s' \"$PATH\"".to_string(),
    ];
    let explicit_env_overrides = HashMap::from([("PATH".to_string(), "/worktree/bin".to_string())]);
    let rewritten = maybe_wrap_shell_lc_with_snapshot(
        &command,
        &session_shell,
        dir.path(),
        &explicit_env_overrides,
        &HashMap::from([("PATH".to_string(), "/worktree/bin".to_string())]),
    );
    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env("PATH", "/worktree/bin")
        .output()
        .expect("run rewritten command");

    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "/worktree/bin");
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
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path,
        dir.path().to_path_buf(),
    );
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
        dir.path(),
        &explicit_env_overrides,
        &HashMap::from([(
            "OPENAI_API_KEY".to_string(),
            "super-secret-value".to_string(),
        )]),
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
    let session_shell = shell_with_snapshot(
        ShellType::Bash,
        "/bin/bash",
        snapshot_path,
        dir.path().to_path_buf(),
    );
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
        dir.path(),
        &explicit_env_overrides,
        &HashMap::new(),
    );

    let output = Command::new(&rewritten[0])
        .args(&rewritten[1..])
        .env_remove("CODEX_TEST_UNSET_OVERRIDE")
        .output()
        .expect("run rewritten command");
    assert!(output.status.success(), "command failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "unset");
}
