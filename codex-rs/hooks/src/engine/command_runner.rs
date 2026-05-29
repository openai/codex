use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use codex_protocol::protocol::HookEventName;
use codex_protocol::shell_environment::CLAUDE_ENV_FILE_ENV_VAR;
use codex_protocol::shell_environment::CODEX_ENV_FILE_ENV_VAR;

use super::CommandShell;
use super::ConfiguredHandler;

#[derive(Debug)]
pub(crate) struct CommandRunResult {
    pub started_at: i64,
    pub completed_at: i64,
    pub duration_ms: i64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

pub(crate) async fn run_command(
    shell: &CommandShell,
    handler: &ConfiguredHandler,
    input_json: &str,
    cwd: &Path,
) -> CommandRunResult {
    let started_at = chrono::Utc::now().timestamp();
    let started = Instant::now();

    let mut command = build_command(shell, handler);
    command
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return CommandRunResult {
                started_at,
                completed_at: chrono::Utc::now().timestamp(),
                duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(err.to_string()),
            };
        }
    };

    if let Some(mut stdin) = child.stdin.take()
        && let Err(err) = stdin.write_all(input_json.as_bytes()).await
    {
        let _ = child.kill().await;
        return CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(format!("failed to write hook stdin: {err}")),
        };
    }

    let timeout_duration = Duration::from_secs(handler.timeout_sec);
    match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            error: None,
        },
        Ok(Err(err)) => CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(err.to_string()),
        },
        Err(_) => CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(format!("hook timed out after {}s", handler.timeout_sec)),
        },
    }
}

fn build_command(shell: &CommandShell, handler: &ConfiguredHandler) -> Command {
    let mut command = if shell.program.is_empty() {
        default_shell_command()
    } else {
        Command::new(&shell.program)
    };
    if shell.program.is_empty() {
        command.arg(&handler.command);
    } else {
        command.args(&shell.args);
        command.arg(&handler.command);
    }
    command.envs(&handler.env);
    // Only SessionStart hooks receive session env-file paths.
    command.env_remove(CODEX_ENV_FILE_ENV_VAR);
    command.env_remove(CLAUDE_ENV_FILE_ENV_VAR);
    if handler.event_name == HookEventName::SessionStart {
        command.envs(&shell.env);
    }
    command
}

fn default_shell_command() -> Command {
    #[cfg(windows)]
    {
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
        let mut command = Command::new(comspec);
        command.arg("/C");
        command
    }

    #[cfg(not(windows))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut command = Command::new(shell);
        command.arg("-lc");
        command
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ffi::OsStr;

    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookSource;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tokio::process::Command;

    use super::CLAUDE_ENV_FILE_ENV_VAR;
    use super::CODEX_ENV_FILE_ENV_VAR;
    use super::CommandShell;
    use super::ConfiguredHandler;
    use super::build_command;

    fn shell() -> CommandShell {
        CommandShell {
            program: "hook-shell".to_string(),
            args: Vec::new(),
            env: HashMap::from([
                (
                    CODEX_ENV_FILE_ENV_VAR.to_string(),
                    "session-owned-env-file".to_string(),
                ),
                (
                    CLAUDE_ENV_FILE_ENV_VAR.to_string(),
                    "session-owned-env-file".to_string(),
                ),
            ]),
        }
    }

    fn handler(event_name: HookEventName) -> ConfiguredHandler {
        ConfiguredHandler {
            event_name,
            matcher: None,
            command: "echo hook".to_string(),
            timeout_sec: 10,
            status_message: None,
            source_path: AbsolutePathBuf::current_dir().expect("current dir"),
            source: HookSource::User,
            display_order: 0,
            env: HashMap::new(),
        }
    }

    fn command_env<'a>(command: &'a Command, name: &str) -> Option<Option<&'a OsStr>> {
        command
            .as_std()
            .get_envs()
            .find(|(key, _)| key == &OsStr::new(name))
            .map(|(_, value)| value)
    }

    #[test]
    fn non_session_start_hook_masks_inherited_env_file_paths() {
        let command = build_command(&shell(), &handler(HookEventName::PreToolUse));

        assert_eq!(command_env(&command, CODEX_ENV_FILE_ENV_VAR), Some(None));
        assert_eq!(command_env(&command, CLAUDE_ENV_FILE_ENV_VAR), Some(None));
    }

    #[test]
    fn session_start_hook_receives_session_owned_env_file_paths() {
        let command = build_command(&shell(), &handler(HookEventName::SessionStart));

        assert_eq!(
            command_env(&command, CODEX_ENV_FILE_ENV_VAR),
            Some(Some(OsStr::new("session-owned-env-file")))
        );
        assert_eq!(
            command_env(&command, CLAUDE_ENV_FILE_ENV_VAR),
            Some(Some(OsStr::new("session-owned-env-file")))
        );
    }
}
