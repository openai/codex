/*
Module: runtimes

Concrete ToolRuntime implementations for specific tools. Each runtime stays
small and focused and reuses the orchestrator for approvals + sandbox + retry.
*/
use crate::exec_env::CODEX_THREAD_ID_ENV_VAR;
use crate::path_utils;
use crate::shell::Shell;
use crate::tools::sandboxing::ToolError;
#[cfg(target_os = "macos")]
use codex_network_proxy::MANAGED_GIT_SSH_COMMAND_ENV_KEY;
#[cfg(target_os = "macos")]
use codex_network_proxy::MANAGED_GIT_SSH_COMMAND_MARKER;
use codex_network_proxy::MANAGED_PROXY_ENV_ACTIVE_ENV_KEY;
use codex_network_proxy::MANAGED_PROXY_ENV_KEYS;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::SandboxCommand;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;

pub(crate) mod apply_patch;
pub(crate) mod shell;
pub(crate) mod unified_exec;

/// Shared helper to construct sandbox transform inputs from a tokenized command line.
/// Validates that at least a program is present.
pub(crate) fn build_sandbox_command(
    command: &[String],
    cwd: &AbsolutePathBuf,
    env: &HashMap<String, String>,
    additional_permissions: Option<PermissionProfile>,
) -> Result<SandboxCommand, ToolError> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| ToolError::Rejected("command args are empty".to_string()))?;
    Ok(SandboxCommand {
        program: program.clone().into(),
        args: args.to_vec(),
        cwd: cwd.clone(),
        env: env.clone(),
        additional_permissions,
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
///
/// `explicit_env_overrides` and `env` are intentionally separate inputs.
/// `explicit_env_overrides` contains policy-driven shell env overrides that
/// should win after the snapshot is sourced, while `env` is the full live exec
/// environment. We need access to both so snapshot restore logic can preserve
/// runtime-only vars like `CODEX_THREAD_ID` without pretending they came from
/// the explicit override policy.
pub(crate) fn maybe_wrap_shell_lc_with_snapshot(
    command: &[String],
    session_shell: &Shell,
    cwd: &Path,
    explicit_env_overrides: &HashMap<String, String>,
    env: &HashMap<String, String>,
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
    let mut override_env = explicit_env_overrides.clone();
    if let Some(thread_id) = env.get(CODEX_THREAD_ID_ENV_VAR) {
        override_env.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.clone());
    }
    let (override_captures, override_exports) = build_override_exports(&override_env);
    let (proxy_captures, proxy_exports) = build_managed_proxy_env_exports();
    let override_captures = join_shell_blocks([override_captures, proxy_captures]);
    let override_exports = join_shell_blocks([override_exports, proxy_exports]);
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
        .map(String::as_str)
        .filter(|key| is_valid_shell_variable_name(key))
        .collect::<Vec<_>>();
    keys.sort_unstable();

    build_override_exports_for_keys("__CODEX_SNAPSHOT_OVERRIDE", &keys)
}

fn build_managed_proxy_env_exports() -> (String, String) {
    let mut keys = MANAGED_PROXY_ENV_KEYS
        .iter()
        .copied()
        .filter(|key| is_valid_shell_variable_name(key))
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();

    let (captures, restores) =
        build_override_exports_for_keys("__CODEX_SNAPSHOT_PROXY_OVERRIDE", &keys);
    let key = MANAGED_PROXY_ENV_ACTIVE_ENV_KEY;
    let proxy_blocks = (
        format!(
            "__CODEX_SNAPSHOT_PROXY_ENV_SET=\"${{{key}+x}}\"\nif [ -n \"$__CODEX_SNAPSHOT_PROXY_ENV_SET\" ]; then\n{captures}\nfi"
        ),
        format!("if [ -n \"$__CODEX_SNAPSHOT_PROXY_ENV_SET\" ]; then\n{restores}\nfi"),
    );
    let git_blocks = build_managed_git_ssh_command_exports();
    (
        join_shell_blocks([proxy_blocks.0, git_blocks.0]),
        join_shell_blocks([proxy_blocks.1, git_blocks.1]),
    )
}

#[cfg(target_os = "macos")]
fn build_managed_git_ssh_command_exports() -> (String, String) {
    let key = MANAGED_GIT_SSH_COMMAND_ENV_KEY;
    let marker_pattern = format!("{}\\ *", MANAGED_GIT_SSH_COMMAND_MARKER.trim_end());
    (
        format!(
            "__CODEX_SNAPSHOT_MANAGED_GIT_SSH_COMMAND=\"${{{key}-}}\"\ncase \"$__CODEX_SNAPSHOT_MANAGED_GIT_SSH_COMMAND\" in\n  {marker_pattern}) __CODEX_SNAPSHOT_MANAGED_GIT_SSH_COMMAND_SET=1 ;;\n  *) __CODEX_SNAPSHOT_MANAGED_GIT_SSH_COMMAND_SET= ;;\nesac"
        ),
        format!(
            "if [ -n \"$__CODEX_SNAPSHOT_MANAGED_GIT_SSH_COMMAND_SET\" ]; then\n  case \"${{{key}-}}\" in\n    {marker_pattern}) export {key}=\"$__CODEX_SNAPSHOT_MANAGED_GIT_SSH_COMMAND\" ;;\n    *) if [ -z \"${{{key}+x}}\" ]; then export {key}=\"$__CODEX_SNAPSHOT_MANAGED_GIT_SSH_COMMAND\"; fi ;;\n  esac\nfi"
        ),
    )
}

#[cfg(not(target_os = "macos"))]
fn build_managed_git_ssh_command_exports() -> (String, String) {
    (String::new(), String::new())
}

fn build_override_exports_for_keys(variable_prefix: &str, keys: &[&str]) -> (String, String) {
    if keys.is_empty() {
        return (String::new(), String::new());
    }

    let captures = keys
        .iter()
        .enumerate()
        .map(|(idx, key)| {
            let set_var = format!("{variable_prefix}_SET_{idx}");
            let value_var = format!("{variable_prefix}_{idx}");
            format!("{set_var}=\"${{{key}+x}}\"\n{value_var}=\"${{{key}-}}\"")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let restores = keys
        .iter()
        .enumerate()
        .map(|(idx, key)| {
            let set_var = format!("{variable_prefix}_SET_{idx}");
            let value_var = format!("{variable_prefix}_{idx}");
            format!(
                "if [ -n \"${{{set_var}}}\" ]; then export {key}=\"${{{value_var}}}\"; else unset {key}; fi"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    (captures, restores)
}

fn join_shell_blocks(blocks: impl IntoIterator<Item = String>) -> String {
    blocks
        .into_iter()
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
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
#[path = "mod_tests.rs"]
mod tests;
