/*
Module: runtimes

Concrete ToolRuntime implementations for specific tools. Each runtime stays
small and focused and reuses the orchestrator for approvals + sandbox + retry.
*/
use crate::exec::ExecExpiration;
use crate::path_utils;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxPermissions;
use crate::shell::Shell;
use crate::tools::sandboxing::ToolError;
use codex_protocol::models::PermissionProfile;
use std::collections::HashMap;
use std::path::Path;

pub mod apply_patch;
pub mod shell;
pub mod unified_exec;

/// Shared helper to construct a CommandSpec from a tokenized command line.
/// Validates that at least a program is present.
pub(crate) fn build_command_spec(
    command: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    expiration: ExecExpiration,
    sandbox_permissions: SandboxPermissions,
    additional_permissions: Option<PermissionProfile>,
    justification: Option<String>,
) -> Result<CommandSpec, ToolError> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| ToolError::Rejected("command args are empty".to_string()))?;
    Ok(CommandSpec {
        program: program.clone(),
        args: args.to_vec(),
        cwd: cwd.to_path_buf(),
        env: env.clone(),
        expiration,
        sandbox_permissions,
        additional_permissions,
        justification,
    })
}

/// POSIX-only helper: for commands produced by `Shell::derive_exec_args`
/// for Bash/Zsh/sh of the form `[shell_path, "-lc", "<script>"]`, and
/// when a snapshot is configured on the session shell, rewrite the argv
/// to a single non-login shell that sources the snapshot before running
/// the original script:
///
///   shell -lc "<script>"
///   => user_shell -c ". SNAPSHOT (best effort); exec shell -c <script>"
///
/// This wrapper script uses POSIX constructs (`if`, `.`, `exec`) so it can
/// be run by Bash/Zsh/sh. On non-matching commands, or when command cwd does
/// not match the snapshot cwd, this is a no-op.
pub(crate) fn maybe_wrap_shell_lc_with_snapshot(
    command: &[String],
    session_shell: &Shell,
    cwd: &Path,
    explicit_env_overrides: &HashMap<String, String>,
) -> Vec<String> {
    if cfg!(windows) {
        return command.to_vec();
    }

    let Some(snapshot) = session_shell.shell_snapshot() else {
        return command.to_vec();
    };

    if !snapshot.path.exists() {
        return command.to_vec();
    }

    if if let (Ok(snapshot_cwd), Ok(command_cwd)) = (
        path_utils::normalize_for_path_comparison(snapshot.cwd.as_path()),
        path_utils::normalize_for_path_comparison(cwd),
    ) {
        snapshot_cwd != command_cwd
    } else {
        snapshot.cwd != cwd
    } {
        return command.to_vec();
    }

    if command.len() < 3 {
        return command.to_vec();
    }

    let flag = command[1].as_str();
    if flag != "-lc" {
        return command.to_vec();
    }

    let snapshot_path = snapshot.path.to_string_lossy();
    let shell_path = session_shell.shell_path.to_string_lossy();
    let original_shell = shell_single_quote(&command[0]);
    let original_script = shell_single_quote(&command[2]);
    let snapshot_path = shell_single_quote(snapshot_path.as_ref());
    let trailing_args = command[3..]
        .iter()
        .map(|arg| format!(" '{}'", shell_single_quote(arg)))
        .collect::<String>();
    let (override_captures, override_exports) = build_override_exports(explicit_env_overrides);
    let rewritten_script = if override_exports.is_empty() {
        format!(
            "if . '{snapshot_path}' >/dev/null 2>&1; then :; fi\n\nexec '{original_shell}' -c '{original_script}'{trailing_args}"
        )
    } else {
        format!(
            "{override_captures}\n\nif . '{snapshot_path}' >/dev/null 2>&1; then :; fi\n\n{override_exports}\n\nexec '{original_shell}' -c '{original_script}'{trailing_args}"
        )
    };

    vec![shell_path.to_string(), "-c".to_string(), rewritten_script]
}

fn build_override_exports(explicit_env_overrides: &HashMap<String, String>) -> (String, String) {
    let mut keys = explicit_env_overrides
        .keys()
        .filter(|key| is_valid_shell_variable_name(key))
        .collect::<Vec<_>>();
    keys.sort_unstable();

    if keys.is_empty() {
        return (String::new(), String::new());
    }

    let captures = keys
        .iter()
        .enumerate()
        .map(|(idx, key)| {
            format!(
                "__CODEX_SNAPSHOT_OVERRIDE_SET_{idx}=\"${{{key}+x}}\"\n__CODEX_SNAPSHOT_OVERRIDE_{idx}=\"${{{key}-}}\""
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let restores = keys
        .iter()
        .enumerate()
        .map(|(idx, key)| {
            format!(
                "if [ -n \"${{__CODEX_SNAPSHOT_OVERRIDE_SET_{idx}}}\" ]; then export {key}=\"${{__CODEX_SNAPSHOT_OVERRIDE_{idx}}}\"; else unset {key}; fi"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    (captures, restores)
}

fn is_valid_shell_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn shell_single_quote(input: &str) -> String {
    input.replace('\'', r#"'"'"'"#)
}

#[cfg(all(test, unix))]
mod tests {
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
        );

        assert!(
            rewritten[2].contains(
                r#"exec '/bin/bash' -c 'printf '"'"'%s %s'"'"' "$0" "$1"' 'arg0' 'arg1'"#
            )
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
        let explicit_env_overrides =
            HashMap::from([("PATH".to_string(), "/worktree/bin".to_string())]);
        let rewritten = maybe_wrap_shell_lc_with_snapshot(
            &command,
            &session_shell,
            dir.path(),
            &explicit_env_overrides,
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
        );

        let output = Command::new(&rewritten[0])
            .args(&rewritten[1..])
            .env_remove("CODEX_TEST_UNSET_OVERRIDE")
            .output()
            .expect("run rewritten command");
        assert!(output.status.success(), "command failed: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "unset");
    }
}
