/*
Runtime: shell

Executes shell requests under the orchestrator: asks for approval when needed,
builds sandbox transform inputs, and runs them under the current SandboxAttempt.
*/
#[cfg(unix)]
pub(crate) mod unix_escalation;
pub(crate) mod zsh_fork_backend;

use crate::command_canonicalization::canonicalize_command_for_approval;
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::exec::StdoutStream;
use crate::exec::is_likely_sandbox_denied;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::GuardianNetworkAccessTrigger;
use crate::guardian::review_approval_request;
use crate::sandboxing::ExecOptions;
use crate::sandboxing::ExecRequest;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::execute_env;
use crate::shell::ShellType;
use crate::tools::network_approval::NetworkApprovalMode;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::runtimes::build_sandbox_command;
use crate::tools::runtimes::exec_env_for_sandbox_permissions;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::PermissionRequestPayload;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxOverride;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::managed_network_for_sandbox_permissions;
use crate::tools::sandboxing::sandbox_override_for_first_attempt;
use crate::tools::sandboxing::with_cached_approval;
use codex_exec_server::Environment;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams as ExecServerParams;
use codex_exec_server::ProcessId;
use codex_network_proxy::NetworkProxy;
use codex_protocol::error::CodexErr;
use codex_protocol::error::SandboxErr;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::exec_output::StreamOutput;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandOutputDeltaEvent;
use codex_protocol::protocol::ReviewDecision;
use codex_sandboxing::SandboxablePreference;
use codex_shell_command::powershell::prefix_powershell_script_with_utf8;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_pty::DEFAULT_OUTPUT_BYTES_CAP;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Clone, Debug)]
pub struct ShellRequest {
    pub command: Vec<String>,
    pub hook_command: String,
    pub cwd: AbsolutePathBuf,
    pub environment: Arc<Environment>,
    pub timeout_ms: Option<u64>,
    pub env: HashMap<String, String>,
    pub exec_server_env_config: Option<crate::sandboxing::ExecServerEnvConfig>,
    pub explicit_env_overrides: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<AdditionalPermissionProfile>,
    #[cfg(unix)]
    pub additional_permissions_preapproved: bool,
    pub justification: Option<String>,
    pub exec_approval_requirement: ExecApprovalRequirement,
}

/// Selects `ShellRuntime` behavior for different callers.
///
/// Note: `Generic` is not the same as `ShellCommandClassic`.
/// `Generic` means "no `shell_command`-specific backend behavior" (used by the
/// generic `shell` tool path). The `ShellCommand*` variants are only for the
/// `shell_command` tool family.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ShellRuntimeBackend {
    /// Tool-agnostic/default runtime path.
    ///
    /// Uses the normal `ShellRuntime` execution flow without enabling any
    /// `shell_command`-specific backend selection.
    #[default]
    Generic,
    /// Legacy backend for the `shell_command` tool.
    ///
    /// Keeps `shell_command` on the standard shell runtime flow without the
    /// zsh-fork shell-escalation adapter.
    ShellCommandClassic,
    /// zsh-fork backend for the `shell_command` tool.
    ///
    /// On Unix, attempts to run via the zsh-fork + `codex-shell-escalation`
    /// adapter, with fallback to the standard shell runtime flow if
    /// prerequisites are not met.
    ShellCommandZshFork,
}

#[derive(Default)]
pub struct ShellRuntime {
    backend: ShellRuntimeBackend,
}

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    command: Vec<String>,
    cwd: AbsolutePathBuf,
    sandbox_permissions: SandboxPermissions,
    additional_permissions: Option<AdditionalPermissionProfile>,
}

impl ShellRuntime {
    pub fn new() -> Self {
        Self {
            backend: ShellRuntimeBackend::Generic,
        }
    }

    pub(crate) fn for_shell_command(backend: ShellRuntimeBackend) -> Self {
        Self { backend }
    }

    fn stdout_stream(ctx: &ToolCtx) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.turn.sub_id.clone(),
            call_id: ctx.call_id.clone(),
            tx_event: ctx.session.get_tx_event(),
        })
    }
}

impl Sandboxable for ShellRuntime {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }
    fn escalate_on_failure(&self) -> bool {
        true
    }
}

impl Approvable<ShellRequest> for ShellRuntime {
    type ApprovalKey = ApprovalKey;

    fn approval_keys(&self, req: &ShellRequest) -> Vec<Self::ApprovalKey> {
        vec![ApprovalKey {
            command: canonicalize_command_for_approval(&req.command),
            cwd: req.cwd.clone(),
            sandbox_permissions: req.sandbox_permissions,
            additional_permissions: req.additional_permissions.clone(),
        }]
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ShellRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let keys = self.approval_keys(req);
        let command = req.command.clone();
        let cwd = req.cwd.clone();
        let retry_reason = ctx.retry_reason.clone();
        let reason = retry_reason.clone().or_else(|| req.justification.clone());
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let guardian_review_id = ctx.guardian_review_id.clone();
        Box::pin(async move {
            if let Some(review_id) = guardian_review_id {
                return review_approval_request(
                    session,
                    turn,
                    review_id,
                    GuardianApprovalRequest::Shell {
                        id: call_id,
                        command,
                        cwd: cwd.clone(),
                        sandbox_permissions: req.sandbox_permissions,
                        additional_permissions: req.additional_permissions.clone(),
                        justification: req.justification.clone(),
                    },
                    retry_reason,
                )
                .await;
            }
            with_cached_approval(&session.services, "shell", keys, move || async move {
                let available_decisions = None;
                session
                    .request_command_approval(
                        turn,
                        call_id,
                        /*approval_id*/ None,
                        command,
                        cwd,
                        reason,
                        ctx.network_approval_context.clone(),
                        req.exec_approval_requirement
                            .proposed_execpolicy_amendment()
                            .cloned(),
                        req.additional_permissions.clone(),
                        available_decisions,
                    )
                    .await
            })
            .await
        })
    }

    fn exec_approval_requirement(&self, req: &ShellRequest) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn permission_request_payload(&self, req: &ShellRequest) -> Option<PermissionRequestPayload> {
        Some(PermissionRequestPayload::bash(
            req.hook_command.clone(),
            req.justification.clone(),
        ))
    }

    fn sandbox_mode_for_first_attempt(&self, req: &ShellRequest) -> SandboxOverride {
        sandbox_override_for_first_attempt(req.sandbox_permissions, &req.exec_approval_requirement)
    }
}

impl ToolRuntime<ShellRequest, ExecToolCallOutput> for ShellRuntime {
    fn sandbox_cwd<'a>(&self, req: &'a ShellRequest) -> Option<&'a AbsolutePathBuf> {
        Some(&req.cwd)
    }

    fn network_approval_spec(
        &self,
        req: &ShellRequest,
        ctx: &ToolCtx,
    ) -> Option<NetworkApprovalSpec> {
        let network =
            managed_network_for_sandbox_permissions(req.network.as_ref(), req.sandbox_permissions)?;
        Some(NetworkApprovalSpec {
            network: Some(network.clone()),
            mode: NetworkApprovalMode::Immediate,
            trigger: GuardianNetworkAccessTrigger {
                call_id: ctx.call_id.clone(),
                tool_name: ctx.tool_name.clone(),
                command: req.command.clone(),
                cwd: req.cwd.clone(),
                sandbox_permissions: req.sandbox_permissions,
                additional_permissions: req.additional_permissions.clone(),
                justification: req.justification.clone(),
                tty: None,
            },
            command: req.hook_command.clone(),
        })
    }

    async fn run(
        &mut self,
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let session_shell = ctx.session.user_shell();
        let managed_network =
            managed_network_for_sandbox_permissions(req.network.as_ref(), req.sandbox_permissions);
        let env = exec_env_for_sandbox_permissions(&req.env, req.sandbox_permissions);
        let command = if req.environment.is_remote() {
            req.command.clone()
        } else {
            maybe_wrap_shell_lc_with_snapshot(
                &req.command,
                session_shell.as_ref(),
                &req.cwd,
                &req.explicit_env_overrides,
                &env,
            )
        };
        let command = if matches!(session_shell.shell_type, ShellType::PowerShell) {
            prefix_powershell_script_with_utf8(&command)
        } else {
            command
        };

        if self.backend == ShellRuntimeBackend::ShellCommandZshFork && !req.environment.is_remote()
        {
            match zsh_fork_backend::maybe_run_shell_command(req, attempt, ctx, &command).await? {
                Some(out) => return Ok(out),
                None => {
                    tracing::warn!(
                        "ZshFork backend specified, but conditions for using it were not met, falling back to normal execution",
                    );
                }
            }
        }

        let command =
            build_sandbox_command(&command, &req.cwd, &env, req.additional_permissions.clone())?;
        let mut expiration: ExecExpiration = req.timeout_ms.into();
        if let Some(cancellation) = attempt.network_denial_cancellation_token.clone() {
            expiration = expiration.with_cancellation(cancellation);
        }
        let options = ExecOptions {
            expiration,
            capture_policy: ExecCapturePolicy::ShellTool,
        };
        let mut exec_env = attempt
            .env_for(command, options, managed_network)
            .map_err(|err| ToolError::Codex(err.into()))?;
        exec_env.exec_server_env_config = req.exec_server_env_config.clone();
        let out = if req.environment.is_remote() {
            run_remote_shell(req, exec_env, attempt, ctx).await?
        } else {
            execute_env(exec_env, Self::stdout_stream(ctx))
                .await
                .map_err(ToolError::Codex)?
        };
        Ok(out)
    }
}

fn exec_server_env_for_request(
    request: &ExecRequest,
) -> (
    Option<codex_exec_server::ExecEnvPolicy>,
    HashMap<String, String>,
) {
    if let Some(exec_server_env_config) = &request.exec_server_env_config {
        let env = exec_server_env_config.env_overlay(&request.env);
        (Some(exec_server_env_config.policy.clone()), env)
    } else {
        (None, request.env.clone())
    }
}

fn exec_server_params_for_request(request: &ExecRequest, call_id: &str) -> ExecServerParams {
    let (env_policy, env) = exec_server_env_for_request(request);
    ExecServerParams {
        process_id: ProcessId::from(call_id.to_string()),
        argv: request.command.clone(),
        cwd: request.cwd.to_path_buf(),
        env_policy,
        env,
        tty: false,
        pipe_stdin: false,
        arg0: request.arg0.clone(),
    }
}

fn append_capped(dst: &mut Vec<u8>, src: &[u8]) {
    if dst.len() >= DEFAULT_OUTPUT_BYTES_CAP {
        return;
    }
    let remaining = DEFAULT_OUTPUT_BYTES_CAP.saturating_sub(dst.len());
    dst.extend_from_slice(&src[..src.len().min(remaining)]);
}

async fn emit_output_delta(
    stdout_stream: &StdoutStream,
    call_stream: ExecOutputStream,
    chunk: Vec<u8>,
) {
    let stream = match call_stream {
        ExecOutputStream::Stdout | ExecOutputStream::Pty => {
            codex_protocol::protocol::ExecOutputStream::Stdout
        }
        ExecOutputStream::Stderr => codex_protocol::protocol::ExecOutputStream::Stderr,
    };
    let msg = EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
        call_id: stdout_stream.call_id.clone(),
        stream,
        chunk,
    });
    let event = Event {
        id: stdout_stream.sub_id.clone(),
        msg,
    };
    let _ = stdout_stream.tx_event.send(event).await;
}

fn shell_output(
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    aggregated_output: Vec<u8>,
    exit_code: i32,
    duration: Duration,
    timed_out: bool,
) -> ExecToolCallOutput {
    let stdout = StreamOutput {
        text: stdout,
        truncated_after_lines: None,
    };
    let stderr = StreamOutput {
        text: stderr,
        truncated_after_lines: None,
    };
    ExecToolCallOutput {
        exit_code,
        stdout: stdout.from_utf8_lossy(),
        stderr: stderr.from_utf8_lossy(),
        aggregated_output: StreamOutput {
            text: codex_protocol::exec_output::bytes_to_string_smart(&aggregated_output),
            truncated_after_lines: None,
        },
        duration,
        timed_out,
    }
}

async fn run_remote_shell(
    req: &ShellRequest,
    exec_env: ExecRequest,
    attempt: &SandboxAttempt<'_>,
    ctx: &ToolCtx,
) -> Result<ExecToolCallOutput, ToolError> {
    let start = Instant::now();
    let expiration: ExecExpiration = req.timeout_ms.into();
    let timeout = expiration.timeout_ms().map(Duration::from_millis);
    let deadline = timeout.map(|timeout| start + timeout);
    let started = req
        .environment
        .get_exec_backend()
        .start(exec_server_params_for_request(&exec_env, &ctx.call_id))
        .await
        .map_err(|err| ToolError::Rejected(err.to_string()))?;
    let stdout_stream = ShellRuntime::stdout_stream(ctx);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut aggregated_output = Vec::new();
    let mut after_seq = None;
    let mut exit_code = None;

    loop {
        if let Some(cancellation) = attempt.network_denial_cancellation_token.as_ref()
            && cancellation.is_cancelled()
        {
            let _ = started.process.terminate().await;
            return Err(ToolError::Rejected(
                "Network access was denied by the Codex sandbox network proxy.".to_string(),
            ));
        }
        let wait_ms = deadline
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
            .map(|remaining| remaining.min(Duration::from_millis(100)).as_millis() as u64)
            .unwrap_or(100);
        if wait_ms == 0 {
            let _ = started.process.terminate().await;
            let output = shell_output(
                stdout,
                stderr,
                aggregated_output,
                124,
                start.elapsed(),
                /*timed_out*/ true,
            );
            return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Timeout {
                output: Box::new(output),
            })));
        }

        let response = started
            .process
            .read(after_seq, /*max_bytes*/ None, Some(wait_ms))
            .await
            .map_err(|err| ToolError::Rejected(err.to_string()))?;
        for chunk in response.chunks {
            let stream = chunk.stream;
            let bytes = chunk.chunk.into_inner();
            if let Some(stdout_stream) = stdout_stream.as_ref() {
                emit_output_delta(stdout_stream, stream, bytes.clone()).await;
            }
            match stream {
                ExecOutputStream::Stdout | ExecOutputStream::Pty => {
                    append_capped(&mut stdout, &bytes);
                    append_capped(&mut aggregated_output, &bytes);
                }
                ExecOutputStream::Stderr => {
                    append_capped(&mut stderr, &bytes);
                    append_capped(&mut aggregated_output, &bytes);
                }
            }
        }
        if let Some(failure) = response.failure {
            return Err(ToolError::Rejected(failure));
        }
        if response.exited {
            exit_code = response.exit_code;
        }
        if response.closed {
            break;
        }
        after_seq = response.next_seq.checked_sub(1);
    }

    let output = shell_output(
        stdout,
        stderr,
        aggregated_output,
        exit_code.unwrap_or(-1),
        start.elapsed(),
        /*timed_out*/ false,
    );
    if is_likely_sandbox_denied(exec_env.sandbox, &output) {
        return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
            output: Box::new(output),
            network_policy_decision: None,
        })));
    }
    Ok(output)
}
