use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use super::CommandShell;
use super::ConfiguredHandler;

const SHUTDOWN_BOUNDED_TIMEOUT_SEC: u64 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandExecutionPolicy {
    Standard,
    ShutdownBounded { timeout_cap_sec: u64 },
}

impl CommandExecutionPolicy {
    pub const fn shutdown_bounded() -> Self {
        Self::ShutdownBounded {
            timeout_cap_sec: SHUTDOWN_BOUNDED_TIMEOUT_SEC,
        }
    }

    fn effective_timeout_sec(self, configured_timeout_sec: u64) -> u64 {
        match self {
            Self::Standard => configured_timeout_sec,
            Self::ShutdownBounded { timeout_cap_sec } => {
                configured_timeout_sec.min(timeout_cap_sec)
            }
        }
        .max(1)
    }
}

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
    execution_policy: CommandExecutionPolicy,
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

    let effective_timeout_sec = execution_policy.effective_timeout_sec(handler.timeout_sec);
    let timeout_duration = Duration::from_secs(effective_timeout_sec);
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
            error: Some(format!("hook timed out after {effective_timeout_sec}s")),
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
    use std::time::Duration;
    use std::time::Instant;

    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookSource;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use tempfile::tempdir;

    use super::CommandExecutionPolicy;
    use super::CommandShell;
    use super::ConfiguredHandler;
    use super::run_command;

    #[test]
    fn shutdown_bounded_policy_caps_timeout() {
        assert_eq!(
            CommandExecutionPolicy::Standard.effective_timeout_sec(600),
            600
        );
        assert_eq!(
            CommandExecutionPolicy::shutdown_bounded().effective_timeout_sec(600),
            8
        );
        assert_eq!(
            CommandExecutionPolicy::ShutdownBounded { timeout_cap_sec: 3 }.effective_timeout_sec(2),
            2
        );
    }

    #[tokio::test]
    async fn shutdown_bounded_timeout_uses_capped_timeout() {
        let temp_dir = tempdir().expect("create temp dir");
        let started = Instant::now();
        let result = run_command(
            &test_shell(),
            &handler(sleep_command(), 10),
            "{}",
            temp_dir.path(),
            CommandExecutionPolicy::ShutdownBounded { timeout_cap_sec: 1 },
        )
        .await;

        assert!(started.elapsed() < Duration::from_millis(1500));
        assert_eq!(result.exit_code, None);
        assert_eq!(result.error.as_deref(), Some("hook timed out after 1s"));
    }

    fn handler(command: String, timeout_sec: u64) -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::Interrupt,
            matcher: None,
            command,
            timeout_sec,
            status_message: None,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: HookSource::User,
            display_order: 0,
            env: HashMap::new(),
        }
    }

    fn test_shell() -> CommandShell {
        #[cfg(windows)]
        {
            CommandShell {
                program: "powershell".to_string(),
                args: vec!["-NoProfile".to_string(), "-Command".to_string()],
            }
        }

        #[cfg(not(windows))]
        {
            CommandShell {
                program: "/bin/sh".to_string(),
                args: vec!["-lc".to_string()],
            }
        }
    }

    fn sleep_command() -> String {
        #[cfg(windows)]
        {
            "Start-Sleep -Seconds 2".to_string()
        }

        #[cfg(not(windows))]
        {
            "sleep 2".to_string()
        }
    }
}
