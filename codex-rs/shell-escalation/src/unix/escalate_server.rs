use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context as _;
use codex_utils_absolute_path::AbsolutePathBuf;
use socket2::Socket;
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::unix::escalate_protocol::ESCALATE_SOCKET_ENV_VAR;
use crate::unix::escalate_protocol::EXEC_WRAPPER_ENV_VAR;
use crate::unix::escalate_protocol::EscalateAction;
use crate::unix::escalate_protocol::EscalateRequest;
use crate::unix::escalate_protocol::EscalateResponse;
use crate::unix::escalate_protocol::EscalationDecision;
use crate::unix::escalate_protocol::EscalationExecution;
use crate::unix::escalate_protocol::SuperExecMessage;
use crate::unix::escalate_protocol::SuperExecResult;
use crate::unix::escalation_policy::EscalationPolicy;
use crate::unix::socket::AsyncDatagramSocket;
use crate::unix::socket::AsyncSocket;

/// Adapter for running the shell command after the escalation server has been set up.
///
/// This lets `shell-escalation` own the Unix escalation protocol while the caller
/// keeps control over process spawning, output capture, and sandbox integration.
/// Implementations can capture any sandbox state they need.
#[async_trait::async_trait]
pub trait ShellCommandExecutor: Send + Sync {
    /// Runs the requested shell command and returns the captured result.
    ///
    /// `env_overlay` contains only the wrapper/socket variables exported by
    /// `EscalationSession::env()`, not a complete child environment.
    /// Implementations should merge it into whatever base environment they use
    /// for the shell process. `after_spawn` should be invoked immediately after
    /// the shell process has been spawned so the parent copy of the inherited
    /// escalation socket can be closed.
    async fn run(
        &self,
        command: Vec<String>,
        cwd: PathBuf,
        env_overlay: HashMap<String, String>,
        cancel_rx: CancellationToken,
        after_spawn: Option<Box<dyn FnOnce() + Send>>,
    ) -> anyhow::Result<ExecResult>;

    /// Prepares an escalated subcommand for execution on the server side.
    async fn prepare_escalated_exec(
        &self,
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
        env: HashMap<String, String>,
        execution: EscalationExecution,
    ) -> anyhow::Result<PreparedExec>;
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ExecParams {
    /// The command string to pass to the shell via `-c` or `-lc`.
    pub command: String,
    /// The working directory to execute the command in. Must be an absolute path.
    pub workdir: String,
    /// The timeout for the command in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Launch the shell with -lc instead of -c: defaults to true.
    pub login: Option<bool>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    /// Aggregated stdout+stderr output for compatibility with existing callers.
    pub output: String,
    pub duration: Duration,
    pub timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedExec {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub arg0: Option<String>,
}

#[derive(Debug)]
pub struct EscalationSession {
    env: HashMap<String, String>,
    task: JoinHandle<anyhow::Result<()>>,
    client_socket: Arc<Mutex<Option<Socket>>>,
    cancellation_token: CancellationToken,
}

impl EscalationSession {
    /// Returns just the environment overlay needed by the execve wrapper.
    ///
    /// Callers should merge this into their own child-process environment
    /// rather than treating it as the full environment for the shell.
    pub fn env(&self) -> &HashMap<String, String> {
        &self.env
    }

    pub fn close_client_socket(&self) {
        if let Ok(mut client_socket) = self.client_socket.lock() {
            client_socket.take();
        }
    }
}

impl Drop for EscalationSession {
    fn drop(&mut self) {
        self.close_client_socket();
        self.cancellation_token.cancel();
        self.task.abort();
    }
}

pub struct EscalateServer {
    shell_path: PathBuf,
    execve_wrapper: PathBuf,
    policy: Arc<dyn EscalationPolicy>,
}

impl EscalateServer {
    pub fn new<Policy>(shell_path: PathBuf, execve_wrapper: PathBuf, policy: Policy) -> Self
    where
        Policy: EscalationPolicy + Send + Sync + 'static,
    {
        Self {
            shell_path,
            execve_wrapper,
            policy: Arc::new(policy),
        }
    }

    pub async fn exec(
        &self,
        params: ExecParams,
        cancel_rx: CancellationToken,
        command_executor: Arc<dyn ShellCommandExecutor>,
    ) -> anyhow::Result<ExecResult> {
        let session = self.start_session(cancel_rx.clone(), Arc::clone(&command_executor))?;
        let env_overlay = session.env().clone();
        let client_socket = Arc::clone(&session.client_socket);
        let command = vec![
            self.shell_path.to_string_lossy().to_string(),
            if params.login == Some(false) {
                "-c".to_string()
            } else {
                "-lc".to_string()
            },
            params.command,
        ];
        let workdir = AbsolutePathBuf::try_from(params.workdir)?;
        let result = command_executor
            .run(
                command,
                workdir.to_path_buf(),
                env_overlay,
                cancel_rx,
                Some(Box::new(move || {
                    if let Ok(mut client_socket) = client_socket.lock() {
                        client_socket.take();
                    }
                })),
            )
            .await?;
        Ok(result)
    }

    /// Starts an escalation session and returns the environment overlay a shell
    /// needs in order to route intercepted execs through this server.
    ///
    /// This does not spawn the shell itself. Callers own process creation and
    /// only use the returned environment plus the session lifetime handle.
    pub fn start_session(
        &self,
        parent_cancellation_token: CancellationToken,
        command_executor: Arc<dyn ShellCommandExecutor>,
    ) -> anyhow::Result<EscalationSession> {
        let cancellation_token = CancellationToken::new();
        let (escalate_server, escalate_client) = AsyncDatagramSocket::pair()?;
        let client_socket = escalate_client.into_inner();
        let client_socket_fd = client_socket.as_raw_fd();
        // Only the client endpoint should cross exec into the wrapper process.
        client_socket.set_cloexec(false)?;
        let client_socket = Arc::new(Mutex::new(Some(client_socket)));
        let task = tokio::spawn(escalate_task(
            escalate_server,
            Arc::clone(&self.policy),
            Arc::clone(&command_executor),
            parent_cancellation_token,
            cancellation_token.clone(),
        ));
        let mut env = HashMap::new();
        env.insert(
            ESCALATE_SOCKET_ENV_VAR.to_string(),
            client_socket_fd.to_string(),
        );
        env.insert(
            EXEC_WRAPPER_ENV_VAR.to_string(),
            self.execve_wrapper.to_string_lossy().to_string(),
        );
        Ok(EscalationSession {
            env,
            task,
            client_socket,
            cancellation_token,
        })
    }
}

async fn escalate_task(
    socket: AsyncDatagramSocket,
    policy: Arc<dyn EscalationPolicy>,
    command_executor: Arc<dyn ShellCommandExecutor>,
    parent_cancellation_token: CancellationToken,
    session_cancellation_token: CancellationToken,
) -> anyhow::Result<()> {
    loop {
        let (_, mut fds) = tokio::select! {
            received = socket.receive_with_fds() => received?,
            _ = parent_cancellation_token.cancelled() => return Ok(()),
            _ = session_cancellation_token.cancelled() => return Ok(()),
        };
        if fds.len() != 1 {
            tracing::error!("expected 1 fd in datagram handshake, got {}", fds.len());
            continue;
        }
        let stream_socket = AsyncSocket::from_fd(fds.remove(0))?;
        let policy = Arc::clone(&policy);
        let command_executor = Arc::clone(&command_executor);
        let parent_cancellation_token = parent_cancellation_token.clone();
        let session_cancellation_token = session_cancellation_token.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_escalate_session_with_policy(
                stream_socket,
                policy,
                command_executor,
                parent_cancellation_token,
                session_cancellation_token,
            )
            .await
            {
                tracing::error!("escalate session failed: {err:?}");
            }
        });
    }
}

async fn handle_escalate_session_with_policy(
    socket: AsyncSocket,
    policy: Arc<dyn EscalationPolicy>,
    command_executor: Arc<dyn ShellCommandExecutor>,
    parent_cancellation_token: CancellationToken,
    session_cancellation_token: CancellationToken,
) -> anyhow::Result<()> {
    let EscalateRequest {
        file,
        argv,
        workdir,
        env,
    } = tokio::select! {
        request = socket.receive::<EscalateRequest>() => request?,
        _ = parent_cancellation_token.cancelled() => return Ok(()),
        _ = session_cancellation_token.cancelled() => return Ok(()),
    };
    let program = AbsolutePathBuf::resolve_path_against_base(file, workdir.as_path());
    let decision = tokio::select! {
        decision = policy.determine_action(&program, &argv, &workdir) => {
            decision.context("failed to determine escalation action")?
        }
        _ = parent_cancellation_token.cancelled() => return Ok(()),
        _ = session_cancellation_token.cancelled() => return Ok(()),
    };

    tracing::debug!("decided {decision:?} for {program:?} {argv:?} {workdir:?}");

    match decision {
        EscalationDecision::Run => {
            socket
                .send(EscalateResponse {
                    action: EscalateAction::Run,
                })
                .await?;
        }
        EscalationDecision::Escalate(execution) => {
            socket
                .send(EscalateResponse {
                    action: EscalateAction::Escalate,
                })
                .await?;
            let (msg, fds) = tokio::select! {
                message = socket.receive_with_fds::<SuperExecMessage>() => {
                    message.context("failed to receive SuperExecMessage")?
                }
                _ = parent_cancellation_token.cancelled() => return Ok(()),
                _ = session_cancellation_token.cancelled() => return Ok(()),
            };
            if fds.len() != msg.fds.len() {
                return Err(anyhow::anyhow!(
                    "mismatched number of fds in SuperExecMessage: {} in the message, {} from the control message",
                    msg.fds.len(),
                    fds.len()
                ));
            }

            let PreparedExec {
                command,
                cwd,
                env,
                arg0,
            } = tokio::select! {
                prepared = command_executor.prepare_escalated_exec(&program, &argv, &workdir, env, execution) => prepared?,
                _ = parent_cancellation_token.cancelled() => return Ok(()),
                _ = session_cancellation_token.cancelled() => return Ok(()),
            };
            let (program, args) = command
                .split_first()
                .ok_or_else(|| anyhow::anyhow!("prepared escalated command must not be empty"))?;
            let mut command = Command::new(program);
            command
                .args(args)
                .arg0(arg0.unwrap_or_else(|| program.clone()))
                .envs(&env)
                .current_dir(&cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true);
            unsafe {
                command.pre_exec(move || {
                    for (dst_fd, src_fd) in msg.fds.iter().zip(&fds) {
                        libc::dup2(src_fd.as_raw_fd(), *dst_fd);
                    }
                    Ok(())
                });
            }
            let mut child = command.spawn()?;
            let exit_status = tokio::select! {
                status = child.wait() => status?,
                _ = parent_cancellation_token.cancelled() => {
                    let _ = child.start_kill();
                    child.wait().await?
                }
                _ = session_cancellation_token.cancelled() => {
                    let _ = child.start_kill();
                    child.wait().await?
                }
            };
            socket
                .send(SuperExecResult {
                    exit_code: exit_status.code().unwrap_or(127),
                })
                .await?;
        }
        EscalationDecision::Deny { reason } => {
            socket
                .send(EscalateResponse {
                    action: EscalateAction::Deny { reason },
                })
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::approvals::EscalationPermissions;
    use codex_protocol::models::NetworkPermissions;
    use codex_protocol::models::PermissionProfile;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::fd::FromRawFd;
    use std::path::PathBuf;
    use std::sync::LazyLock;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;
    use tokio::time::Instant;
    use tokio::time::sleep;

    static ESCALATE_SERVER_TEST_LOCK: LazyLock<tokio::sync::Mutex<()>> =
        LazyLock::new(|| tokio::sync::Mutex::new(()));

    struct DeterministicEscalationPolicy {
        decision: EscalationDecision,
    }

    #[async_trait::async_trait]
    impl EscalationPolicy for DeterministicEscalationPolicy {
        async fn determine_action(
            &self,
            _file: &AbsolutePathBuf,
            _argv: &[String],
            _workdir: &AbsolutePathBuf,
        ) -> anyhow::Result<EscalationDecision> {
            Ok(self.decision.clone())
        }
    }

    struct AssertingEscalationPolicy {
        expected_file: AbsolutePathBuf,
        expected_workdir: AbsolutePathBuf,
    }

    #[async_trait::async_trait]
    impl EscalationPolicy for AssertingEscalationPolicy {
        async fn determine_action(
            &self,
            file: &AbsolutePathBuf,
            _argv: &[String],
            workdir: &AbsolutePathBuf,
        ) -> anyhow::Result<EscalationDecision> {
            assert_eq!(file, &self.expected_file);
            assert_eq!(workdir, &self.expected_workdir);
            Ok(EscalationDecision::run())
        }
    }

    struct ForwardingShellCommandExecutor;

    #[async_trait::async_trait]
    impl ShellCommandExecutor for ForwardingShellCommandExecutor {
        async fn run(
            &self,
            _command: Vec<String>,
            _cwd: PathBuf,
            _env_overlay: HashMap<String, String>,
            _cancel_rx: CancellationToken,
            _after_spawn: Option<Box<dyn FnOnce() + Send>>,
        ) -> anyhow::Result<ExecResult> {
            unreachable!("run() is not used by handle_escalate_session_with_policy() tests")
        }

        async fn prepare_escalated_exec(
            &self,
            program: &AbsolutePathBuf,
            argv: &[String],
            workdir: &AbsolutePathBuf,
            env: HashMap<String, String>,
            _execution: EscalationExecution,
        ) -> anyhow::Result<PreparedExec> {
            Ok(PreparedExec {
                command: std::iter::once(program.to_string_lossy().to_string())
                    .chain(argv.iter().skip(1).cloned())
                    .collect(),
                cwd: workdir.to_path_buf(),
                env,
                arg0: argv.first().cloned(),
            })
        }
    }

    struct PermissionAssertingShellCommandExecutor {
        expected_permissions: EscalationPermissions,
    }

    #[async_trait::async_trait]
    impl ShellCommandExecutor for PermissionAssertingShellCommandExecutor {
        async fn run(
            &self,
            _command: Vec<String>,
            _cwd: PathBuf,
            _env_overlay: HashMap<String, String>,
            _cancel_rx: CancellationToken,
            _after_spawn: Option<Box<dyn FnOnce() + Send>>,
        ) -> anyhow::Result<ExecResult> {
            unreachable!("run() is not used by handle_escalate_session_with_policy() tests")
        }

        async fn prepare_escalated_exec(
            &self,
            program: &AbsolutePathBuf,
            argv: &[String],
            workdir: &AbsolutePathBuf,
            env: HashMap<String, String>,
            execution: EscalationExecution,
        ) -> anyhow::Result<PreparedExec> {
            assert_eq!(
                execution,
                EscalationExecution::Permissions(self.expected_permissions.clone())
            );
            Ok(PreparedExec {
                command: std::iter::once(program.to_string_lossy().to_string())
                    .chain(argv.iter().skip(1).cloned())
                    .collect(),
                cwd: workdir.to_path_buf(),
                env,
                arg0: argv.first().cloned(),
            })
        }
    }

    async fn wait_for_pid_file(pid_file: &std::path::Path) -> anyhow::Result<i32> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(contents) = std::fs::read_to_string(pid_file) {
                return Ok(contents.trim().parse()?);
            }
            if Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "timed out waiting for pid file {}",
                    pid_file.display()
                ));
            }
            sleep(Duration::from_millis(20)).await;
        }
    }

    fn process_exists(pid: i32) -> bool {
        let rc = unsafe { libc::kill(pid, 0) };
        if rc == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }

    struct AfterSpawnAssertingShellCommandExecutor {
        after_spawn_invoked: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl ShellCommandExecutor for AfterSpawnAssertingShellCommandExecutor {
        async fn run(
            &self,
            _command: Vec<String>,
            _cwd: PathBuf,
            env_overlay: HashMap<String, String>,
            _cancel_rx: CancellationToken,
            after_spawn: Option<Box<dyn FnOnce() + Send>>,
        ) -> anyhow::Result<ExecResult> {
            let socket_fd = env_overlay
                .get(ESCALATE_SOCKET_ENV_VAR)
                .expect("session should export shell escalation socket")
                .parse::<i32>()?;
            assert_ne!(unsafe { libc::fcntl(socket_fd, libc::F_GETFD) }, -1);
            after_spawn.expect("one-shot exec should install an after-spawn hook")();
            self.after_spawn_invoked.store(true, Ordering::Relaxed);
            Ok(ExecResult {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                output: String::new(),
                duration: Duration::ZERO,
                timed_out: false,
            })
        }

        async fn prepare_escalated_exec(
            &self,
            _program: &AbsolutePathBuf,
            _argv: &[String],
            _workdir: &AbsolutePathBuf,
            _env: HashMap<String, String>,
            _execution: EscalationExecution,
        ) -> anyhow::Result<PreparedExec> {
            unreachable!("prepare_escalated_exec() is not used by exec() tests")
        }
    }

    async fn wait_for_process_exit(pid: i32) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if !process_exists(pid) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(anyhow::anyhow!("timed out waiting for pid {pid} to exit"));
            }
            sleep(Duration::from_millis(20)).await;
        }
    }

    /// Verifies that `start_session()` returns only the wrapper/socket env
    /// overlay and does not need to touch the configured shell or wrapper
    /// executable paths.
    ///
    /// The `/bin/zsh` and `/tmp/codex-execve-wrapper` values here are
    /// intentionally fake sentinels: this test asserts that the paths are
    /// copied into the exported environment and that the socket fd stays valid
    /// until `close_client_socket()` is called.
    #[tokio::test]
    async fn start_session_exposes_wrapper_env_overlay() -> anyhow::Result<()> {
        let _guard = ESCALATE_SERVER_TEST_LOCK.lock().await;
        let execve_wrapper = PathBuf::from("/tmp/codex-execve-wrapper");
        let execve_wrapper_str = execve_wrapper.to_string_lossy().to_string();
        let server = EscalateServer::new(
            PathBuf::from("/bin/zsh"),
            execve_wrapper,
            DeterministicEscalationPolicy {
                decision: EscalationDecision::run(),
            },
        );

        let session = server.start_session(
            CancellationToken::new(),
            Arc::new(ForwardingShellCommandExecutor),
        )?;
        let env = session.env();
        assert_eq!(env.get(EXEC_WRAPPER_ENV_VAR), Some(&execve_wrapper_str));
        let socket_fd = env
            .get(ESCALATE_SOCKET_ENV_VAR)
            .expect("session should export shell escalation socket");
        let socket_fd = socket_fd.parse::<i32>()?;
        assert!(socket_fd >= 0);
        assert_ne!(unsafe { libc::fcntl(socket_fd, libc::F_GETFD) }, -1);
        assert!(
            session
                .client_socket
                .lock()
                .is_ok_and(|socket| socket.is_some())
        );
        session.close_client_socket();
        assert!(
            session
                .client_socket
                .lock()
                .is_ok_and(|socket| socket.is_none())
        );

        Ok(())
    }

    #[tokio::test]
    async fn exec_closes_parent_socket_after_shell_spawn() -> anyhow::Result<()> {
        let _guard = ESCALATE_SERVER_TEST_LOCK.lock().await;
        let after_spawn_invoked = Arc::new(AtomicBool::new(false));
        let server = EscalateServer::new(
            PathBuf::from("/bin/bash"),
            PathBuf::from("/tmp/codex-execve-wrapper"),
            DeterministicEscalationPolicy {
                decision: EscalationDecision::run(),
            },
        );

        let result = server
            .exec(
                ExecParams {
                    command: "true".to_string(),
                    workdir: AbsolutePathBuf::current_dir()?
                        .to_string_lossy()
                        .to_string(),
                    timeout_ms: None,
                    login: Some(false),
                },
                CancellationToken::new(),
                Arc::new(AfterSpawnAssertingShellCommandExecutor {
                    after_spawn_invoked: Arc::clone(&after_spawn_invoked),
                }),
            )
            .await?;
        assert_eq!(0, result.exit_code);
        assert!(after_spawn_invoked.load(Ordering::Relaxed));

        Ok(())
    }

    #[tokio::test]
    async fn handle_escalate_session_respects_run_in_sandbox_decision() -> anyhow::Result<()> {
        let _guard = ESCALATE_SERVER_TEST_LOCK.lock().await;
        let (server, client) = AsyncSocket::pair()?;
        let server_task = tokio::spawn(handle_escalate_session_with_policy(
            server,
            Arc::new(DeterministicEscalationPolicy {
                decision: EscalationDecision::run(),
            }),
            Arc::new(ForwardingShellCommandExecutor),
            CancellationToken::new(),
            CancellationToken::new(),
        ));

        let mut env = HashMap::new();
        for i in 0..10 {
            let value = "A".repeat(1024);
            env.insert(format!("CODEX_TEST_VAR{i}"), value);
        }

        client
            .send(EscalateRequest {
                file: PathBuf::from("/bin/echo"),
                argv: vec!["echo".to_string()],
                workdir: AbsolutePathBuf::try_from(PathBuf::from("/tmp"))?,
                env,
            })
            .await?;

        let response = client.receive::<EscalateResponse>().await?;
        assert_eq!(
            EscalateResponse {
                action: EscalateAction::Run,
            },
            response
        );
        server_task.await?
    }

    #[tokio::test]
    async fn handle_escalate_session_resolves_relative_file_against_request_workdir()
    -> anyhow::Result<()> {
        let _guard = ESCALATE_SERVER_TEST_LOCK.lock().await;
        let (server, client) = AsyncSocket::pair()?;
        let tmp = tempfile::TempDir::new()?;
        let workdir = tmp.path().join("workspace");
        std::fs::create_dir(&workdir)?;
        let workdir = AbsolutePathBuf::try_from(workdir)?;
        let expected_file = workdir.join("bin/tool");
        let server_task = tokio::spawn(handle_escalate_session_with_policy(
            server,
            Arc::new(AssertingEscalationPolicy {
                expected_file,
                expected_workdir: workdir.clone(),
            }),
            Arc::new(ForwardingShellCommandExecutor),
            CancellationToken::new(),
            CancellationToken::new(),
        ));

        client
            .send(EscalateRequest {
                file: PathBuf::from("./bin/tool"),
                argv: vec!["./bin/tool".to_string()],
                workdir,
                env: HashMap::new(),
            })
            .await?;

        let response = client.receive::<EscalateResponse>().await?;
        assert_eq!(
            EscalateResponse {
                action: EscalateAction::Run,
            },
            response
        );
        server_task.await?
    }

    #[tokio::test]
    async fn handle_escalate_session_executes_escalated_command() -> anyhow::Result<()> {
        let _guard = ESCALATE_SERVER_TEST_LOCK.lock().await;
        let (server, client) = AsyncSocket::pair()?;
        let server_task = tokio::spawn(handle_escalate_session_with_policy(
            server,
            Arc::new(DeterministicEscalationPolicy {
                decision: EscalationDecision::escalate(EscalationExecution::Unsandboxed),
            }),
            Arc::new(ForwardingShellCommandExecutor),
            CancellationToken::new(),
            CancellationToken::new(),
        ));

        client
            .send(EscalateRequest {
                file: PathBuf::from("/bin/sh"),
                argv: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    r#"if [ "$KEY" = VALUE ]; then exit 42; else exit 1; fi"#.to_string(),
                ],
                workdir: AbsolutePathBuf::current_dir()?,
                env: HashMap::from([("KEY".to_string(), "VALUE".to_string())]),
            })
            .await?;

        let response = client.receive::<EscalateResponse>().await?;
        assert_eq!(
            EscalateResponse {
                action: EscalateAction::Escalate,
            },
            response
        );

        client
            .send_with_fds(SuperExecMessage { fds: Vec::new() }, &[])
            .await?;

        let result = client.receive::<SuperExecResult>().await?;
        assert_eq!(42, result.exit_code);

        server_task.await?
    }

    /// Saves a target descriptor, closes it, and restores it when dropped.
    ///
    /// The overlap regression test needs the next received `SCM_RIGHTS` handle
    /// to land on a specific descriptor number such as stdin. Temporarily
    /// closing the descriptor makes that allocation possible while still
    /// letting the test put the process back the way it found it.
    struct RestoredFd {
        target_fd: i32,
        original_fd: std::os::fd::OwnedFd,
    }

    impl RestoredFd {
        /// Duplicates `target_fd`, then closes the original descriptor number.
        ///
        /// The duplicate is kept alive so `Drop` can restore the original
        /// process state after the test finishes.
        fn close_temporarily(target_fd: i32) -> anyhow::Result<Self> {
            let original_fd = unsafe { libc::dup(target_fd) };
            if original_fd == -1 {
                return Err(std::io::Error::last_os_error().into());
            }
            if unsafe { libc::close(target_fd) } == -1 {
                let err = std::io::Error::last_os_error();
                unsafe {
                    libc::close(original_fd);
                }
                return Err(err.into());
            }
            Ok(Self {
                target_fd,
                original_fd: unsafe { std::os::fd::OwnedFd::from_raw_fd(original_fd) },
            })
        }
    }

    /// Restores the original descriptor back onto its original fd number.
    ///
    /// This keeps the overlap test self-contained even though it mutates the
    /// current process's stdio table.
    impl Drop for RestoredFd {
        fn drop(&mut self) {
            unsafe {
                libc::dup2(self.original_fd.as_raw_fd(), self.target_fd);
            }
        }
    }

    #[tokio::test]
    async fn handle_escalate_session_accepts_received_fds_that_overlap_destinations()
    -> anyhow::Result<()> {
        let _guard = ESCALATE_SERVER_TEST_LOCK.lock().await;
        let mut pipe_fds = [0; 2];
        if unsafe { libc::pipe(pipe_fds.as_mut_ptr()) } == -1 {
            return Err(std::io::Error::last_os_error().into());
        }
        let read_end = unsafe { std::os::fd::OwnedFd::from_raw_fd(pipe_fds[0]) };
        let mut write_end = unsafe { std::fs::File::from_raw_fd(pipe_fds[1]) };

        // Force the receive-side overlap case for stdin.
        //
        // SCM_RIGHTS installs received descriptors into the lowest available fd
        // numbers in the receiving process. The pipe is opened first so its
        // read end does not consume fd 0. After stdin is temporarily closed,
        // receiving `read_end` should reuse descriptor 0. The message below
        // also asks the server to map that received fd to destination fd 0, so
        // the pre-exec dup2 loop exercises the src_fd == dst_fd case.
        let stdin_restore = RestoredFd::close_temporarily(libc::STDIN_FILENO)?;
        let (server, client) = AsyncSocket::pair()?;
        let server_task = tokio::spawn(handle_escalate_session_with_policy(
            server,
            Arc::new(DeterministicEscalationPolicy {
                decision: EscalationDecision::escalate(EscalationExecution::Unsandboxed),
            }),
            Arc::new(ForwardingShellCommandExecutor),
            CancellationToken::new(),
            CancellationToken::new(),
        ));

        client
            .send(EscalateRequest {
                file: PathBuf::from("/bin/sh"),
                argv: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "IFS= read -r line && [ \"$line\" = overlap-ok ]".to_string(),
                ],
                workdir: AbsolutePathBuf::current_dir()?,
                env: HashMap::new(),
            })
            .await?;

        let response = client.receive::<EscalateResponse>().await?;
        assert_eq!(
            EscalateResponse {
                action: EscalateAction::Escalate,
            },
            response
        );

        client
            .send_with_fds(
                SuperExecMessage {
                    fds: vec![libc::STDIN_FILENO],
                },
                &[read_end],
            )
            .await?;
        write_end.write_all(b"overlap-ok\n")?;
        drop(write_end);

        let result = client.receive::<SuperExecResult>().await?;
        assert_eq!(
            0, result.exit_code,
            "expected the escalated child to read the sent stdin payload even when the received fd reuses fd 0"
        );
        drop(stdin_restore);

        server_task.await?
    }

    #[tokio::test]
    async fn handle_escalate_session_passes_permissions_to_executor() -> anyhow::Result<()> {
        let _guard = ESCALATE_SERVER_TEST_LOCK.lock().await;
        let (server, client) = AsyncSocket::pair()?;
        let server_task = tokio::spawn(handle_escalate_session_with_policy(
            server,
            Arc::new(DeterministicEscalationPolicy {
                decision: EscalationDecision::escalate(EscalationExecution::Permissions(
                    EscalationPermissions::AdditionalPermissionProfile(PermissionProfile {
                        network: Some(NetworkPermissions {
                            enabled: Some(true),
                        }),
                        ..Default::default()
                    }),
                )),
            }),
            Arc::new(PermissionAssertingShellCommandExecutor {
                expected_permissions: EscalationPermissions::AdditionalPermissionProfile(
                    PermissionProfile {
                        network: Some(NetworkPermissions {
                            enabled: Some(true),
                        }),
                        ..Default::default()
                    },
                ),
            }),
            CancellationToken::new(),
            CancellationToken::new(),
        ));

        client
            .send(EscalateRequest {
                file: PathBuf::from("/bin/sh"),
                argv: vec!["sh".to_string(), "-c".to_string(), "exit 0".to_string()],
                workdir: AbsolutePathBuf::current_dir()?,
                env: HashMap::new(),
            })
            .await?;

        let response = client.receive::<EscalateResponse>().await?;
        assert_eq!(
            EscalateResponse {
                action: EscalateAction::Escalate,
            },
            response
        );

        client
            .send_with_fds(SuperExecMessage { fds: Vec::new() }, &[])
            .await?;

        let result = client.receive::<SuperExecResult>().await?;
        assert_eq!(0, result.exit_code);

        server_task.await?
    }

    #[tokio::test]
    async fn dropping_session_aborts_intercept_workers_and_kills_spawned_child()
    -> anyhow::Result<()> {
        let _guard = ESCALATE_SERVER_TEST_LOCK.lock().await;
        let tmp = TempDir::new()?;
        let pid_file = tmp.path().join("escalated-child.pid");
        let pid_file_display = pid_file.display().to_string();
        assert!(
            !pid_file_display.contains('\''),
            "test temp path should not contain single quotes: {pid_file_display}"
        );
        let server = EscalateServer::new(
            PathBuf::from("/bin/bash"),
            PathBuf::from("/tmp/codex-execve-wrapper"),
            DeterministicEscalationPolicy {
                decision: EscalationDecision::escalate(EscalationExecution::Unsandboxed),
            },
        );

        let session = server.start_session(
            CancellationToken::new(),
            Arc::new(ForwardingShellCommandExecutor),
        )?;
        let socket_fd = session
            .env()
            .get(ESCALATE_SOCKET_ENV_VAR)
            .expect("session should export shell escalation socket")
            .parse::<i32>()?;
        let dup_socket_fd = unsafe { libc::dup(socket_fd) };
        assert!(dup_socket_fd >= 0, "expected dup() to succeed");
        let handshake_client = unsafe { AsyncDatagramSocket::from_raw_fd(dup_socket_fd) }?;
        let (server_stream, client_stream) = AsyncSocket::pair()?;
        // Keep one local reference to the server end alive until the worker has
        // responded once. Without that guard, macOS can observe EOF on the
        // client side before the transferred fd is fully servicing the stream.
        let server_stream_guard = server_stream.into_inner();
        let dup_server_stream_fd = unsafe { libc::dup(server_stream_guard.as_raw_fd()) };
        assert!(
            dup_server_stream_fd >= 0,
            "expected dup() of server stream to succeed"
        );
        let server_stream_fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(dup_server_stream_fd) };
        handshake_client
            .send_with_fds(&[0], &[server_stream_fd])
            .await
            .context("failed to send handshake datagram")?;

        client_stream
            .send(EscalateRequest {
                file: PathBuf::from("/bin/sh"),
                argv: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    format!("echo $$ > '{pid_file_display}' && exec /bin/sleep 100"),
                ],
                workdir: AbsolutePathBuf::current_dir()?,
                env: HashMap::new(),
            })
            .await
            .context("failed to send EscalateRequest")?;

        let response = client_stream
            .receive::<EscalateResponse>()
            .await
            .context("failed to receive EscalateResponse")?;
        assert_eq!(
            EscalateResponse {
                action: EscalateAction::Escalate,
            },
            response
        );
        drop(server_stream_guard);

        client_stream
            .send_with_fds(SuperExecMessage { fds: Vec::new() }, &[])
            .await
            .context("failed to send SuperExecMessage")?;

        let pid = wait_for_pid_file(&pid_file).await?;
        assert!(
            process_exists(pid),
            "expected spawned child pid {pid} to exist"
        );

        drop(session);

        wait_for_process_exit(pid).await?;

        Ok(())
    }
}
