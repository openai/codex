use async_trait::async_trait;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::CodeModeSessionProgress;
use super::DEFAULT_WAIT_YIELD_TIME_MS;
use super::ExecContext;
use super::PUBLIC_TOOL_NAME;
use super::WAIT_TOOL_NAME;
use super::handle_node_message;
use super::protocol::HostToNodeMessage;

pub struct CodeModeWaitHandler;

#[derive(Debug, Deserialize)]
struct ExecWaitArgs {
    session_id: i32,
    #[serde(default = "default_wait_yield_time_ms")]
    yield_time_ms: u64,
    #[serde(default)]
    max_tokens: Option<usize>,
    #[serde(default)]
    terminate: bool,
}

fn default_wait_yield_time_ms() -> u64 {
    DEFAULT_WAIT_YIELD_TIME_MS
}

fn parse_arguments<T>(arguments: &str) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err}"))
    })
}

#[async_trait]
impl ToolHandler for CodeModeWaitHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            tool_name,
            payload,
            ..
        } = invocation;

        match payload {
            ToolPayload::Function { arguments } if tool_name == WAIT_TOOL_NAME => {
                let args: ExecWaitArgs = parse_arguments(&arguments)?;
                let exec = ExecContext {
                    session,
                    turn,
                    tracker,
                };
                let request_id = exec
                    .session
                    .services
                    .code_mode_service
                    .allocate_request_id()
                    .await;
                let started_at = std::time::Instant::now();
                let message = if args.terminate {
                    HostToNodeMessage::Terminate {
                        request_id: request_id.clone(),
                        session_id: args.session_id,
                    }
                } else {
                    HostToNodeMessage::Poll {
                        request_id: request_id.clone(),
                        session_id: args.session_id,
                        yield_time_ms: args.yield_time_ms,
                    }
                };
                let process_slot = exec
                    .session
                    .services
                    .code_mode_service
                    .ensure_started()
                    .await
                    .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
                let result = {
                    let mut process_slot = process_slot;
                    let Some(process) = process_slot.as_mut() else {
                        return Err(FunctionCallError::RespondToModel(format!(
                            "{PUBLIC_TOOL_NAME} runner failed to start"
                        )));
                    };
                    if !matches!(process.has_exited(), Ok(false)) {
                        return Err(FunctionCallError::RespondToModel(format!(
                            "{PUBLIC_TOOL_NAME} runner failed to start"
                        )));
                    }
                    let message = process
                        .send(&request_id, &message)
                        .await
                        .map_err(|err| err.to_string());
                    let message = match message {
                        Ok(message) => message,
                        Err(error) => return Err(FunctionCallError::RespondToModel(error)),
                    };
                    handle_node_message(
                        &exec,
                        args.session_id,
                        message,
                        Some(args.max_tokens),
                        started_at,
                    )
                    .await
                };
                match result {
                    Ok(CodeModeSessionProgress::Finished(output))
                    | Ok(CodeModeSessionProgress::Yielded { output }) => Ok(output),
                    Err(error) => Err(FunctionCallError::RespondToModel(error)),
                }
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{WAIT_TOOL_NAME} expects JSON arguments"
            ))),
        }
    }
}
