use std::path::PathBuf;

use crate::function_tool::FunctionCallError;
use crate::is_safe_command::is_known_safe_command;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandOutputDeltaEvent;
use crate::protocol::ExecOutputStream;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventStage;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::unified_exec::ExecCommandRequest;
use crate::unified_exec::UnifiedExecContext;
use crate::unified_exec::UnifiedExecResponse;
use crate::unified_exec::UnifiedExecSessionManager;
use crate::unified_exec::WriteStdinRequest;
use async_trait::async_trait;
use serde::Deserialize;

pub struct UnifiedExecHandler;

#[derive(Debug, Deserialize)]
struct ExecCommandArgs {
    cmd: String,
    #[serde(default)]
    workdir: Option<String>,
    #[serde(default = "default_shell")]
    shell: String,
    #[serde(default = "default_login")]
    login: bool,
    #[serde(default)]
    yield_time_ms: Option<u64>,
    #[serde(default)]
    max_output_tokens: Option<usize>,
    #[serde(default)]
    with_escalated_permissions: Option<bool>,
    #[serde(default)]
    justification: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WriteStdinArgs {
    session_id: i32,
    #[serde(default)]
    chars: String,
    #[serde(default)]
    yield_time_ms: Option<u64>,
    #[serde(default)]
    max_output_tokens: Option<usize>,
}

fn default_shell() -> String {
    "/bin/bash".to_string()
}

fn default_login() -> bool {
    true
}

#[async_trait]
impl ToolHandler for UnifiedExecHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            payload,
            ToolPayload::Function { .. } | ToolPayload::UnifiedExec { .. }
        )
    }

    fn is_mutating(&self, invocation: &ToolInvocation) -> bool {
        let (ToolPayload::Function { arguments } | ToolPayload::UnifiedExec { arguments }) =
            &invocation.payload
        else {
            return true;
        };

        let Ok(params) = serde_json::from_str::<ExecCommandArgs>(arguments) else {
            return true;
        };
        !is_known_safe_command(&["bash".to_string(), "-lc".to_string(), params.cmd])
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            ToolPayload::UnifiedExec { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "unified_exec handler received unsupported payload".to_string(),
                ));
            }
        };

        let manager: &UnifiedExecSessionManager = &session.services.unified_exec_manager;
        let context = UnifiedExecContext::new(session.clone(), turn.clone(), call_id.clone());

        let response = match tool_name.as_str() {
            "exec_command" => {
                let args: ExecCommandArgs = serde_json::from_str(&arguments).map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to parse exec_command arguments: {err:?}"
                    ))
                })?;
                let ExecCommandArgs {
                    cmd,
                    workdir,
                    shell,
                    login,
                    yield_time_ms,
                    max_output_tokens,
                    with_escalated_permissions,
                    justification,
                } = args;

                if with_escalated_permissions.unwrap_or(false)
                    && !matches!(
                        context.turn.approval_policy,
                        codex_protocol::protocol::AskForApproval::OnRequest
                    )
                {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "approval policy is {policy:?}; reject command — you cannot ask for escalated permissions if the approval policy is {policy:?}",
                        policy = context.turn.approval_policy
                    )));
                }

                let workdir = workdir
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from);
                let cwd = workdir.clone().unwrap_or_else(|| context.turn.cwd.clone());

                let event_ctx = ToolEventCtx::new(
                    context.session.as_ref(),
                    context.turn.as_ref(),
                    &context.call_id,
                    None,
                );
                let emitter = ToolEmitter::unified_exec(cmd.clone(), cwd.clone(), true);
                emitter.emit(event_ctx, ToolEventStage::Begin).await;

                manager
                    .exec_command(
                        ExecCommandRequest {
                            command: &cmd,
                            shell: &shell,
                            login,
                            yield_time_ms,
                            max_output_tokens,
                            workdir,
                            with_escalated_permissions,
                            justification,
                        },
                        &context,
                    )
                    .await
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!("exec_command failed: {err:?}"))
                    })?
            }
            "write_stdin" => {
                let args: WriteStdinArgs = serde_json::from_str(&arguments).map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to parse write_stdin arguments: {err:?}"
                    ))
                })?;
                manager
                    .write_stdin(WriteStdinRequest {
                        session_id: args.session_id,
                        input: &args.chars,
                        yield_time_ms: args.yield_time_ms,
                        max_output_tokens: args.max_output_tokens,
                    })
                    .await
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!("write_stdin failed: {err:?}"))
                    })?
            }
            other => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "unsupported unified exec function {other}"
                )));
            }
        };

        // Emit a delta event with the chunk of output we just produced, if any.
        if !response.output.is_empty() {
            let delta = ExecCommandOutputDeltaEvent {
                call_id: response.event_call_id.clone(),
                stream: ExecOutputStream::Stdout,
                chunk: response.output.as_bytes().to_vec(),
            };
            session
                .send_event(turn.as_ref(), EventMsg::ExecCommandOutputDelta(delta))
                .await;
        }

        let content = format_response(&response);

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(true),
        })
    }
}

fn format_response(response: &UnifiedExecResponse) -> String {
    let mut sections = Vec::new();

    if !response.chunk_id.is_empty() {
        sections.push(format!("Chunk ID: {}", response.chunk_id));
    }

    let wall_time_seconds = response.wall_time.as_secs_f64();
    sections.push(format!("Wall time: {wall_time_seconds:.4} seconds"));

    if let Some(exit_code) = response.exit_code {
        sections.push(format!("Process exited with code {exit_code}"));
    }

    if let Some(session_id) = response.session_id {
        sections.push(format!("Process running with session ID {session_id}"));
    }

    if let Some(original_token_count) = response.original_token_count {
        sections.push(format!("Original token count: {original_token_count}"));
    }

    sections.push("Output:".to_string());
    sections.push(response.output.clone());

    sections.join("\n")
}
