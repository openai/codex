/*
Module: runtimes

Concrete ToolRuntime implementations for specific tools. Each runtime stays
small and focused and reuses the orchestrator for approvals + sandbox + retry.
*/
use crate::exec_env::CODEX_PERMISSION_PROFILE_ENV_VAR;
use crate::exec_env::CODEX_SESSION_ID_ENV_VAR;
use crate::exec_env::CODEX_THREAD_ID_ENV_VAR;
use crate::exec_env::CODEX_TOOL_CALL_ID_ENV_VAR;
use crate::exec_env::CODEX_VERSION_ENV_VAR;
use crate::sandboxing::SandboxPermissions;
use crate::shell::Shell;
use crate::shell::ShellType;
use codex_apply_patch::CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR;
use codex_core_plugins::PLUGIN_METRICS_OUTPUT_ENV_VAR;
use codex_file_system::WindowsSandboxSelection;
#[cfg(unix)]
use codex_install_context::InstallContext;
#[cfg(target_os = "macos")]
use codex_network_proxy::CODEX_PROXY_GIT_SSH_COMMAND_MARKER;
use codex_network_proxy::CREDENTIAL_BROKER_ACTIVE_ENV_KEY;
use codex_network_proxy::CUSTOM_CA_ENV_KEYS;
use codex_network_proxy::PROXY_ACTIVE_ENV_KEY;
use codex_network_proxy::PROXY_ENV_KEYS;
#[cfg(target_os = "macos")]
use codex_network_proxy::PROXY_GIT_SSH_COMMAND_ENV_KEY;
pub(crate) use codex_network_proxy::is_managed_proxy_env_var;
pub(crate) use codex_network_proxy::strip_managed_proxy_env;
use codex_protocol::shell_environment::is_non_inheritable_env_var;
use codex_shell_command::shell_snapshot::posix_env_path_expansion_function;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;

pub(crate) mod apply_patch;
pub(crate) mod unified_exec;
pub(crate) mod zsh_fork;

const SNAPSHOT_ORIGINAL_BASH_ENV_ENV_KEY: &str = "CODEX_NETWORK_PROXY_SNAPSHOT_ORIGINAL_BASH_ENV";
const SNAPSHOT_ORIGINAL_POSIX_ENV_ENV_KEY: &str = "CODEX_NETWORK_PROXY_SNAPSHOT_ORIGINAL_POSIX_ENV";
const SNAPSHOT_ORIGINAL_ZDOTDIR_ENV_KEY: &str = "CODEX_NETWORK_PROXY_SNAPSHOT_ORIGINAL_ZDOTDIR";
const SNAPSHOT_BROKERED_VALUE_ENV_PREFIX: &str = "CODEX_NETWORK_PROXY_SNAPSHOT_BROKERED_VALUE_";
const SNAPSHOT_BROKERED_UNSET_ENV_PREFIX: &str = "CODEX_NETWORK_PROXY_SNAPSHOT_BROKERED_UNSET_";

pub(crate) fn exec_env_for_sandbox_permissions(
    env: &HashMap<String, String>,
    sandbox_permissions: SandboxPermissions,
) -> HashMap<String, String> {
    let mut env = env.clone();
    // Escalated commands intentionally use the original, unbrokered environment. This branch is
    // defensive cleanup for a caller that passes an environment already prepared by Codex.
    if sandbox_permissions.requires_escalated_permissions()
        && env.contains_key(PROXY_ACTIVE_ENV_KEY)
    {
        strip_managed_proxy_env(&mut env);
    }
    env
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

pub(crate) fn prepare_powershell_command_for_windows_sandbox(
    command: &[String],
    shell_type: Option<&ShellType>,
    sandbox_requested: bool,
    windows_sandbox: WindowsSandboxSelection,
    environment_is_remote: bool,
) -> Vec<String> {
    prepare_powershell_command_for_windows_sandbox_with_fallback(
        command,
        shell_type,
        sandbox_requested,
        windows_sandbox,
        environment_is_remote,
        codex_shell_command::shell_detect::fallback_powershell_shell_for_windows_sandbox,
    )
}

fn prepare_powershell_command_for_windows_sandbox_with_fallback(
    command: &[String],
    shell_type: Option<&ShellType>,
    sandbox_requested: bool,
    windows_sandbox: WindowsSandboxSelection,
    environment_is_remote: bool,
    find_fallback: impl FnOnce(&Path) -> Option<codex_shell_command::shell_detect::DetectedShell>,
) -> Vec<String> {
    if shell_type != Some(&ShellType::PowerShell)
        || !sandbox_requested
        || !matches!(
            windows_sandbox,
            WindowsSandboxSelection::Elevated | WindowsSandboxSelection::Mxc
        )
        || command.is_empty()
    {
        return command.to_vec();
    }

    let mut command = command.to_vec();
    if !environment_is_remote && let Some(fallback) = find_fallback(Path::new(&command[0])) {
        command[0] = fallback.shell_path.to_string_lossy().to_string();
    }

    if windows_sandbox != WindowsSandboxSelection::Elevated
        || command[1..]
            .iter()
            .any(|arg| arg.eq_ignore_ascii_case("-NoProfile"))
    {
        return command;
    }

    // The elevated Windows sandbox runs as a dedicated sandbox account while
    // HOME/USERPROFILE may still point at the real user profile. Loading
    // PowerShell profiles in that mixed context is not a valid login shell.
    command.insert(1, "-NoProfile".to_string());
    command
}

pub(crate) fn prepare_brokered_shell_snapshot_env(
    env: &mut HashMap<String, String>,
    shell_snapshot: Option<&AbsolutePathBuf>,
    shell: &Shell,
) {
    if shell_snapshot.is_some()
        && env
            .get(CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
            .is_some_and(|active| active == "1")
    {
        if !cfg!(windows) {
            env.retain(|key, _| {
                !key.starts_with(SNAPSHOT_BROKERED_VALUE_ENV_PREFIX)
                    && !key.starts_with(SNAPSHOT_BROKERED_UNSET_ENV_PREFIX)
            });
            for key in codex_network_proxy::brokered_credential_value_env_keys(env) {
                if is_valid_shell_variable_name(&key)
                    && let Some(value) = env.get(&key).cloned()
                {
                    env.insert(format!("{SNAPSHOT_BROKERED_VALUE_ENV_PREFIX}{key}"), value);
                }
            }
            for key in codex_network_proxy::brokered_credential_marker_env_keys(env) {
                if !is_valid_shell_variable_name(&key) {
                    continue;
                }
                if let Some(value) = env.get(&key).cloned() {
                    env.entry(format!("{SNAPSHOT_BROKERED_VALUE_ENV_PREFIX}{key}"))
                        .or_insert(value);
                } else {
                    env.insert(
                        format!("{SNAPSHOT_BROKERED_UNSET_ENV_PREFIX}{key}"),
                        "1".to_string(),
                    );
                }
            }
        }
        let bash_env = env.remove("BASH_ENV");
        if !cfg!(windows)
            && let Some(bash_env) = bash_env
            && !bash_env.is_empty()
        {
            env.insert(SNAPSHOT_ORIGINAL_BASH_ENV_ENV_KEY.to_string(), bash_env);
        } else {
            env.remove(SNAPSHOT_ORIGINAL_BASH_ENV_ENV_KEY);
        }
        if shell.shell_type == ShellType::Zsh {
            if let Some(zdotdir) = env.remove("ZDOTDIR") {
                env.insert(SNAPSHOT_ORIGINAL_ZDOTDIR_ENV_KEY.to_string(), zdotdir);
            } else {
                env.remove(SNAPSHOT_ORIGINAL_ZDOTDIR_ENV_KEY);
            }
            env.insert("ZDOTDIR".to_string(), "/dev/null".to_string());
        }
    }
}

/// POSIX-only helper: for commands produced by `Shell::derive_exec_args`
/// for Bash/Zsh/sh of the form `[shell_path, "-lc", "<script>"]` or brokered
/// `[shell_path, "-c", "<script>"]`, and
/// when a snapshot is configured on the session shell, rewrite the argv
/// to a single non-login shell that sources the snapshot before running
/// the original script:
///
///   shell -lc "<script>"
///   => user_shell -c ". SNAPSHOT (best effort); exec shell -c <script>"
///
/// Brokered Bash and Zsh commands targeting the session shell run in that process
/// to preserve captured shell functions and avoid asking patched Zsh to exec itself.
/// Brokered Zsh wrappers use `-f` so startup files cannot overwrite credentials
/// after the proxy has replaced them with dummies.
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
    shell_snapshot: Option<&AbsolutePathBuf>,
    explicit_env_overrides: &HashMap<String, String>,
    env: &HashMap<String, String>,
    runtime_path_prepends: &RuntimePathPrepends,
) -> Vec<String> {
    if cfg!(windows) {
        return command.to_vec();
    }

    let Some(snapshot) = shell_snapshot else {
        return command.to_vec();
    };

    if !snapshot.exists() {
        return command.to_vec();
    }

    if command.len() < 3 {
        return command.to_vec();
    }

    let brokered = env
        .get(CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
        .is_some_and(|active| active == "1");
    let flag = command[1].as_str();
    if flag != "-lc" && !(brokered && flag == "-c") {
        return command.to_vec();
    }

    let snapshot_path = snapshot.to_string_lossy();
    let shell_path = session_shell.shell_path.to_string_lossy();
    let command_uses_session_zsh =
        session_shell.shell_type == ShellType::Zsh && command[0] == shell_path.as_ref();
    let reuse_session_shell = brokered
        && command[0] == shell_path.as_ref()
        && matches!(session_shell.shell_type, ShellType::Bash | ShellType::Zsh);
    let original_shell_is_zsh = command_uses_session_zsh
        || codex_shell_command::shell_detect::detect_shell_type(&command[0])
            == Some(ShellType::Zsh);
    let brokered_zsh_flag = if flag == "-lc" { "-lfc" } else { "-fc" };
    let original_shell_flag = if brokered && original_shell_is_zsh {
        brokered_zsh_flag
    } else {
        "-c"
    };
    let original_shell = shell_single_quote(&command[0]);
    let snapshot_path = shell_single_quote(snapshot_path.as_ref());
    let trailing_args = command[3..]
        .iter()
        .map(|arg| format!(" '{}'", shell_single_quote(arg)))
        .collect::<String>();
    let mut override_env = explicit_env_overrides.clone();
    for key in [
        CODEX_SESSION_ID_ENV_VAR,
        CODEX_THREAD_ID_ENV_VAR,
        CODEX_TOOL_CALL_ID_ENV_VAR,
        CODEX_VERSION_ENV_VAR,
        CODEX_PERMISSION_PROFILE_ENV_VAR,
        CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR,
        PLUGIN_METRICS_OUTPUT_ENV_VAR,
    ] {
        if let Some(value) = env.get(key) {
            override_env.insert(key.to_string(), value.clone());
        }
    }
    // Do not let a snapshot resurrect stale runtime state when it is inactive.
    let (override_captures, override_exports) = build_override_exports(
        &override_env,
        &[
            CODEX_PERMISSION_PROFILE_ENV_VAR,
            CODEX_APPLY_PATCH_PRESERVE_LINE_ENDINGS_ENV_VAR,
            PLUGIN_METRICS_OUTPUT_ENV_VAR,
            CODEX_TOOL_CALL_ID_ENV_VAR,
        ],
    );
    let (proxy_captures, proxy_exports) = build_proxy_env_exports(env);
    let (env_captures, replayed_startup_capture, env_exports) = if brokered {
        let bash_env_key = SNAPSHOT_ORIGINAL_BASH_ENV_ENV_KEY;
        let posix_env_key = SNAPSHOT_ORIGINAL_POSIX_ENV_ENV_KEY;
        let startup_env_key =
            if session_shell.shell_type == ShellType::Sh && env.contains_key(posix_env_key) {
                posix_env_key
            } else {
                bash_env_key
            };
        let alternate_startup_env_key = if startup_env_key == posix_env_key {
            bash_env_key
        } else {
            posix_env_key
        };
        let zdotdir_key = SNAPSHOT_ORIGINAL_ZDOTDIR_ENV_KEY;
        let expand_env_function = posix_env_path_expansion_function();
        (
            format!(
                r#"__CODEX_SNAPSHOT_ORIGINAL_ENV_SET="${{ENV+x}}"
__CODEX_SNAPSHOT_ORIGINAL_ENV="${{ENV-}}"
{expand_env_function}
__codex_snapshot_expand_env_with_zdotdir() (
  if [ -n "${{{zdotdir_key}+x}}" ]; then
    export ZDOTDIR="${{{zdotdir_key}}}"
  elif [ -n "${{ZSH_VERSION-}}" ] && [ "${{ZDOTDIR-}}" = /dev/null ]; then
    unset ZDOTDIR
  fi
  __codex_snapshot_expand_env "$1"
)
__CODEX_SNAPSHOT_PROTECTED_ENV=$(
  __codex_snapshot_expand_env_with_zdotdir "${{{startup_env_key}-}}"
)
__CODEX_SNAPSHOT_ALTERNATE_PROTECTED_ENV=$(
  __codex_snapshot_expand_env_with_zdotdir "${{{alternate_startup_env_key}-}}"
)"#
            ),
            "__CODEX_SNAPSHOT_REPLAYED_BASH_ENV=\"${BASH_ENV-}\"".to_string(),
            format!(
                r#"__CODEX_SNAPSHOT_CURRENT_ENV=$(
  __codex_snapshot_expand_env_with_zdotdir "${{ENV-}}"
)
__CODEX_SNAPSHOT_ORIGINAL_EXPANDED_ENV=$(
  __codex_snapshot_expand_env_with_zdotdir "$__CODEX_SNAPSHOT_ORIGINAL_ENV"
)
__CODEX_SNAPSHOT_REPLAYED_PROTECTED_ENV=$(
  __codex_snapshot_expand_env_with_zdotdir "$__CODEX_SNAPSHOT_REPLAYED_BASH_ENV"
)
unset -f __codex_snapshot_expand_env __codex_snapshot_expand_env_with_zdotdir
__codex_snapshot_env_is_protected() (
  for __codex_protected_env in \
    "$__CODEX_SNAPSHOT_PROTECTED_ENV" \
    "$__CODEX_SNAPSHOT_ALTERNATE_PROTECTED_ENV" \
    "$__CODEX_SNAPSHOT_REPLAYED_PROTECTED_ENV"; do
    if [ -n "$__codex_protected_env" ] &&
      {{ [ "$1" = "$__codex_protected_env" ] ||
        [ "$1" -ef "$__codex_protected_env" ] 2>/dev/null; }}; then
      return 0
    fi
  done
  return 1
)
if __codex_snapshot_env_is_protected "$__CODEX_SNAPSHOT_CURRENT_ENV"; then
  if [ -n "$__CODEX_SNAPSHOT_ORIGINAL_ENV_SET" ] &&
    ! __codex_snapshot_env_is_protected "$__CODEX_SNAPSHOT_ORIGINAL_EXPANDED_ENV"; then
    builtin export ENV="$__CODEX_SNAPSHOT_ORIGINAL_ENV" 2>/dev/null ||
      command export ENV="$__CODEX_SNAPSHOT_ORIGINAL_ENV" || exit 1
    [ "${{ENV-}}" = "$__CODEX_SNAPSHOT_ORIGINAL_ENV" ] || exit 1
  else
    builtin unset ENV 2>/dev/null || command unset ENV || exit 1
    [ -z "${{ENV+x}}" ] || exit 1
  fi
fi
unset -f __codex_snapshot_env_is_protected
unset __CODEX_SNAPSHOT_ORIGINAL_ENV_SET __CODEX_SNAPSHOT_ORIGINAL_ENV \
  __CODEX_SNAPSHOT_PROTECTED_ENV __CODEX_SNAPSHOT_ALTERNATE_PROTECTED_ENV \
  __CODEX_SNAPSHOT_REPLAYED_BASH_ENV __CODEX_SNAPSHOT_REPLAYED_PROTECTED_ENV \
  __CODEX_SNAPSHOT_CURRENT_ENV __CODEX_SNAPSHOT_ORIGINAL_EXPANDED_ENV \
  {bash_env_key} {posix_env_key}"#
            ),
        )
    } else {
        (String::new(), String::new(), String::new())
    };
    let zsh_startup_exports = if brokered && session_shell.shell_type == ShellType::Zsh {
        let key = SNAPSHOT_ORIGINAL_ZDOTDIR_ENV_KEY;
        format!(
            "if [ -n \"${{{key}+x}}\" ]; then\n  export ZDOTDIR=\"${{{key}}}\"\nelif [ \"${{ZDOTDIR-}}\" = /dev/null ]; then\n  unset ZDOTDIR\nfi\nunset {key}"
        )
    } else {
        String::new()
    };
    // Zsh always reads the global zshenv, even with `-f`. Keep private copies of child-visible
    // dummy values until the command shell has finished startup, then remove the copies.
    let outer_brokered_credential_exports = if brokered {
        build_brokered_credential_exports(env, /*remove_copies*/ false)
    } else {
        String::new()
    };
    let inner_brokered_credential_exports = if brokered {
        build_brokered_credential_exports(env, /*remove_copies*/ true)
    } else {
        String::new()
    };
    let original_script = if !inner_brokered_credential_exports.is_empty() {
        format!("{inner_brokered_credential_exports}\n{}", command[2])
    } else {
        command[2].clone()
    };
    let runtime_path_prepend_exports =
        runtime_path_prepends.shell_exports_after_snapshot(explicit_env_overrides);
    let override_captures = join_shell_blocks([
        outer_brokered_credential_exports.clone(),
        override_captures,
        proxy_captures,
        env_captures,
    ]);
    let override_exports = join_shell_blocks([
        outer_brokered_credential_exports.clone(),
        replayed_startup_capture,
        override_exports,
        proxy_exports,
        runtime_path_prepend_exports,
        env_exports,
        zsh_startup_exports,
        outer_brokered_credential_exports,
    ]);
    let run_original = if reuse_session_shell {
        original_script
    } else {
        let original_script = shell_single_quote(&original_script);
        format!("exec '{original_shell}' {original_shell_flag} '{original_script}'{trailing_args}")
    };
    let rewritten_script = if override_exports.is_empty() {
        format!("if . '{snapshot_path}' >/dev/null 2>&1; then :; fi\n\n{run_original}")
    } else {
        format!(
            "{override_captures}\n\nif . '{snapshot_path}' >/dev/null 2>&1; then :; fi\n\n{override_exports}\n\n{run_original}"
        )
    };

    let wrapper_flag = if brokered && session_shell.shell_type == ShellType::Zsh {
        brokered_zsh_flag
    } else {
        "-c"
    };
    let mut rewritten = vec![
        shell_path.to_string(),
        wrapper_flag.to_string(),
        rewritten_script,
    ];
    if reuse_session_shell {
        rewritten.extend_from_slice(&command[3..]);
    }
    rewritten
}

fn build_brokered_credential_exports(env: &HashMap<String, String>, remove_copies: bool) -> String {
    let mut value_copies = env
        .keys()
        .filter_map(|copy_key| {
            let key = copy_key.strip_prefix(SNAPSHOT_BROKERED_VALUE_ENV_PREFIX)?;
            is_valid_shell_variable_name(key).then_some((key, copy_key))
        })
        .collect::<Vec<_>>();
    value_copies.sort_unstable();
    let value_exports = value_copies
        .into_iter()
        .map(|(key, copy_key)| {
            let restore = format!(
                "if [ -z \"${{{copy_key}+x}}\" ]; then exit 1; fi\nif [ -n \"${{{key}+x}}\" ] && [ -n \"${{{key}}}\" ] && [ \"${{{key}}}\" != \"${{{copy_key}}}\" ]; then export {key}=\"${{{copy_key}}}\" || exit 1; fi\nif [ -n \"${{{key}+x}}\" ] && [ -n \"${{{key}}}\" ] && [ \"${{{key}}}\" != \"${{{copy_key}}}\" ]; then exit 1; fi"
            );
            if remove_copies {
                format!("{restore}\nunset {copy_key} || exit 1")
            } else {
                restore
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut unset_markers = env
        .keys()
        .filter_map(|marker_key| {
            let key = marker_key.strip_prefix(SNAPSHOT_BROKERED_UNSET_ENV_PREFIX)?;
            is_valid_shell_variable_name(key).then_some((key, marker_key))
        })
        .collect::<Vec<_>>();
    unset_markers.sort_unstable();
    let unset_exports = unset_markers
        .into_iter()
        .map(|(key, marker_key)| {
            let restore = format!(
                "if [ -z \"${{{marker_key}+x}}\" ]; then exit 1; fi\nif [ -n \"${{{key}+x}}\" ] && [ -n \"${{{key}}}\" ]; then unset {key} || exit 1; fi\nif [ -n \"${{{key}+x}}\" ] && [ -n \"${{{key}}}\" ]; then exit 1; fi"
            );
            if remove_copies {
                format!("{restore}\nunset {marker_key} || exit 1")
            } else {
                restore
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let exports = join_shell_blocks([value_exports, unset_exports]);
    if exports.is_empty() {
        return exports;
    }
    format!(
        "case $- in\n  *x*) __CODEX_SNAPSHOT_BROKER_XTRACE=1; set +x ;;\n  *) __CODEX_SNAPSHOT_BROKER_XTRACE= ;;\nesac\n{exports}\nif [ -n \"$__CODEX_SNAPSHOT_BROKER_XTRACE\" ]; then\n  unset __CODEX_SNAPSHOT_BROKER_XTRACE\n  set -x\nelse\n  unset __CODEX_SNAPSHOT_BROKER_XTRACE\nfi"
    )
}

fn build_override_exports(
    explicit_env_overrides: &HashMap<String, String>,
    restore_even_when_absent: &[&str],
) -> (String, String) {
    let mut keys = explicit_env_overrides
        .keys()
        .map(String::as_str)
        .chain(restore_even_when_absent.iter().copied())
        .filter(|key| !is_non_inheritable_env_var(key))
        .filter(|key| is_valid_shell_variable_name(key))
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();

    build_override_exports_for_keys("__CODEX_SNAPSHOT_OVERRIDE", &keys)
}

fn build_proxy_env_exports(env: &HashMap<String, String>) -> (String, String) {
    let mut keys = PROXY_ENV_KEYS
        .iter()
        .copied()
        .chain(codex_network_proxy::brokered_credential_env_keys(env))
        .chain(CUSTOM_CA_ENV_KEYS)
        .chain(
            env.get(CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
                .filter(|active| active.as_str() == "1")
                .map(|_| "BASH_ENV"),
        )
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
                "if [ -n \"${{{set_var}}}\" ]; then\n  if [ -z \"${{{key}+x}}\" ] || [ \"${{{key}-}}\" != \"${{{value_var}}}\" ]; then export {key}=\"${{{value_var}}}\"; else export {key}; fi\nelse builtin unset {key} 2>/dev/null || command unset {key}; fi\nbuiltin unset {set_var} {value_var} 2>/dev/null || command unset {set_var} {value_var}"
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
mod prepare_powershell_command_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn inserts_no_profile_for_elevated_windows_sandbox() {
        let command = vec![
            "powershell.exe".to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];

        let rewritten = prepare_powershell_command_for_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            /*sandbox_requested*/ true,
            WindowsSandboxSelection::Elevated,
            /*environment_is_remote*/ false,
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

        let rewritten = prepare_powershell_command_for_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            /*sandbox_requested*/ true,
            WindowsSandboxSelection::Elevated,
            /*environment_is_remote*/ false,
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

    #[test_case::test_case(WindowsSandboxSelection::Elevated; "elevated")]
    #[test_case::test_case(WindowsSandboxSelection::Mxc; "mxc")]
    fn preserves_existing_no_profile(windows_sandbox: WindowsSandboxSelection) {
        let command = vec![
            "pwsh.exe".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];

        let rewritten = prepare_powershell_command_for_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            /*sandbox_requested*/ true,
            windows_sandbox,
            /*environment_is_remote*/ false,
        );

        assert_eq!(rewritten, command);
    }

    #[test]
    fn leaves_legacy_restricted_token_backend_alone() {
        let command = vec![
            r"C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.4.0_x64__8wekyb3d8bbwe\pwsh.exe"
                .to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];

        let rewritten = prepare_powershell_command_for_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            /*sandbox_requested*/ true,
            WindowsSandboxSelection::RestrictedToken,
            /*environment_is_remote*/ false,
        );

        assert_eq!(rewritten, command);
    }

    #[test_case::test_case(WindowsSandboxSelection::Elevated; "elevated")]
    #[test_case::test_case(WindowsSandboxSelection::Mxc; "mxc")]
    fn leaves_unsandboxed_attempts_alone(windows_sandbox: WindowsSandboxSelection) {
        let command = vec![
            r"C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.4.0_x64__8wekyb3d8bbwe\pwsh.exe"
                .to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];

        let rewritten = prepare_powershell_command_for_windows_sandbox(
            &command,
            Some(&ShellType::PowerShell),
            /*sandbox_requested*/ false,
            windows_sandbox,
            /*environment_is_remote*/ false,
        );

        assert_eq!(rewritten, command);
    }

    #[test_case::test_case(WindowsSandboxSelection::Elevated; "elevated")]
    #[test_case::test_case(WindowsSandboxSelection::Mxc; "mxc")]
    fn leaves_non_powershell_alone(windows_sandbox: WindowsSandboxSelection) {
        let command = vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "echo ok".to_string(),
        ];

        let rewritten = prepare_powershell_command_for_windows_sandbox(
            &command,
            Some(&ShellType::Bash),
            /*sandbox_requested*/ true,
            windows_sandbox,
            /*environment_is_remote*/ false,
        );

        assert_eq!(rewritten, command);
    }
    #[test_case::test_case(WindowsSandboxSelection::Elevated, &["-NoProfile", "-Command", "Write-Output ok"]; "elevated")]
    #[test_case::test_case(WindowsSandboxSelection::Mxc, &["-Command", "Write-Output ok"]; "mxc")]
    fn local_sandboxed_powershell_uses_discovered_fallback(
        windows_sandbox: WindowsSandboxSelection,
        expected_args: &[&str],
    ) {
        let command = vec![
            r"C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.4.0_x64__8wekyb3d8bbwe\pwsh.exe"
                .to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];
        let fallback_path = std::path::PathBuf::from(r"C:\Program Files\PowerShell\7\pwsh.exe");

        let rewritten = prepare_powershell_command_for_windows_sandbox_with_fallback(
            &command,
            Some(&ShellType::PowerShell),
            /*sandbox_requested*/ true,
            windows_sandbox,
            /*environment_is_remote*/ false,
            |_| {
                Some(codex_shell_command::shell_detect::DetectedShell {
                    shell_type: ShellType::PowerShell,
                    shell_path: fallback_path.clone(),
                })
            },
        );

        assert_eq!(
            rewritten,
            vec![fallback_path.to_string_lossy().to_string()]
                .into_iter()
                .chain(expected_args.iter().map(std::string::ToString::to_string))
                .collect::<Vec<_>>()
        );
    }

    #[test_case::test_case(WindowsSandboxSelection::Elevated, &["-NoProfile", "-Command", "Write-Output ok"]; "elevated")]
    #[test_case::test_case(WindowsSandboxSelection::Mxc, &["-Command", "Write-Output ok"]; "mxc")]
    fn remote_sandboxed_powershell_keeps_remote_store_path(
        windows_sandbox: WindowsSandboxSelection,
        expected_args: &[&str],
    ) {
        let command = vec![
            r"C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.4.0_x64__8wekyb3d8bbwe\pwsh.exe"
                .to_string(),
            "-Command".to_string(),
            "Write-Output ok".to_string(),
        ];

        let mut discovery_called = false;
        let rewritten = prepare_powershell_command_for_windows_sandbox_with_fallback(
            &command,
            Some(&ShellType::PowerShell),
            /*sandbox_requested*/ true,
            windows_sandbox,
            /*environment_is_remote*/ true,
            |_| {
                discovery_called = true;
                Some(codex_shell_command::shell_detect::DetectedShell {
                    shell_type: ShellType::PowerShell,
                    shell_path: std::path::PathBuf::from(r"C:\Program Files\PowerShell\7\pwsh.exe"),
                })
            },
        );

        assert!(!discovery_called);
        assert_eq!(
            rewritten,
            vec![command[0].clone()]
                .into_iter()
                .chain(expected_args.iter().map(std::string::ToString::to_string))
                .collect::<Vec<_>>()
        );
    }
}

#[cfg(all(test, unix))]
#[path = "mod_tests.rs"]
mod tests;
