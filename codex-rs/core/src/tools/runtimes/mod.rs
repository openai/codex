/*
Module: runtimes

Concrete ToolRuntime implementations for specific tools. Each runtime stays
small and focused and reuses the orchestrator for approvals + sandbox + retry.
*/
use crate::exec_env::CODEX_THREAD_ID_ENV_VAR;
use crate::path_utils;
use crate::sandboxing::SandboxPermissions;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::tools::sandboxing::ToolError;
#[cfg(unix)]
use codex_install_context::InstallContext;
#[cfg(target_os = "macos")]
use codex_network_proxy::CODEX_PROXY_GIT_SSH_COMMAND_MARKER;
use codex_network_proxy::CUSTOM_CA_ENV_KEYS;
use codex_network_proxy::PROXY_ACTIVE_ENV_KEY;
use codex_network_proxy::PROXY_ENV_KEYS;
#[cfg(target_os = "macos")]
use codex_network_proxy::PROXY_GIT_SSH_COMMAND_ENV_KEY;
use codex_network_proxy::is_managed_mitm_ca_trust_bundle_path;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxType;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
#[cfg(unix)]
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
    additional_permissions: Option<AdditionalPermissionProfile>,
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

pub(crate) fn exec_env_for_sandbox_permissions(
    env: &HashMap<String, String>,
    sandbox_permissions: SandboxPermissions,
) -> HashMap<String, String> {
    let mut env = env.clone();
    if sandbox_permissions.requires_escalated_permissions()
        && env.contains_key(PROXY_ACTIVE_ENV_KEY)
    {
        strip_managed_proxy_env(&mut env);
    }
    env
}

pub(crate) fn strip_managed_proxy_env(env: &mut HashMap<String, String>) {
    for key in PROXY_ENV_KEYS {
        env.remove(*key);
    }
    for key in CUSTOM_CA_ENV_KEYS {
        if env
            .get(key)
            .is_some_and(|value| is_managed_mitm_ca_trust_bundle_path(value))
        {
            env.remove(key);
        }
    }
    // Only macOS injects a Codex-owned SSH wrapper for the managed SOCKS proxy.
    #[cfg(target_os = "macos")]
    if env
        .get(PROXY_GIT_SSH_COMMAND_ENV_KEY)
        .is_some_and(|command| command.starts_with(CODEX_PROXY_GIT_SSH_COMMAND_MARKER))
    {
        env.remove(PROXY_GIT_SSH_COMMAND_ENV_KEY);
    }
}

/// Prepends `path_entry` to `PATH`, removing duplicate and empty existing
/// entries.
///
/// Returns the updated `PATH` value when `env` was changed. Returns `None` when
/// `path_entry` is empty, leaving `env` untouched so an empty entry does not add
/// the current working directory to command lookup.
#[cfg(unix)]
fn prepend_path_entry(env: &mut HashMap<String, String>, path_entry: &str) -> Option<String> {
    if path_entry.is_empty() {
        None
    } else {
        let updated_path = match env.get("PATH") {
            Some(path) if !path.is_empty() => std::iter::once(path_entry)
                .chain(
                    path.split(':')
                        .filter(|entry| !entry.is_empty() && *entry != path_entry),
                )
                .collect::<Vec<_>>()
                .join(":"),
            _ => path_entry.to_string(),
        };
        env.insert("PATH".to_string(), updated_path.clone());
        Some(updated_path)
    }
}

/// PATH entries owned by Codex runtime setup.
///
/// These are applied to the live exec environment immediately and replayed after
/// restoring a shell snapshot, unless the user explicitly overrides `PATH`.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimePathPrepends {
    entries: Vec<String>,
}

impl RuntimePathPrepends {
    #[cfg(unix)]
    pub(crate) fn prepend(&mut self, env: &mut HashMap<String, String>, path_entry: &Path) {
        let path_entry = path_entry.to_string_lossy().to_string();
        if prepend_path_entry(env, &path_entry).is_some() {
            self.entries.retain(|entry| entry != &path_entry);
            self.entries.push(path_entry);
        }
    }

    fn shell_exports_after_snapshot(
        &self,
        explicit_env_overrides: &HashMap<String, String>,
    ) -> String {
        if explicit_env_overrides.contains_key("PATH") {
            return String::new();
        }

        self.entries
            .iter()
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                let entry = shell_single_quote(entry);
                format!(
                    "if [ -n \"${{PATH:-}}\" ]; then export PATH='{entry}':\"$PATH\"; else export PATH='{entry}'; fi"
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(unix)]
pub(crate) fn apply_package_path_prepend(
    env: &mut HashMap<String, String>,
    runtime_path_prepends: &mut RuntimePathPrepends,
) {
    let Some(path_dir) = InstallContext::current()
        .package_layout
        .as_ref()
        .and_then(|package_layout| package_layout.path_dir.as_ref())
    else {
        return;
    };

    runtime_path_prepends.prepend(env, path_dir.as_path());
}

#[cfg(unix)]
pub(crate) fn prepend_zsh_fork_bin_to_path(
    env: &mut HashMap<String, String>,
    shell_zsh_path: &Path,
) -> Option<String> {
    let zsh_bin_dir = shell_zsh_path
        .parent()
        .map(|path| path.to_string_lossy().to_string())?;
    prepend_path_entry(env, &zsh_bin_dir)
}

#[cfg(unix)]
pub(crate) fn apply_zsh_fork_path_prepend(
    env: &mut HashMap<String, String>,
    runtime_path_prepends: &mut RuntimePathPrepends,
    shell_zsh_path: &Path,
) {
    let Some(zsh_bin_dir) = shell_zsh_path.parent() else {
        return;
    };
    runtime_path_prepends.prepend(env, zsh_bin_dir);
}

pub(crate) fn disable_powershell_profile_for_elevated_windows_sandbox(
    command: &[String],
    shell_type: Option<&ShellType>,
    sandbox: SandboxType,
    windows_sandbox_level: WindowsSandboxLevel,
    proxy_enforced: bool,
) -> Vec<String> {
    if shell_type != Some(&ShellType::PowerShell)
        || sandbox != SandboxType::WindowsRestrictedToken
        || !crate::exec::windows_sandbox_uses_elevated_backend(
            windows_sandbox_level,
            proxy_enforced,
        )
        || command.is_empty()
    {
        return command.to_vec();
    }

    if command[1..]
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("-NoProfile"))
    {
        return command.to_vec();
    }

    // The elevated Windows sandbox runs as a dedicated sandbox account while
    // HOME/USERPROFILE may still point at the real user profile. Loading
    // PowerShell profiles in that mixed context is not a valid login shell.
    let mut command = command.to_vec();
    command.insert(1, "-NoProfile".to_string());
    command
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
///
/// `runtime_path_prepends` contains Codex-owned PATH entries already applied to
/// the live `env`; snapshot wrapping replays them after restoring the snapshot
/// PATH unless the user explicitly overrides `PATH`.
pub(crate) fn maybe_wrap_shell_lc_with_snapshot(
    command: &[String],
    session_shell: &Shell,
    cwd: &AbsolutePathBuf,
    explicit_env_overrides: &HashMap<String, String>,
    env: &HashMap<String, String>,
    runtime_path_prepends: &RuntimePathPrepends,
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

    if !path_utils::paths_match_after_normalization(snapshot.cwd.as_path(), cwd) {
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
    let (proxy_captures, proxy_exports) = build_proxy_env_exports();
    let runtime_path_prepend_exports =
        runtime_path_prepends.shell_exports_after_snapshot(explicit_env_overrides);
    let override_captures = join_shell_blocks([override_captures, proxy_captures]);
    let override_exports = join_shell_blocks([
        override_exports,
        proxy_exports,
        runtime_path_prepend_exports,
    ]);
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

fn build_proxy_env_exports() -> (String, String) {
    let mut keys = PROXY_ENV_KEYS
        .iter()
        .copied()
        .chain(CUSTOM_CA_ENV_KEYS)
        .filter(|key| is_valid_shell_variable_name(key))
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();

    let (captures, restores) =
        build_override_exports_for_keys("__CODEX_SNAPSHOT_PROXY_OVERRIDE", &keys);
    let key = PROXY_ACTIVE_ENV_KEY;
    let proxy_blocks = (
        format!("{captures}\n__CODEX_SNAPSHOT_PROXY_ENV_SET=\"${{{key}+x}}\""),
        format!(
            "if [ -n \"$__CODEX_SNAPSHOT_PROXY_ENV_SET\" ] || [ -n \"${{{key}+x}}\" ]; then\n{restores}\nfi"
        ),
    );
    let git_blocks = build_codex_proxy_git_ssh_command_exports();
    (
        join_shell_blocks([proxy_blocks.0, git_blocks.0]),
        join_shell_blocks([proxy_blocks.1, git_blocks.1]),
    )
}

#[cfg(target_os = "macos")]
fn build_codex_proxy_git_ssh_command_exports() -> (String, String) {
    let key = PROXY_GIT_SSH_COMMAND_ENV_KEY;
    let marker_pattern = format!("{}\\ *", CODEX_PROXY_GIT_SSH_COMMAND_MARKER.trim_end());
    (
        format!(
            "__CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND_SET=\"${{{key}+x}}\"\n__CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND=\"${{{key}-}}\"\ncase \"$__CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND\" in\n  {marker_pattern}) __CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND_LIVE_MARKED=1 ;;\n  *) __CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND_LIVE_MARKED= ;;\nesac"
        ),
        format!(
            "case \"${{{key}-}}\" in\n  {marker_pattern}) __CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND_AFTER_MARKED=1 ;;\n  *) __CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND_AFTER_MARKED= ;;\nesac\nif [ -n \"$__CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND_LIVE_MARKED\" ]; then\n  if [ -z \"${{{key}+x}}\" ] || [ -n \"$__CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND_AFTER_MARKED\" ]; then\n    export {key}=\"$__CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND\"\n  fi\nelif [ -n \"$__CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND_AFTER_MARKED\" ]; then\n  if [ -n \"$__CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND_SET\" ]; then\n    export {key}=\"$__CODEX_SNAPSHOT_PROXY_GIT_SSH_COMMAND\"\n  else\n    unset {key}\n  fi\nfi"
        ),
    )
}

#[cfg(not(target_os = "macos"))]
fn build_codex_proxy_git_ssh_command_exports() -> (String, String) {
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

#[cfg(test)]
mod disable_powershell_profile_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn inserts_no_profile_for_elevated_windows_sandbox() {
        let command = vec![
            "powershell.exe".to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];

        let rewritten = disable_powershell_profile_for_elevated_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            SandboxType::WindowsRestrictedToken,
            WindowsSandboxLevel::Elevated,
            /*proxy_enforced*/ false,
        );

        assert_eq!(
            rewritten,
            vec![
                "powershell.exe".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Write-Output ok".to_string(),
            ]
        );
    }

    #[test]
    fn inserts_no_profile_before_encoded_command() {
        let command = vec![
            "powershell.exe".to_string(),
            "-EncodedCommand".to_string(),
            "VwByAGkAdABlAC0ATwB1AHQAcAB1AHQAIABvAGsA".to_string(),
        ];

        let rewritten = disable_powershell_profile_for_elevated_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            SandboxType::WindowsRestrictedToken,
            WindowsSandboxLevel::Elevated,
            /*proxy_enforced*/ false,
        );

        assert_eq!(
            rewritten,
            vec![
                "powershell.exe".to_string(),
                "-NoProfile".to_string(),
                "-EncodedCommand".to_string(),
                "VwByAGkAdABlAC0ATwB1AHQAcAB1AHQAIABvAGsA".to_string(),
            ]
        );
    }

    #[test]
    fn preserves_existing_no_profile() {
        let command = vec![
            "pwsh.exe".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];

        let rewritten = disable_powershell_profile_for_elevated_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            SandboxType::WindowsRestrictedToken,
            WindowsSandboxLevel::Elevated,
            /*proxy_enforced*/ false,
        );

        assert_eq!(rewritten, command);
    }

    #[test]
    fn inserts_no_profile_for_proxy_elevated_restricted_token_sandbox() {
        let command = vec![
            "powershell.exe".to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];

        let rewritten = disable_powershell_profile_for_elevated_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            SandboxType::WindowsRestrictedToken,
            WindowsSandboxLevel::RestrictedToken,
            /*proxy_enforced*/ true,
        );

        assert_eq!(
            rewritten,
            vec![
                "powershell.exe".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Write-Output ok".to_string(),
            ]
        );
    }

    #[test]
    fn leaves_legacy_restricted_token_backend_alone() {
        let command = vec![
            "powershell.exe".to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];

        let rewritten = disable_powershell_profile_for_elevated_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            SandboxType::WindowsRestrictedToken,
            WindowsSandboxLevel::RestrictedToken,
            /*proxy_enforced*/ false,
        );

        assert_eq!(rewritten, command);
    }

    #[test]
    fn leaves_unsandboxed_attempts_alone() {
        let command = vec![
            "powershell.exe".to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];

        let rewritten = disable_powershell_profile_for_elevated_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            SandboxType::None,
            WindowsSandboxLevel::Elevated,
            /*proxy_enforced*/ false,
        );

        assert_eq!(rewritten, command);
    }

    #[test]
    fn leaves_non_powershell_alone() {
        let command = vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "echo ok".to_string(),
        ];

        let rewritten = disable_powershell_profile_for_elevated_windows_sandbox(
            &command,
            Some(&ShellType::Bash),
            SandboxType::WindowsRestrictedToken,
            WindowsSandboxLevel::Elevated,
            /*proxy_enforced*/ false,
        );

        assert_eq!(rewritten, command);
    }
}

#[cfg(all(test, unix))]
#[path = "mod_tests.rs"]
mod tests;
