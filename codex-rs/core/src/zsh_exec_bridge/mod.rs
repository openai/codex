use crate::exec::ExecToolCallOutput;
use crate::tools::sandboxing::ToolError;
use std::path::PathBuf;
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
use serde::Deserialize;
#[cfg(unix)]
use serde::Serialize;
#[cfg(unix)]
use std::io::Read;
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::time::Instant;
#[cfg(unix)]
use tokio::io::AsyncReadExt;
#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(unix)]
use tokio::net::UnixStream;

pub(crate) const ZSH_EXEC_BRIDGE_WRAPPER_SOCKET_ENV_VAR: &str =
    "CODEX_ZSH_EXEC_BRIDGE_WRAPPER_SOCKET";
pub(crate) const ZSH_EXEC_WRAPPER_MODE_ENV_VAR: &str = "CODEX_ZSH_EXEC_WRAPPER_MODE";
#[cfg(unix)]
pub(crate) const EXEC_WRAPPER_ENV_VAR: &str = "EXEC_WRAPPER";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ZshExecBridgeSessionState {
    pub(crate) initialized_session_id: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct ZshExecBridge {
    zsh_path: Option<PathBuf>,
    state: Mutex<ZshExecBridgeSessionState>,
}

#[cfg(unix)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WrapperIpcRequest {
    ExecRequest {
        request_id: String,
        file: String,
        argv: Vec<String>,
        cwd: String,
    },
}

#[cfg(unix)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WrapperIpcResponse {
    ExecResponse {
        request_id: String,
        action: WrapperExecAction,
        reason: Option<String>,
    },
}

#[cfg(unix)]
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WrapperExecAction {
    Run,
    Deny,
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

    pub(crate) fn next_wrapper_socket_path(&self) -> PathBuf {
        let socket_id = Uuid::new_v4().as_simple().to_string();
        let temp_dir = std::env::temp_dir();
        let canonical_temp_dir = temp_dir.canonicalize().unwrap_or(temp_dir);
        canonical_temp_dir.join(format!("czs-{}.sock", &socket_id[..12]))
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

        let wrapper_socket_path = req
            .env
            .get(ZSH_EXEC_BRIDGE_WRAPPER_SOCKET_ENV_VAR)
            .map(PathBuf::from)
            .unwrap_or_else(|| self.next_wrapper_socket_path());

        let listener = {
            let _ = std::fs::remove_file(&wrapper_socket_path);
            UnixListener::bind(&wrapper_socket_path).map_err(|err| {
                ToolError::Rejected(format!(
                    "bind wrapper socket at {}: {err}",
                    wrapper_socket_path.display()
                ))
            })?
        };

        let wrapper_path = std::env::current_exe().map_err(|err| {
            ToolError::Rejected(format!("resolve current executable path: {err}"))
        })?;

        let mut cmd = tokio::process::Command::new(&command[0]);
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
        cmd.env(
            ZSH_EXEC_BRIDGE_WRAPPER_SOCKET_ENV_VAR,
            wrapper_socket_path.to_string_lossy().to_string(),
        );
        cmd.env(EXEC_WRAPPER_ENV_VAR, &wrapper_path);
        cmd.env(ZSH_EXEC_WRAPPER_MODE_ENV_VAR, "1");

        let mut child = cmd.spawn().map_err(|err| {
            ToolError::Rejected(format!(
                "failed to start zsh fork command {} with zsh_path {}: {err}",
                command[0],
                zsh_path.display()
            ))
        })?;

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
                    child_exit = Some(result.map_err(|err| ToolError::Rejected(format!("wait for zsh fork command exit: {err}")))?);
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
                accept_result = listener.accept(), if child_exit.is_none() => {
                    let (stream, _) = accept_result.map_err(|err| {
                        ToolError::Rejected(format!("failed to accept wrapper request: {err}"))
                    })?;
                    if self
                        .handle_wrapper_request(stream, req.justification.clone(), session, turn, call_id)
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

        let _ = std::fs::remove_file(&wrapper_socket_path);

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
        mut stream: UnixStream,
        approval_reason: Option<String>,
        session: &crate::codex::Session,
        turn: &crate::codex::TurnContext,
        call_id: &str,
    ) -> Result<bool, ToolError> {
        let mut request_buf = Vec::new();
        stream.read_to_end(&mut request_buf).await.map_err(|err| {
            ToolError::Rejected(format!("read wrapper request from socket: {err}"))
        })?;
        let request_line = String::from_utf8(request_buf).map_err(|err| {
            ToolError::Rejected(format!("decode wrapper request as utf-8: {err}"))
        })?;
        let request = parse_wrapper_request_line(request_line.trim())?;

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

        let (action, reason, user_rejected) = match decision {
            ReviewDecision::Approved
            | ReviewDecision::ApprovedForSession
            | ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
                (WrapperExecAction::Run, None, false)
            }
            ReviewDecision::Denied => (
                WrapperExecAction::Deny,
                Some("command denied by host approval policy".to_string()),
                true,
            ),
            ReviewDecision::Abort => (
                WrapperExecAction::Deny,
                Some("command aborted by host approval policy".to_string()),
                true,
            ),
        };

        write_json_line(
            &mut stream,
            &WrapperIpcResponse::ExecResponse {
                request_id,
                action,
                reason,
            },
        )
        .await?;

        Ok(user_rejected)
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

pub fn maybe_run_zsh_exec_wrapper_mode() -> anyhow::Result<bool> {
    if std::env::var_os(ZSH_EXEC_WRAPPER_MODE_ENV_VAR).is_none() {
        return Ok(false);
    }

    run_exec_wrapper_mode()?;
    Ok(true)
}

fn run_exec_wrapper_mode() -> anyhow::Result<()> {
    #[cfg(not(unix))]
    {
        anyhow::bail!("zsh exec wrapper mode is only supported on unix");
    }

    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream as StdUnixStream;

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
        let cwd = std::env::current_dir()
            .context("resolve wrapper cwd")?
            .to_string_lossy()
            .to_string();
        let socket_path = std::env::var(ZSH_EXEC_BRIDGE_WRAPPER_SOCKET_ENV_VAR)
            .context("missing wrapper socket path env var")?;

        let request_id = Uuid::new_v4().to_string();
        let request = WrapperIpcRequest::ExecRequest {
            request_id: request_id.clone(),
            file: file.clone(),
            argv: argv.clone(),
            cwd,
        };

        let mut stream = StdUnixStream::connect(&socket_path)
            .with_context(|| format!("connect to wrapper socket at {socket_path}"))?;
        let encoded = serde_json::to_string(&request).context("serialize wrapper request")?;
        stream
            .write_all(encoded.as_bytes())
            .context("write wrapper request")?;
        stream
            .write_all(b"\n")
            .context("write wrapper request newline")?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .context("shutdown wrapper write")?;

        let mut response_buf = String::new();
        stream
            .read_to_string(&mut response_buf)
            .context("read wrapper response")?;
        let response: WrapperIpcResponse =
            serde_json::from_str(response_buf.trim()).context("parse wrapper response")?;

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
        if argv.len() > 1 {
            command.args(&argv[1..]);
        }
        command.env_remove(ZSH_EXEC_WRAPPER_MODE_ENV_VAR);
        command.env_remove(ZSH_EXEC_BRIDGE_WRAPPER_SOCKET_ENV_VAR);
        command.env_remove(EXEC_WRAPPER_ENV_VAR);
        let status = command.status().context("spawn wrapped executable")?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

#[cfg(unix)]
fn parse_wrapper_request_line(request_line: &str) -> Result<WrapperIpcRequest, ToolError> {
    serde_json::from_str(request_line)
        .map_err(|err| ToolError::Rejected(format!("parse wrapper request payload: {err}")))
}

#[cfg(unix)]
async fn write_json_line<W: tokio::io::AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    message: &T,
) -> Result<(), ToolError> {
    let encoded = serde_json::to_string(message)
        .map_err(|err| ToolError::Rejected(format!("serialize wrapper message: {err}")))?;
    tokio::io::AsyncWriteExt::write_all(writer, encoded.as_bytes())
        .await
        .map_err(|err| ToolError::Rejected(format!("write wrapper message: {err}")))?;
    tokio::io::AsyncWriteExt::write_all(writer, b"\n")
        .await
        .map_err(|err| ToolError::Rejected(format!("write wrapper newline: {err}")))?;
    tokio::io::AsyncWriteExt::flush(writer)
        .await
        .map_err(|err| ToolError::Rejected(format!("flush wrapper message: {err}")))?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn parse_wrapper_request_line_rejects_malformed_json() {
        let err = parse_wrapper_request_line("this-is-not-json").unwrap_err();
        let ToolError::Rejected(message) = err else {
            panic!("expected ToolError::Rejected");
        };
        assert!(message.starts_with("parse wrapper request payload:"));
    }
}
