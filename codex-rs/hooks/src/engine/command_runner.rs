use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

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
    for variable in &shell.excluded_environment_variables {
        command.env_remove(variable);
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

    use super::*;

    #[test]
    fn excluded_environment_variable_overrides_handler_env() {
        let variable = "CODEX_WIF_HOOK_ASSERTION";
        let shell = CommandShell {
            program: String::new(),
            args: Vec::new(),
            excluded_environment_variables: vec![variable.to_string()],
        };
        let handler = ConfiguredHandler {
            event_name: HookEventName::PreToolUse,
            matcher: None,
            command: "true".to_string(),
            timeout_sec: 1,
            status_message: None,
            source_path: AbsolutePathBuf::current_dir().expect("current dir"),
            source: HookSource::Unknown,
            display_order: 0,
            env: HashMap::from([(variable.to_string(), "secret.assertion".to_string())]),
        };

        let command = build_command(&shell, &handler);
        assert!(
            command
                .as_std()
                .get_envs()
                .any(|(name, value)| { name == OsStr::new(variable) && value.is_none() })
        );
    }
}
