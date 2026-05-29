use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ProcessId;
use codex_exec_server::WriteStatus;
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
    if let Some(environment_id) = handler.environment_id.as_deref() {
        let Some(environment) = shell.environment_manager.get_environment(environment_id) else {
            return command_error(
                started_at,
                started,
                format!("unknown hook environment id: {environment_id}"),
            );
        };
        if environment.is_remote() {
            return run_remote_command(handler, environment, input_json, cwd).await;
        }
    }

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
            return command_error(started_at, started, err.to_string());
        }
    };

    if let Some(mut stdin) = child.stdin.take()
        && let Err(err) = stdin.write_all(input_json.as_bytes()).await
    {
        let _ = child.kill().await;
        return command_error(
            started_at,
            started,
            format!("failed to write hook stdin: {err}"),
        );
    }

    let timeout_duration = Duration::from_secs(handler.timeout_sec);
    match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: elapsed_ms(started),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            error: None,
        },
        Ok(Err(err)) => command_error(started_at, started, err.to_string()),
        Err(_) => CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: elapsed_ms(started),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(format!("hook timed out after {}s", handler.timeout_sec)),
        },
    }
}

async fn run_remote_command(
    handler: &ConfiguredHandler,
    environment: std::sync::Arc<codex_exec_server::Environment>,
    input_json: &str,
    cwd: &Path,
) -> CommandRunResult {
    let started_at = chrono::Utc::now().timestamp();
    let started = Instant::now();
    let process = match environment
        .get_exec_backend()
        .start(ExecParams {
            process_id: ProcessId::new(format!("hook-{}", uuid::Uuid::new_v4())),
            argv: default_shell_argv(handler),
            cwd: cwd.to_path_buf(),
            env_policy: None,
            env: handler.env.clone(),
            tty: false,
            pipe_stdin: true,
            arg0: None,
        })
        .await
    {
        Ok(started) => started.process,
        Err(err) => return command_error(started_at, started, err.to_string()),
    };
    if let Err(error) = write_remote_stdin(&process, input_json).await {
        let _ = process.terminate().await;
        return command_error(started_at, started, error);
    }

    match timeout(
        Duration::from_secs(handler.timeout_sec),
        collect_output(std::sync::Arc::clone(&process)),
    )
    .await
    {
        Ok(Ok((stdout, stderr, exit_code))) => CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: elapsed_ms(started),
            exit_code,
            stdout,
            stderr,
            error: None,
        },
        Ok(Err(error)) => {
            let _ = process.terminate().await;
            command_error(started_at, started, error)
        }
        Err(_) => {
            let _ = process.terminate().await;
            CommandRunResult {
                started_at,
                completed_at: chrono::Utc::now().timestamp(),
                duration_ms: elapsed_ms(started),
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("hook timed out after {}s", handler.timeout_sec)),
            }
        }
    }
}

async fn write_remote_stdin(
    process: &std::sync::Arc<dyn ExecProcess>,
    input_json: &str,
) -> Result<(), String> {
    let response = process
        .write(Some(input_json.as_bytes().to_vec()), true)
        .await
        .map_err(|err| format!("failed to write hook stdin: {err}"))?;
    if response.status == WriteStatus::Accepted {
        Ok(())
    } else {
        Err(format!("failed to write hook stdin: {:?}", response.status))
    }
}

async fn collect_output(
    process: std::sync::Arc<dyn ExecProcess>,
) -> Result<(String, String, Option<i32>), String> {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut after_seq = None;
    loop {
        let response = process
            .read(after_seq, None, Some(50))
            .await
            .map_err(|err| err.to_string())?;
        for chunk in response.chunks {
            match chunk.stream {
                ExecOutputStream::Stdout | ExecOutputStream::Pty => {
                    stdout.extend_from_slice(&chunk.chunk.0);
                }
                ExecOutputStream::Stderr => stderr.extend_from_slice(&chunk.chunk.0),
            }
        }
        if let Some(failure) = response.failure {
            return Err(failure);
        }
        if response.closed {
            return Ok((
                String::from_utf8_lossy(&stdout).to_string(),
                String::from_utf8_lossy(&stderr).to_string(),
                response.exit_code,
            ));
        }
        after_seq = response.next_seq.checked_sub(1);
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

#[cfg(windows)]
fn default_shell_argv(handler: &ConfiguredHandler) -> Vec<String> {
    vec![
        "cmd.exe".to_string(),
        "/C".to_string(),
        handler.command.clone(),
    ]
}

#[cfg(not(windows))]
fn default_shell_argv(handler: &ConfiguredHandler) -> Vec<String> {
    vec![
        "/bin/sh".to_string(),
        "-lc".to_string(),
        handler.command.clone(),
    ]
}

fn command_error(started_at: i64, started: Instant, error: String) -> CommandRunResult {
    CommandRunResult {
        started_at,
        completed_at: chrono::Utc::now().timestamp(),
        duration_ms: elapsed_ms(started),
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        error: Some(error),
    }
}

fn elapsed_ms(started: Instant) -> i64 {
    started.elapsed().as_millis().try_into().unwrap_or(i64::MAX)
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
    use codex_exec_server::EnvironmentManager;
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookSource;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    use super::CommandShell;
    use super::ConfiguredHandler;
    use super::run_command;

    fn shell(environment_manager: std::sync::Arc<EnvironmentManager>) -> CommandShell {
        CommandShell {
            program: String::new(),
            args: Vec::new(),
            environment_manager,
        }
    }

    fn handler(command: &str, environment_id: Option<&str>) -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::PreToolUse,
            matcher: None,
            command: command.to_string(),
            environment_id: environment_id.map(str::to_string),
            timeout_sec: 5,
            status_message: None,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: HookSource::User,
            display_order: 0,
            env: Default::default(),
        }
    }

    #[tokio::test]
    async fn omitted_environment_id_runs_locally() {
        let result = run_command(
            &shell(std::sync::Arc::new(EnvironmentManager::default_for_tests())),
            &handler("printf local-hook", None),
            "{}",
            test_path_buf("/tmp").as_path(),
        )
        .await;

        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout, "local-hook");
        assert_eq!(result.error, None);
    }

    #[tokio::test]
    async fn unknown_environment_id_does_not_fall_back_to_local() {
        let result = run_command(
            &shell(std::sync::Arc::new(EnvironmentManager::default_for_tests())),
            &handler("printf local-hook", Some("missing")),
            "{}",
            test_path_buf("/tmp").as_path(),
        )
        .await;

        assert_eq!(result.exit_code, None);
        assert_eq!(result.stdout, "");
        assert_eq!(
            result.error.as_deref(),
            Some("unknown hook environment id: missing")
        );
    }

    #[tokio::test]
    async fn explicit_remote_environment_uses_exec_backend() {
        let environment_manager = std::sync::Arc::new(EnvironmentManager::default_for_tests());
        environment_manager
            .upsert_environment("remote-hook".to_string(), "ws://127.0.0.1:1".to_string())
            .expect("remote hook environment");
        let result = run_command(
            &shell(environment_manager),
            &handler("printf local-hook", Some("remote-hook")),
            "{}",
            test_path_buf("/tmp").as_path(),
        )
        .await;

        assert_eq!(result.exit_code, None);
        assert_eq!(result.stdout, "");
        assert!(
            result
                .error
                .as_deref()
                .is_some_and(|error| error.contains("failed"))
        );
    }
}
