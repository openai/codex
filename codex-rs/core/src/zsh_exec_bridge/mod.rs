use crate::exec::ExecToolCallOutput;
use crate::tools::sandboxing::ToolError;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(unix)]
use crate::error::CodexErr;
#[cfg(unix)]
use crate::error::SandboxErr;
#[cfg(unix)]
use crate::protocol::EventMsg;
#[cfg(unix)]
use crate::protocol::ExecCommandOutputDeltaEvent;
#[cfg(unix)]
use crate::protocol::ExecOutputStream;
#[cfg(unix)]
use crate::protocol::ReviewDecision;
#[cfg(unix)]
use anyhow::Context as _;
#[cfg(unix)]
use codex_protocol::approvals::ExecPolicyAmendment;
#[cfg(unix)]
use codex_utils_pty::process_group::kill_child_process_group;
#[cfg(unix)]
#[cfg(unix)]
use tokio::io::AsyncReadExt;

use codex_shell_exec_bridge::AsyncSocket;
use codex_shell_exec_bridge::EXEC_WRAPPER_ENV_VAR;
use codex_shell_exec_bridge::WrapperExecAction;
use codex_shell_exec_bridge::WrapperIpcRequest;
use codex_shell_exec_bridge::WrapperIpcResponse;
use codex_shell_exec_bridge::ZSH_EXEC_BRIDGE_SOCKET_ENV_VAR;
use codex_shell_exec_bridge::ZSH_EXEC_WRAPPER_MODE_ENV_VAR;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ZshExecBridgeSessionState {
    pub(crate) initialized_session_id: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct ZshExecBridge {
    zsh_path: Option<PathBuf>,
    state: Mutex<ZshExecBridgeSessionState>,
}

impl ZshExecBridge {
    pub(crate) fn new(zsh_path: Option<PathBuf>, _codex_home: PathBuf) -> Self {
        Self {
            zsh_path,
            state: Mutex::new(ZshExecBridgeSessionState::default()),
        }
    }

    pub(crate) async fn initialize_for_session(&self, session_id: &str) {
        let mut state = self.state.lock().await;
        state.initialized_session_id = Some(session_id.to_string());
    }

    pub(crate) async fn shutdown(&self) {
        let mut state = self.state.lock().await;
        state.initialized_session_id = None;
    }

    #[cfg(not(unix))]
    pub(crate) async fn execute_shell_request(
        &self,
        _req: &crate::sandboxing::ExecRequest,
        _session: &crate::codex::Session,
        _turn: &crate::codex::TurnContext,
        _call_id: &str,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let _ = &self.zsh_path;
        Err(ToolError::Rejected(
            "shell_zsh_fork is only supported on unix".to_string(),
        ))
    }

    #[cfg(unix)]
    pub(crate) async fn execute_shell_request(
        &self,
        req: &crate::sandboxing::ExecRequest,
        session: &crate::codex::Session,
        turn: &crate::codex::TurnContext,
        call_id: &str,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let zsh_path = self.zsh_path.clone().ok_or_else(|| {
            ToolError::Rejected(
                "shell_zsh_fork enabled, but zsh_path is not configured".to_string(),
            )
        })?;

        let command = req.command.clone();
        if command.is_empty() {
            return Err(ToolError::Rejected("command args are empty".to_string()));
        }

        let (server_socket, client_socket) = AsyncSocket::pair().map_err(|err| {
            ToolError::Rejected(format!("failed to create zsh wrapper socket pair: {err}"))
        })?;
        client_socket.set_cloexec(false).map_err(|err| {
            ToolError::Rejected(format!("disable cloexec on wrapper socket: {err}"))
        })?;
        let fd = client_socket.as_raw_fd().to_string();

        let wrapper_path = std::env::current_exe().map_err(|err| {
            ToolError::Rejected(format!("resolve current executable path: {err}"))
        })?;

        let mut cmd = tokio::process::Command::new(&command[0]);
        #[cfg(unix)]
        if let Some(arg0) = &req.arg0 {
            cmd.arg0(arg0);
        }
        if command.len() > 1 {
            cmd.args(&command[1..]);
        }
        cmd.current_dir(&req.cwd);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);
        cmd.env_clear();
        cmd.envs(&req.env);
        cmd.env(ZSH_EXEC_BRIDGE_SOCKET_ENV_VAR, fd);
        cmd.env(EXEC_WRAPPER_ENV_VAR, &wrapper_path);
        cmd.env(ZSH_EXEC_WRAPPER_MODE_ENV_VAR, "1");

        let mut child = cmd.spawn().map_err(|err| {
            ToolError::Rejected(format!(
                "failed to start zsh fork command {} with zsh_path {}: {err}",
                command[0],
                zsh_path.display()
            ))
        })?;
        drop(client_socket);

        let (stream_tx, mut stream_rx) =
            tokio::sync::mpsc::unbounded_channel::<(ExecOutputStream, Vec<u8>)>();

        if let Some(mut out) = child.stdout.take() {
            let tx = stream_tx.clone();
            tokio::spawn(async move {
                let mut buf = [0_u8; 8192];
                loop {
                    let read = match out.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(err) => {
                            tracing::warn!("zsh fork stdout read error: {err}");
                            break;
                        }
                    };
                    let _ = tx.send((ExecOutputStream::Stdout, buf[..read].to_vec()));
                }
            });
        }

        if let Some(mut err) = child.stderr.take() {
            let tx = stream_tx.clone();
            tokio::spawn(async move {
                let mut buf = [0_u8; 8192];
                loop {
                    let read = match err.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(err) => {
                            tracing::warn!("zsh fork stderr read error: {err}");
                            break;
                        }
                    };
                    let _ = tx.send((ExecOutputStream::Stderr, buf[..read].to_vec()));
                }
            });
        }
        drop(stream_tx);

        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let mut child_exit = None;
        let mut timed_out = false;
        let mut stream_open = true;
        let mut user_rejected = false;
        let start = Instant::now();

        let expiration = req.expiration.clone().wait();
        tokio::pin!(expiration);

        while child_exit.is_none() || stream_open {
            tokio::select! {
                result = child.wait(), if child_exit.is_none() => {
                    child_exit = Some(
                        result.map_err(|err| {
                            ToolError::Rejected(format!(
                                "wait for zsh fork command exit: {err}"
                            ))
                        })?
                    );
                }
                stream = stream_rx.recv(), if stream_open => {
                    if let Some((output_stream, chunk)) = stream {
                        match output_stream {
                            ExecOutputStream::Stdout => stdout_bytes.extend_from_slice(&chunk),
                            ExecOutputStream::Stderr => stderr_bytes.extend_from_slice(&chunk),
                        }
                        session
                            .send_event(
                                turn,
                                EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                                    call_id: call_id.to_string(),
                                    stream: output_stream,
                                    chunk,
                                }),
                            )
                            .await;
                    } else {
                        stream_open = false;
                    }
                }
                result = server_socket.receive_with_fds::<WrapperIpcRequest>(), if child_exit.is_none() => {
                    let (request, fds) = result.map_err(|err| {
                        ToolError::Rejected(format!("failed to receive wrapper request: {err}"))
                    })?;
                    if !fds.is_empty() {
                        return Err(ToolError::Rejected(format!(
                            "unexpected fds in wrapper request: {}",
                            fds.len()
                        )));
                    }
                    if self
                        .handle_wrapper_request(
                            request,
                            req.justification.clone(),
                            session,
                            turn,
                            call_id,
                            &server_socket,
                        )
                        .await?
                    {
                        user_rejected = true;
                    }
                }
                _ = &mut expiration, if child_exit.is_none() => {
                    timed_out = true;
                    kill_child_process_group(&mut child).map_err(|err| {
                        ToolError::Rejected(format!("kill zsh fork command process group: {err}"))
                    })?;
                    child.start_kill().map_err(|err| {
                        ToolError::Rejected(format!("kill zsh fork command process: {err}"))
                    })?;
                }
            }
        }

        let status = child_exit.ok_or_else(|| {
            ToolError::Rejected("zsh fork command did not return exit status".to_string())
        })?;

        if user_rejected {
            return Err(ToolError::Rejected("rejected by user".to_string()));
        }

        let stdout_text = crate::text_encoding::bytes_to_string_smart(&stdout_bytes);
        let stderr_text = crate::text_encoding::bytes_to_string_smart(&stderr_bytes);
        let output = ExecToolCallOutput {
            exit_code: status.code().unwrap_or(-1),
            stdout: crate::exec::StreamOutput::new(stdout_text.clone()),
            stderr: crate::exec::StreamOutput::new(stderr_text.clone()),
            aggregated_output: crate::exec::StreamOutput::new(format!(
                "{stdout_text}{stderr_text}"
            )),
            duration: start.elapsed(),
            timed_out,
        };

        Self::map_exec_result(req.sandbox, output)
    }

    #[cfg(unix)]
    async fn handle_wrapper_request(
        &self,
        request: WrapperIpcRequest,
        approval_reason: Option<String>,
        session: &crate::codex::Session,
        turn: &crate::codex::TurnContext,
        call_id: &str,
        socket: &AsyncSocket,
    ) -> Result<bool, ToolError> {
        let (request_id, file, argv, cwd) = match request {
            WrapperIpcRequest::ExecRequest {
                request_id,
                file,
                argv,
                cwd,
            } => (request_id, file, argv, cwd),
        };

        let command_for_approval = if argv.is_empty() {
            vec![file.clone()]
        } else {
            argv.clone()
        };

        let approval_id = Uuid::new_v4().to_string();
        let decision = session
            .request_command_approval(
                turn,
                call_id.to_string(),
                Some(approval_id),
                command_for_approval,
                PathBuf::from(cwd),
                approval_reason,
                None,
                None::<ExecPolicyAmendment>,
            )
            .await;

        let (action, reason) = match decision {
            ReviewDecision::Approved
            | ReviewDecision::ApprovedForSession
            | ReviewDecision::ApprovedExecpolicyAmendment { .. } => (WrapperExecAction::Run, None),
            ReviewDecision::Denied => (
                WrapperExecAction::Deny,
                Some("command denied by host approval policy".to_string()),
            ),
            ReviewDecision::Abort => (
                WrapperExecAction::Deny,
                Some("command aborted by host approval policy".to_string()),
            ),
        };

        let response = WrapperIpcResponse::ExecResponse {
            request_id,
            action,
            reason,
        };
        socket
            .send(response.clone())
            .await
            .map_err(|err| ToolError::Rejected(format!("send wrapper response failed: {err}")))?;

        Ok(matches!(
            response,
            WrapperIpcResponse::ExecResponse {
                action: WrapperExecAction::Deny,
                ..
            }
        ))
    }

    #[cfg(unix)]
    fn map_exec_result(
        sandbox: crate::exec::SandboxType,
        output: ExecToolCallOutput,
    ) -> Result<ExecToolCallOutput, ToolError> {
        if output.timed_out {
            return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Timeout {
                output: Box::new(output),
            })));
        }

        if crate::exec::is_likely_sandbox_denied(sandbox, &output) {
            return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                output: Box::new(output),
                network_policy_decision: None,
            })));
        }

        Ok(output)
    }
}

pub async fn maybe_run_zsh_exec_wrapper_mode() -> anyhow::Result<bool> {
    if std::env::var_os(ZSH_EXEC_WRAPPER_MODE_ENV_VAR).is_none() {
        return Ok(false);
    }

    run_exec_wrapper_mode().await?;
    Ok(true)
}

#[cfg(unix)]
async fn run_exec_wrapper_mode() -> anyhow::Result<()> {
    let raw_fd = std::env::var(ZSH_EXEC_BRIDGE_SOCKET_ENV_VAR)?
        .parse::<i32>()
        .context("invalid wrapper socket fd")?;
    if raw_fd < 0 {
        anyhow::bail!("wrapper socket fd must be non-negative");
    }

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        anyhow::bail!("exec wrapper mode requires target executable path");
    }
    let file = args[1].clone();
    let argv = if args.len() > 2 {
        args[2..].to_vec()
    } else {
        vec![file.clone()]
    };
    let cwd = std::env::current_dir()?.to_string_lossy().to_string();

    let socket = {
        use std::os::fd::FromRawFd;
        use std::os::fd::OwnedFd;
        unsafe { AsyncSocket::from_fd(OwnedFd::from_raw_fd(raw_fd))? }
    };
    let request_id = Uuid::new_v4().to_string();
    let request = WrapperIpcRequest::ExecRequest {
        request_id: request_id.clone(),
        file: file.clone(),
        argv,
        cwd,
    };
    socket.send(request).await?;
    let response = socket.receive::<WrapperIpcResponse>().await?;
    let (response_request_id, action, reason) = match response {
        WrapperIpcResponse::ExecResponse {
            request_id,
            action,
            reason,
        } => (request_id, action, reason),
    };
    if response_request_id != request_id {
        anyhow::bail!(
            "wrapper response request_id mismatch: expected {request_id}, got {response_request_id}"
        );
    }

    if action == WrapperExecAction::Deny {
        if let Some(reason) = reason {
            tracing::warn!("execution denied: {reason}");
        } else {
            tracing::warn!("execution denied");
        }
        std::process::exit(1);
    }

    let mut command = std::process::Command::new(&file);
    if args.len() > 2 {
        command.args(&args[2..]);
    }
    command.env_remove(ZSH_EXEC_WRAPPER_MODE_ENV_VAR);
    command.env_remove(ZSH_EXEC_BRIDGE_SOCKET_ENV_VAR);
    command.env_remove(EXEC_WRAPPER_ENV_VAR);
    let status = command.status().context("spawn wrapped executable")?;
    std::process::exit(status.code().unwrap_or(1));
}
