/*
Module: runtimes

Concrete ToolRuntime implementations for specific tools. Each runtime stays
small and focused and reuses the orchestrator for approvals + sandbox + retry.
*/
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::path_utils;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxPermissions;
use crate::shell::Shell;
use crate::skills::SkillMetadata;
use crate::tools::sandboxing::ToolError;
use codex_protocol::models::PermissionProfile;
use std::collections::HashMap;
use std::path::Path;

pub mod apply_patch;
pub mod shell;
pub mod unified_exec;

#[derive(Debug, Clone)]
pub(crate) struct ExecveSessionApproval {
    /// If this execve session approval is associated with a skill script, this
    /// field contains metadata about the skill.
    #[cfg_attr(not(unix), allow(dead_code))]
    pub skill: Option<SkillMetadata>,
}

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
        capture_policy: ExecCapturePolicy::ShellTool,
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
    if command.len() < 3 {
        return command.to_vec();
    }

    let flag = command[1].as_str();
    if flag != "-lc" {
        return command.to_vec();
    }

    let shell_path = session_shell.shell_path.to_string_lossy();
    let original_shell = shell_single_quote(&command[0]);
    let original_script = shell_single_quote(&command[2]);
    let trailing_args = command[3..]
        .iter()
        .map(|arg| format!(" '{}'", shell_single_quote(arg)))
        .collect::<String>();
    let Some(snapshot_restore_preamble) =
        maybe_build_snapshot_restore_preamble(session_shell, cwd, explicit_env_overrides)
    else {
        return command.to_vec();
    };
    let rewritten_script = format!(
        "{snapshot_restore_preamble}\n\nexec '{original_shell}' -c '{original_script}'{trailing_args}"
    );

    vec![shell_path.to_string(), "-c".to_string(), rewritten_script]
}

pub(crate) fn maybe_build_snapshot_restore_preamble(
    session_shell: &Shell,
    cwd: &Path,
    explicit_env_overrides: &HashMap<String, String>,
) -> Option<String> {
    if cfg!(windows) {
        return None;
    }

    let snapshot = session_shell.shell_snapshot()?;
    if !snapshot.path.exists() {
        return None;
    }

    let snapshot_matches_cwd = if let (Ok(snapshot_cwd), Ok(command_cwd)) = (
        path_utils::normalize_for_path_comparison(snapshot.cwd.as_path()),
        path_utils::normalize_for_path_comparison(cwd),
    ) {
        snapshot_cwd == command_cwd
    } else {
        snapshot.cwd == cwd
    };
    if !snapshot_matches_cwd {
        return None;
    }

    let snapshot_path = shell_single_quote(snapshot.path.to_string_lossy().as_ref());
    let (override_captures, override_exports) = build_override_exports(explicit_env_overrides);
    let source_snapshot = format!("if . '{snapshot_path}' >/dev/null 2>&1; then :; fi");
    Some(if override_exports.is_empty() {
        source_snapshot
    } else {
        format!("{override_captures}\n\n{source_snapshot}\n\n{override_exports}")
    })
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
#[path = "mod_tests.rs"]
mod tests;
