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
            return run_remote_hook_command(handler, environment, input_json, cwd).await;
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

async fn run_remote_hook_command(
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
    run_started_remote_hook_command(process, handler, input_json, started_at, started).await
}

async fn run_started_remote_hook_command(
    process: std::sync::Arc<dyn ExecProcess>,
    handler: &ConfiguredHandler,
    input_json: &str,
    started_at: i64,
    started: Instant,
) -> CommandRunResult {
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
    use std::collections::VecDeque;
    use std::future::pending;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    use async_trait::async_trait;
    use codex_exec_server::EnvironmentManager;
    use codex_exec_server::ExecProcessEventReceiver;
    use codex_exec_server::ExecServerError;
    use codex_exec_server::ProcessOutputChunk;
    use codex_exec_server::ReadResponse;
    use codex_exec_server::WriteResponse;
    use codex_exec_server::WriteStatus;
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookSource;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use tokio::sync::Mutex;
    use tokio::sync::watch;

    use super::CommandShell;
    use super::ConfiguredHandler;
    use super::collect_output;
    use super::run_command;
    use super::run_started_remote_hook_command;
    use super::write_remote_stdin;

    struct MockExecProcess {
        process_id: codex_exec_server::ProcessId,
        write_response: WriteResponse,
        read_responses: Mutex<VecDeque<Result<ReadResponse, ExecServerError>>>,
        block_reads: bool,
        writes: Mutex<Vec<(Option<Vec<u8>>, bool)>>,
        terminate_calls: AtomicUsize,
        wake_tx: watch::Sender<u64>,
    }

    impl MockExecProcess {
        fn new(
            write_status: WriteStatus,
            read_responses: Vec<Result<ReadResponse, ExecServerError>>,
        ) -> Arc<Self> {
            let (wake_tx, _wake_rx) = watch::channel(0);
            Arc::new(Self {
                process_id: "hook-process".to_string().into(),
                write_response: WriteResponse {
                    status: write_status,
                },
                read_responses: Mutex::new(VecDeque::from(read_responses)),
                block_reads: false,
                writes: Mutex::new(Vec::new()),
                terminate_calls: AtomicUsize::new(0),
                wake_tx,
            })
        }

        fn blocking() -> Arc<Self> {
            let (wake_tx, _wake_rx) = watch::channel(0);
            Arc::new(Self {
                process_id: "hook-process".to_string().into(),
                write_response: WriteResponse {
                    status: WriteStatus::Accepted,
                },
                read_responses: Mutex::new(VecDeque::new()),
                block_reads: true,
                writes: Mutex::new(Vec::new()),
                terminate_calls: AtomicUsize::new(0),
                wake_tx,
            })
        }
    }

    #[async_trait]
    impl codex_exec_server::ExecProcess for MockExecProcess {
        fn process_id(&self) -> &codex_exec_server::ProcessId {
            &self.process_id
        }

        fn subscribe_wake(&self) -> watch::Receiver<u64> {
            self.wake_tx.subscribe()
        }

        fn subscribe_events(&self) -> ExecProcessEventReceiver {
            ExecProcessEventReceiver::empty()
        }

        async fn read(
            &self,
            _after_seq: Option<u64>,
            _max_bytes: Option<usize>,
            _wait_ms: Option<u64>,
        ) -> Result<ReadResponse, ExecServerError> {
            if self.block_reads {
                return pending().await;
            }
            self.read_responses
                .lock()
                .await
                .pop_front()
                .unwrap_or_else(|| Ok(closed_read_response(Vec::new(), Some(0))))
        }

        async fn write(
            &self,
            chunk: Option<Vec<u8>>,
            close_stdin: bool,
        ) -> Result<WriteResponse, ExecServerError> {
            self.writes.lock().await.push((chunk, close_stdin));
            Ok(self.write_response.clone())
        }

        async fn terminate(&self) -> Result<(), ExecServerError> {
            self.terminate_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

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

    fn read_response(
        chunks: Vec<ProcessOutputChunk>,
        next_seq: u64,
        exited: bool,
        exit_code: Option<i32>,
        closed: bool,
    ) -> ReadResponse {
        ReadResponse {
            chunks,
            next_seq,
            exited,
            exit_code,
            closed,
            failure: None,
        }
    }

    fn closed_read_response(
        chunks: Vec<ProcessOutputChunk>,
        exit_code: Option<i32>,
    ) -> ReadResponse {
        read_response(chunks, 4, true, exit_code, true)
    }

    fn output_chunk(
        seq: u64,
        stream: codex_exec_server::ExecOutputStream,
        chunk: &[u8],
    ) -> ProcessOutputChunk {
        ProcessOutputChunk {
            seq,
            stream,
            chunk: chunk.to_vec().into(),
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
    async fn explicit_local_environment_id_runs_locally() {
        let result = run_command(
            &shell(std::sync::Arc::new(EnvironmentManager::default_for_tests())),
            &handler("printf explicit-local-hook", Some("local")),
            "{}",
            test_path_buf("/tmp").as_path(),
        )
        .await;

        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout, "explicit-local-hook");
        assert_eq!(result.error, None);
    }

    #[tokio::test]
    async fn local_hook_receives_stdin_and_captures_stderr_and_exit_code() {
        let result = run_command(
            &shell(std::sync::Arc::new(EnvironmentManager::default_for_tests())),
            &handler("cat; printf stderr >&2; exit 7", None),
            "{\"hook\":true}",
            test_path_buf("/tmp").as_path(),
        )
        .await;

        assert_eq!(result.exit_code, Some(7));
        assert_eq!(result.stdout, "{\"hook\":true}");
        assert_eq!(result.stderr, "stderr");
        assert_eq!(result.error, None);
    }

    #[tokio::test]
    async fn local_hook_timeout_returns_error() {
        let mut handler = handler("sleep 5", None);
        handler.timeout_sec = 1;
        let result = run_command(
            &shell(std::sync::Arc::new(EnvironmentManager::default_for_tests())),
            &handler,
            "{}",
            test_path_buf("/tmp").as_path(),
        )
        .await;

        assert_eq!(result.exit_code, None);
        assert_eq!(result.stdout, "");
        assert_eq!(result.stderr, "");
        assert_eq!(result.error.as_deref(), Some("hook timed out after 1s"));
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

    #[tokio::test]
    async fn remote_hook_stdin_writes_payload_and_closes_stdin() {
        let process = MockExecProcess::new(WriteStatus::Accepted, Vec::new());

        write_remote_stdin(
            &(process.clone() as Arc<dyn codex_exec_server::ExecProcess>),
            "{\"hook\":true}",
        )
        .await
        .expect("remote hook stdin should be accepted");

        assert_eq!(
            process.writes.lock().await.as_slice(),
            &[(Some(b"{\"hook\":true}".to_vec()), true)]
        );
    }

    #[tokio::test]
    async fn remote_hook_stdin_rejects_non_accepted_write_status() {
        let process = MockExecProcess::new(WriteStatus::StdinClosed, Vec::new());

        let err = write_remote_stdin(
            &(process.clone() as Arc<dyn codex_exec_server::ExecProcess>),
            "{}",
        )
        .await
        .expect_err("closed stdin should fail");

        assert_eq!(err, "failed to write hook stdin: StdinClosed");
    }

    #[tokio::test]
    async fn remote_hook_collects_stdout_stderr_and_pty_output() {
        let process = MockExecProcess::new(
            WriteStatus::Accepted,
            vec![Ok(closed_read_response(
                vec![
                    output_chunk(1, codex_exec_server::ExecOutputStream::Stdout, b"stdout"),
                    output_chunk(2, codex_exec_server::ExecOutputStream::Stderr, b"stderr"),
                    output_chunk(3, codex_exec_server::ExecOutputStream::Pty, b"pty"),
                ],
                Some(0),
            ))],
        );

        let actual = collect_output(process as Arc<dyn codex_exec_server::ExecProcess>)
            .await
            .expect("remote output should collect");

        assert_eq!(
            actual,
            ("stdoutpty".to_string(), "stderr".to_string(), Some(0))
        );
    }

    #[tokio::test]
    async fn remote_hook_collect_output_surfaces_process_failure() {
        let mut response = closed_read_response(Vec::new(), None);
        response.failure = Some("transport disconnected".to_string());
        let process = MockExecProcess::new(WriteStatus::Accepted, vec![Ok(response)]);

        let err = collect_output(process as Arc<dyn codex_exec_server::ExecProcess>)
            .await
            .expect_err("remote output failure should surface");

        assert_eq!(err, "transport disconnected");
    }

    #[tokio::test]
    async fn remote_hook_write_failure_terminates_process() {
        let process = MockExecProcess::new(WriteStatus::StdinClosed, Vec::new());
        let result = run_started_remote_hook_command(
            process.clone() as Arc<dyn codex_exec_server::ExecProcess>,
            &handler("printf ignored", Some("remote-hook")),
            "{}",
            chrono::Utc::now().timestamp(),
            Instant::now(),
        )
        .await;

        assert_eq!(result.exit_code, None);
        assert_eq!(
            result.error.as_deref(),
            Some("failed to write hook stdin: StdinClosed")
        );
        assert_eq!(process.terminate_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn remote_hook_read_failure_terminates_process() {
        let mut response = closed_read_response(Vec::new(), None);
        response.failure = Some("transport disconnected".to_string());
        let process = MockExecProcess::new(WriteStatus::Accepted, vec![Ok(response)]);
        let result = run_started_remote_hook_command(
            process.clone() as Arc<dyn codex_exec_server::ExecProcess>,
            &handler("printf ignored", Some("remote-hook")),
            "{}",
            chrono::Utc::now().timestamp(),
            Instant::now(),
        )
        .await;

        assert_eq!(result.exit_code, None);
        assert_eq!(result.error.as_deref(), Some("transport disconnected"));
        assert_eq!(process.terminate_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn remote_hook_timeout_terminates_process() {
        let process = MockExecProcess::blocking();
        let mut handler = handler("printf ignored", Some("remote-hook"));
        handler.timeout_sec = 0;
        let result = run_started_remote_hook_command(
            process.clone() as Arc<dyn codex_exec_server::ExecProcess>,
            &handler,
            "{}",
            chrono::Utc::now().timestamp(),
            Instant::now(),
        )
        .await;

        assert_eq!(result.exit_code, None);
        assert_eq!(result.error.as_deref(), Some("hook timed out after 0s"));
        assert_eq!(process.terminate_calls.load(Ordering::SeqCst), 1);
    }
}
