use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecToolCallOutput;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandOutputDeltaEvent;
use crate::protocol::ExecOutputStream;
use crate::protocol::ReviewDecision;
use crate::protocol::TerminalInteractionEvent;
use crate::tools::sandboxing::ToolError;
use crate::zsh_sidecar::protocol::ApprovalDecision;
use crate::zsh_sidecar::protocol::EmptyResult;
use crate::zsh_sidecar::protocol::ExecExitedEvent;
use crate::zsh_sidecar::protocol::ExecPolicyAmendmentProposal;
use crate::zsh_sidecar::protocol::ExecStartParams;
use crate::zsh_sidecar::protocol::ExecStartedEvent;
use crate::zsh_sidecar::protocol::ExecStderrEvent;
use crate::zsh_sidecar::protocol::ExecStdoutEvent;
use crate::zsh_sidecar::protocol::InitializeParams;
use crate::zsh_sidecar::protocol::InitializeResult;
use crate::zsh_sidecar::protocol::JSONRPC_VERSION;
use crate::zsh_sidecar::protocol::JsonRpcId;
use crate::zsh_sidecar::protocol::JsonRpcNotification;
use crate::zsh_sidecar::protocol::JsonRpcRequest;
use crate::zsh_sidecar::protocol::JsonRpcSuccess;
use crate::zsh_sidecar::protocol::METHOD_ZSH_EVENT_EXEC_EXITED;
use crate::zsh_sidecar::protocol::METHOD_ZSH_EVENT_EXEC_STARTED;
use crate::zsh_sidecar::protocol::METHOD_ZSH_EVENT_EXEC_STDERR;
use crate::zsh_sidecar::protocol::METHOD_ZSH_EVENT_EXEC_STDOUT;
use crate::zsh_sidecar::protocol::METHOD_ZSH_EVENT_TERMINAL_INTERACTION;
use crate::zsh_sidecar::protocol::METHOD_ZSH_EXEC_START;
use crate::zsh_sidecar::protocol::METHOD_ZSH_INITIALIZE;
use crate::zsh_sidecar::protocol::METHOD_ZSH_REQUEST_APPROVAL;
use crate::zsh_sidecar::protocol::METHOD_ZSH_SHUTDOWN;
use crate::zsh_sidecar::protocol::RequestApprovalParams;
use crate::zsh_sidecar::protocol::RequestApprovalResult;
use crate::zsh_sidecar::protocol::ShutdownParams;
use base64::Engine as _;
use codex_protocol::approvals::ExecPolicyAmendment;
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::ChildStdin;
use tokio::sync::Mutex;
use uuid::Uuid;

pub(crate) mod protocol;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ZshSidecarState {
    pub(crate) initialized_session_id: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct ZshSidecarManager {
    zsh_path: Option<PathBuf>,
    codex_home: Option<PathBuf>,
    state: Mutex<ZshSidecarState>,
}

impl ZshSidecarManager {
    pub(crate) fn new(zsh_path: Option<PathBuf>, codex_home: PathBuf) -> Self {
        Self {
            zsh_path,
            codex_home: Some(codex_home),
            state: Mutex::new(ZshSidecarState::default()),
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

    pub(crate) fn zsh_path(&self) -> Option<&PathBuf> {
        self.zsh_path.as_ref()
    }

    pub(crate) fn codex_home(&self) -> Option<&PathBuf> {
        self.codex_home.as_ref()
    }

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
        let session_id = {
            let state = self.state.lock().await;
            state.initialized_session_id.clone().ok_or_else(|| {
                ToolError::Rejected(
                    "zsh sidecar manager is not initialized for a session".to_string(),
                )
            })?
        };
        let command = req.command.clone();
        if command.is_empty() {
            return Err(ToolError::Rejected("command args are empty".to_string()));
        }

        let mut child = tokio::process::Command::new("codex-zsh-sidecar")
            .arg("--zsh-path")
            .arg(&zsh_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| {
                ToolError::Rejected(format!(
                    "failed to start zsh sidecar `codex-zsh-sidecar`: {err}"
                ))
            })?;

        let mut stdin = child.stdin.take().ok_or_else(|| {
            ToolError::Rejected("zsh sidecar subprocess missing stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ToolError::Rejected("zsh sidecar subprocess missing stdout".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            ToolError::Rejected("zsh sidecar subprocess missing stderr".to_string())
        })?;

        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    tracing::warn!("zsh sidecar stderr: {trimmed}");
                }
            }
        });

        let mut lines = BufReader::new(stdout).lines();

        let mut next_id = 1_i64;
        let initialize_req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: JsonRpcId::Number(next_id),
            method: METHOD_ZSH_INITIALIZE.to_string(),
            params: InitializeParams { session_id },
        };
        next_id += 1;

        Self::write_json_line(&mut stdin, &initialize_req).await?;
        let _initialize_result: InitializeResult = self
            .wait_for_response(
                &mut lines,
                &mut stdin,
                initialize_req.id.clone(),
                session,
                turn,
                call_id,
            )
            .await?;

        let exec_id = format!("exec-{}", Uuid::new_v4());
        let exec_start_req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: JsonRpcId::Number(next_id),
            method: METHOD_ZSH_EXEC_START.to_string(),
            params: ExecStartParams {
                exec_id: exec_id.clone(),
                command,
                cwd: req.cwd.to_string_lossy().to_string(),
                env: Some(req.env.clone().into_iter().collect()),
                tty: Some(false),
                cols: None,
                rows: None,
            },
        };
        next_id += 1;

        Self::write_json_line(&mut stdin, &exec_start_req).await?;
        let _exec_start_result: EmptyResult = self
            .wait_for_response(
                &mut lines,
                &mut stdin,
                exec_start_req.id.clone(),
                session,
                turn,
                call_id,
            )
            .await?;

        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let mut exit_event = None;
        let start = Instant::now();

        while exit_event.is_none() {
            let line = lines
                .next_line()
                .await
                .map_err(|err| ToolError::Rejected(format!("zsh sidecar read error: {err}")))?
                .ok_or_else(|| {
                    ToolError::Rejected(
                        "zsh sidecar stream closed before command completion".to_string(),
                    )
                })?;
            self.handle_message(
                &line,
                &mut stdin,
                session,
                turn,
                call_id,
                &exec_id,
                &mut stdout_bytes,
                &mut stderr_bytes,
                &mut exit_event,
            )
            .await?;
        }

        let shutdown_req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: JsonRpcId::Number(next_id),
            method: METHOD_ZSH_SHUTDOWN.to_string(),
            params: ShutdownParams { grace_ms: None },
        };
        if Self::write_json_line(&mut stdin, &shutdown_req)
            .await
            .is_ok()
        {
            let _shutdown: Result<EmptyResult, ToolError> = self
                .wait_for_response(
                    &mut lines,
                    &mut stdin,
                    shutdown_req.id.clone(),
                    session,
                    turn,
                    call_id,
                )
                .await;
        }
        match tokio::time::timeout(Duration::from_secs(1), child.wait()).await {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                tracing::warn!("zsh sidecar wait after shutdown failed: {err}");
            }
            Err(_) => {
                tracing::warn!("zsh sidecar did not exit after shutdown; killing process");
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        }
        let exited = exit_event.ok_or_else(|| {
            ToolError::Rejected("zsh sidecar did not emit execExited event".to_string())
        })?;
        let stdout_text = crate::text_encoding::bytes_to_string_smart(&stdout_bytes);
        let stderr_text = crate::text_encoding::bytes_to_string_smart(&stderr_bytes);

        Ok(ExecToolCallOutput {
            exit_code: exited.exit_code,
            stdout: crate::exec::StreamOutput::new(stdout_text.clone()),
            stderr: crate::exec::StreamOutput::new(stderr_text.clone()),
            aggregated_output: crate::exec::StreamOutput::new(format!(
                "{stdout_text}{stderr_text}"
            )),
            duration: start.elapsed(),
            timed_out: exited.timed_out.unwrap_or(false),
        })
        .and_then(|output| Self::map_exec_result(req.sandbox, output))
    }

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
            })));
        }

        Ok(output)
    }

    async fn wait_for_response<T: serde::de::DeserializeOwned>(
        &self,
        lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
        stdin: &mut ChildStdin,
        expected_id: JsonRpcId,
        session: &crate::codex::Session,
        turn: &crate::codex::TurnContext,
        call_id: &str,
    ) -> Result<T, ToolError> {
        loop {
            let line = lines
                .next_line()
                .await
                .map_err(|err| ToolError::Rejected(format!("zsh sidecar read error: {err}")))?
                .ok_or_else(|| {
                    ToolError::Rejected("zsh sidecar stream closed unexpectedly".to_string())
                })?;

            let value: JsonValue = serde_json::from_str(&line).map_err(|err| {
                ToolError::Rejected(format!("zsh sidecar sent invalid JSON: {err}"))
            })?;

            if value.get("id").is_some()
                && (value.get("result").is_some() || value.get("error").is_some())
            {
                let id: JsonRpcId = serde_json::from_value(value["id"].clone()).map_err(|err| {
                    ToolError::Rejected(format!("zsh sidecar sent invalid response id: {err}"))
                })?;

                if id != expected_id {
                    tracing::warn!("ignoring unexpected zsh sidecar response id while waiting");
                    continue;
                }

                if value.get("error").is_some() {
                    let message = value["error"]["message"]
                        .as_str()
                        .unwrap_or("unknown zsh sidecar error");
                    return Err(ToolError::Rejected(format!(
                        "zsh sidecar request failed: {message}"
                    )));
                }

                return serde_json::from_value(value["result"].clone()).map_err(|err| {
                    ToolError::Rejected(format!("zsh sidecar result parse error: {err}"))
                });
            }

            let mut ignored_stdout = Vec::new();
            let mut ignored_stderr = Vec::new();
            let mut ignored_exit = None;
            self.handle_message(
                &line,
                stdin,
                session,
                turn,
                call_id,
                "",
                &mut ignored_stdout,
                &mut ignored_stderr,
                &mut ignored_exit,
            )
            .await?;
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_message(
        &self,
        line: &str,
        stdin: &mut ChildStdin,
        session: &crate::codex::Session,
        turn: &crate::codex::TurnContext,
        call_id: &str,
        exec_id: &str,
        stdout_bytes: &mut Vec<u8>,
        stderr_bytes: &mut Vec<u8>,
        exit_event: &mut Option<ExecExitedEvent>,
    ) -> Result<(), ToolError> {
        let value: JsonValue = serde_json::from_str(line).map_err(|err| {
            ToolError::Rejected(format!("zsh sidecar sent invalid JSON message: {err}"))
        })?;
        let method = value
            .get("method")
            .and_then(JsonValue::as_str)
            .unwrap_or_default();

        if method.is_empty() {
            return Ok(());
        }

        if value.get("id").is_some() {
            if method == METHOD_ZSH_REQUEST_APPROVAL {
                let id: JsonRpcId = serde_json::from_value(value["id"].clone()).map_err(|err| {
                    ToolError::Rejected(format!("invalid zsh sidecar approval callback id: {err}"))
                })?;
                let req: JsonRpcRequest<RequestApprovalParams> = serde_json::from_value(value)
                    .map_err(|err| {
                        ToolError::Rejected(format!("invalid zsh sidecar approval payload: {err}"))
                    })?;

                let decision = session
                    .request_command_approval(
                        turn,
                        format!("{call_id}:{}", req.params.approval_id),
                        req.params.command.clone(),
                        PathBuf::from(req.params.cwd),
                        Some(req.params.reason.clone()),
                        req.params
                            .proposed_execpolicy_amendment
                            .as_ref()
                            .map(Self::proposal_to_execpolicy_amendment),
                    )
                    .await;

                let response = JsonRpcSuccess {
                    jsonrpc: JSONRPC_VERSION.to_string(),
                    id,
                    result: RequestApprovalResult {
                        decision: Self::review_to_approval_decision(decision),
                    },
                };
                Self::write_json_line(stdin, &response).await?;
            }
            return Ok(());
        }

        match method {
            METHOD_ZSH_EVENT_EXEC_STARTED => {
                let note: JsonRpcNotification<ExecStartedEvent> = serde_json::from_value(value)
                    .map_err(|err| {
                        ToolError::Rejected(format!("invalid zsh execStarted event: {err}"))
                    })?;
                if !exec_id.is_empty() && note.params.exec_id != exec_id {
                    return Ok(());
                }
            }
            METHOD_ZSH_EVENT_EXEC_STDOUT => {
                let note: JsonRpcNotification<ExecStdoutEvent> = serde_json::from_value(value)
                    .map_err(|err| {
                        ToolError::Rejected(format!("invalid zsh execStdout event: {err}"))
                    })?;
                if !exec_id.is_empty() && note.params.exec_id != exec_id {
                    return Ok(());
                }

                let chunk = base64::engine::general_purpose::STANDARD
                    .decode(note.params.chunk_base64)
                    .map_err(|err| {
                        ToolError::Rejected(format!(
                            "invalid base64 stdout chunk from sidecar: {err}"
                        ))
                    })?;
                stdout_bytes.extend_from_slice(&chunk);

                session
                    .send_event(
                        turn,
                        EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                            call_id: call_id.to_string(),
                            stream: ExecOutputStream::Stdout,
                            chunk,
                        }),
                    )
                    .await;
            }
            METHOD_ZSH_EVENT_EXEC_STDERR => {
                let note: JsonRpcNotification<ExecStderrEvent> = serde_json::from_value(value)
                    .map_err(|err| {
                        ToolError::Rejected(format!("invalid zsh execStderr event: {err}"))
                    })?;
                if !exec_id.is_empty() && note.params.exec_id != exec_id {
                    return Ok(());
                }

                let chunk = base64::engine::general_purpose::STANDARD
                    .decode(note.params.chunk_base64)
                    .map_err(|err| {
                        ToolError::Rejected(format!(
                            "invalid base64 stderr chunk from sidecar: {err}"
                        ))
                    })?;
                stderr_bytes.extend_from_slice(&chunk);

                session
                    .send_event(
                        turn,
                        EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                            call_id: call_id.to_string(),
                            stream: ExecOutputStream::Stderr,
                            chunk,
                        }),
                    )
                    .await;
            }
            METHOD_ZSH_EVENT_TERMINAL_INTERACTION => {
                let note: JsonRpcNotification<protocol::TerminalInteractionEvent> =
                    serde_json::from_value(value).map_err(|err| {
                        ToolError::Rejected(format!("invalid zsh terminalInteraction event: {err}"))
                    })?;
                if !exec_id.is_empty() && note.params.exec_id != exec_id {
                    return Ok(());
                }

                session
                    .send_event(
                        turn,
                        EventMsg::TerminalInteraction(TerminalInteractionEvent {
                            call_id: call_id.to_string(),
                            process_id: note.params.exec_id,
                            stdin: note.params.interaction,
                        }),
                    )
                    .await;
            }
            METHOD_ZSH_EVENT_EXEC_EXITED => {
                let note: JsonRpcNotification<ExecExitedEvent> = serde_json::from_value(value)
                    .map_err(|err| {
                        ToolError::Rejected(format!("invalid zsh execExited event: {err}"))
                    })?;
                if !exec_id.is_empty() && note.params.exec_id != exec_id {
                    return Ok(());
                }
                *exit_event = Some(note.params);
            }
            _ => {
                tracing::warn!("unknown zsh sidecar message method: {method}");
            }
        }

        Ok(())
    }

    async fn write_json_line<T: serde::Serialize>(
        stdin: &mut ChildStdin,
        msg: &T,
    ) -> Result<(), ToolError> {
        let encoded = serde_json::to_string(msg)
            .map_err(|err| ToolError::Rejected(format!("zsh sidecar serialize error: {err}")));
        let encoded = encoded?;

        stdin
            .write_all(encoded.as_bytes())
            .await
            .map_err(|err| ToolError::Rejected(format!("zsh sidecar write error: {err}")))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|err| ToolError::Rejected(format!("zsh sidecar write error: {err}")))?;
        Ok(())
    }

    fn proposal_to_execpolicy_amendment(
        proposal: &ExecPolicyAmendmentProposal,
    ) -> ExecPolicyAmendment {
        ExecPolicyAmendment::new(proposal.command_prefix.clone())
    }

    fn review_to_approval_decision(decision: ReviewDecision) -> ApprovalDecision {
        match decision {
            ReviewDecision::Approved => ApprovalDecision::Approved,
            ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
                ApprovalDecision::ApprovedExecpolicyAmendment
            }
            ReviewDecision::ApprovedForSession => ApprovalDecision::ApprovedForSession,
            ReviewDecision::Denied => ApprovalDecision::Denied,
            ReviewDecision::Abort => ApprovalDecision::Abort,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn initialize_and_shutdown_track_session() {
        let manager = ZshSidecarManager::new(
            Some(PathBuf::from("/bin/zsh")),
            PathBuf::from("/tmp/codex-home"),
        );
        manager.initialize_for_session("session-1").await;
        {
            let state = manager.state.lock().await;
            assert_eq!(state.initialized_session_id.as_deref(), Some("session-1"));
        }
        manager.shutdown().await;
        {
            let state = manager.state.lock().await;
            assert_eq!(state.initialized_session_id, None);
        }
    }
}
