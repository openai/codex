use crate::sandboxing::SandboxPermissions;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::shell::get_shell_by_model_provided_path;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::PostToolUsePayload;
use codex_exec_server::Environment;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_tools::UnifiedExecShellMode;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(test)]
use crate::tools::handlers::parse_arguments;

mod exec_command;
mod write_stdin;

pub use exec_command::ExecCommandHandler;
pub(crate) use exec_command::ExecCommandHandlerOptions;
pub use write_stdin::WriteStdinHandler;

#[derive(Debug, Deserialize)]
pub(crate) struct ExecCommandArgs {
    cmd: String,
    #[serde(default)]
    pub(crate) workdir: Option<String>,
    #[serde(default)]
    shell: Option<String>,
    #[serde(default)]
    login: Option<bool>,
    #[serde(default = "default_tty")]
    tty: bool,
    #[serde(default = "default_exec_yield_time_ms")]
    yield_time_ms: u64,
    #[serde(default)]
    max_output_tokens: Option<usize>,
    #[serde(default)]
    sandbox_permissions: SandboxPermissions,
    #[serde(default)]
    additional_permissions: Option<AdditionalPermissionProfile>,
    #[serde(default)]
    justification: Option<String>,
    #[serde(default)]
    prefix_rule: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ExecCommandEnvironmentArgs {
    #[serde(default)]
    environment_id: Option<String>,
    // Keep this raw until after environment selection; relative paths must be
    // resolved against the selected environment cwd, not the process cwd.
    #[serde(default)]
    workdir: Option<String>,
}

fn default_exec_yield_time_ms() -> u64 {
    10_000
}

fn default_write_stdin_yield_time_ms() -> u64 {
    250
}

fn default_tty() -> bool {
    false
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ResolvedCommand {
    pub(crate) command: Vec<String>,
    pub(crate) shell_type: ShellType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandShellResolution {
    LocalHost,
    RemoteTarget,
}

fn post_unified_exec_tool_use_payload(
    invocation: &ToolInvocation,
    result: &dyn ToolOutput,
) -> Option<PostToolUsePayload> {
    let ToolPayload::Function { .. } = &invocation.payload else {
        return None;
    };

    let tool_input = result.post_tool_use_input(&invocation.payload)?;
    let tool_use_id = result.post_tool_use_id(&invocation.call_id);
    let tool_response = result.post_tool_use_response(&tool_use_id, &invocation.payload)?;
    Some(PostToolUsePayload {
        tool_name: HookToolName::bash(),
        tool_use_id,
        tool_input,
        tool_response,
    })
}

pub(crate) fn get_command(
    args: &ExecCommandArgs,
    default_shell: Arc<Shell>,
    shell_mode: &UnifiedExecShellMode,
    allow_login_shell: bool,
    shell_resolution: CommandShellResolution,
) -> Result<ResolvedCommand, String> {
    let use_login_shell = match args.login {
        Some(true) if !allow_login_shell => {
            return Err(
                "login shell is disabled by config; omit `login` or set it to false.".to_string(),
            );
        }
        Some(use_login_shell) => use_login_shell,
        None => allow_login_shell,
    };

    match shell_mode {
        UnifiedExecShellMode::Direct => {
            let model_shell = args
                .shell
                .as_deref()
                .map(|shell| resolve_model_shell(shell, shell_resolution))
                .transpose()?;
            let shell = model_shell.as_ref().unwrap_or(default_shell.as_ref());
            Ok(ResolvedCommand {
                command: shell.derive_exec_args(&args.cmd, use_login_shell),
                shell_type: shell.shell_type,
            })
        }
        UnifiedExecShellMode::ZshFork(zsh_fork_config) => {
            if args.shell.is_some() {
                return Err(
                    "`shell` is not supported for local zsh-fork exec; omit `shell` to use zsh-fork, or target a remote environment where `shell` is supported.".to_string(),
                );
            }

            Ok(ResolvedCommand {
                command: vec![
                    zsh_fork_config.shell_zsh_path.to_string_lossy().to_string(),
                    if use_login_shell { "-lc" } else { "-c" }.to_string(),
                    args.cmd.clone(),
                ],
                shell_type: ShellType::Zsh,
            })
        }
    }
}

fn resolve_model_shell(
    shell_path: &str,
    shell_resolution: CommandShellResolution,
) -> Result<Shell, String> {
    match shell_resolution {
        CommandShellResolution::LocalHost => {
            let mut shell = get_shell_by_model_provided_path(&PathBuf::from(shell_path));
            shell.shell_snapshot = crate::shell::empty_shell_snapshot_receiver();
            Ok(shell)
        }
        CommandShellResolution::RemoteTarget => {
            let file_name = shell_path
                .rsplit(['/', '\\'])
                .next()
                .filter(|file_name| !file_name.is_empty())
                .ok_or_else(|| {
                    format!("remote shell path `{shell_path}` has no executable name")
                })?;
            let file_name = file_name.to_ascii_lowercase();
            let shell_name = file_name.strip_suffix(".exe").unwrap_or(&file_name);
            let shell_type = match shell_name {
                "zsh" => ShellType::Zsh,
                "bash" => ShellType::Bash,
                "sh" => ShellType::Sh,
                "pwsh" | "powershell" => ShellType::PowerShell,
                "cmd" => ShellType::Cmd,
                _ => {
                    return Err(format!(
                        "unsupported remote shell `{shell_path}`; expected zsh, bash, sh, pwsh, powershell, or cmd"
                    ));
                }
            };
            Ok(Shell {
                shell_type,
                shell_path: PathBuf::from(shell_path),
                shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
            })
        }
    }
}

pub(crate) fn resolve_workdir_uri(
    base: &PathUri,
    workdir: Option<&str>,
    convention: PathConvention,
) -> Result<PathUri, String> {
    match workdir.filter(|workdir| !workdir.is_empty()) {
        None => Ok(base.clone()),
        Some(workdir) => base
            .resolve_native(workdir, convention)
            .map_err(|error| format!("invalid working directory `{workdir}`: {error}")),
    }
}

pub(crate) fn shell_mode_for_environment(
    turn_shell_mode: &UnifiedExecShellMode,
    environment: &Environment,
) -> UnifiedExecShellMode {
    if environment.is_remote() {
        UnifiedExecShellMode::Direct
    } else {
        turn_shell_mode.clone()
    }
}

#[cfg(test)]
#[path = "unified_exec_tests.rs"]
mod tests;
