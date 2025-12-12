use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::function_tool::FunctionCallError;
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

pub struct UnifiedExecHandler;

#[derive(Debug, Deserialize)]
struct ExecCommandArgs {
    cmd: String,
    #[serde(default = "default_shell")]
    shell: String,
    #[serde(default = "default_login")]
    login: bool,
    #[serde(default)]
    yield_time_ms: Option<u64>,
    #[serde(default)]
    max_output_tokens: Option<usize>,
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

                let event_ctx = ToolEventCtx::new(
                    context.session.as_ref(),
                    context.turn.as_ref(),
                    &context.call_id,
                    None,
                );
                let emitter =
                    ToolEmitter::unified_exec(args.cmd.clone(), context.turn.cwd.clone(), true);
                emitter.emit(event_ctx, ToolEventStage::Begin).await;

                manager
                    .exec_command(
                        ExecCommandRequest {
                            command: &args.cmd,
                            shell: &args.shell,
                            login: args.login,
                            yield_time_ms: args.yield_time_ms,
                            max_output_tokens: args.max_output_tokens,
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

        let content = serialize_response(&response).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to serialize unified exec output: {err:?}"
            ))
        })?;

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(true),
        })
    }
}

#[derive(Serialize)]
struct SerializedUnifiedExecResponse<'a> {
    chunk_id: &'a str,
    wall_time_seconds: f64,
    output: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_token_count: Option<usize>,
}

fn serialize_response(response: &UnifiedExecResponse) -> Result<String, serde_json::Error> {
    let payload = SerializedUnifiedExecResponse {
        chunk_id: &response.chunk_id,
        wall_time_seconds: duration_to_seconds(response.wall_time),
        output: &response.output,
        session_id: response.session_id,
        exit_code: response.exit_code,
        original_token_count: response.original_token_count,
    };

    serde_json::to_string(&payload)
}

fn duration_to_seconds(duration: Duration) -> f64 {
    duration.as_secs_f64()
}
