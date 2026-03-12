use async_trait::async_trait;
use serde::Deserialize;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::CodeModeSessionProgress;
use super::DEFAULT_WAIT_YIELD_TIME_MS;
use super::ExecContext;
use super::PUBLIC_TOOL_NAME;
use super::WAIT_TOOL_NAME;
use super::build_enabled_tools;
use super::handle_node_message;
use super::protocol::HostToNodeMessage;
use super::protocol::build_source;

pub struct CodeModeHandler;
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

impl CodeModeHandler {
    async fn execute(
        &self,
        session: std::sync::Arc<Session>,
        turn: std::sync::Arc<TurnContext>,
        tracker: SharedTurnDiffTracker,
        code: String,
    ) -> Result<FunctionToolOutput, FunctionCallError> {
        let exec = ExecContext {
            session,
            turn,
            tracker,
        };
        let enabled_tools = build_enabled_tools(&exec).await;
        let service = &exec.session.services.code_mode_service;
        let stored_values = service.stored_values().await;
        let source =
            build_source(&code, &enabled_tools).map_err(FunctionCallError::RespondToModel)?;
        let session_id = service.allocate_session_id().await;
        let request_id = service.allocate_request_id().await;
        let process_slot = service
            .ensure_started()
            .await
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
        let started_at = std::time::Instant::now();
        let message = HostToNodeMessage::Start {
            request_id: request_id.clone(),
            session_id,
            enabled_tools,
            stored_values,
            source,
        };
        let result = {
            let mut process_slot = process_slot;
            let Some(process) = process_slot.as_mut() else {
                return Err(FunctionCallError::RespondToModel(format!(
                    "{PUBLIC_TOOL_NAME} runner failed to start"
                )));
            };
            let message = process
                .send(&request_id, &message)
                .await
                .map_err(|err| err.to_string());
            let message = match message {
                Ok(message) => message,
                Err(error) => return Err(FunctionCallError::RespondToModel(error)),
            };
            handle_node_message(&exec, session_id, message, None, started_at).await
        };
        match result {
            Ok(CodeModeSessionProgress::Finished(output))
            | Ok(CodeModeSessionProgress::Yielded { output }) => Ok(output),
            Err(error) => Err(FunctionCallError::RespondToModel(error)),
        }
    }
}

impl CodeModeWaitHandler {
    async fn wait(
        &self,
        session: std::sync::Arc<Session>,
        turn: std::sync::Arc<TurnContext>,
        tracker: SharedTurnDiffTracker,
        session_id: i32,
        yield_time_ms: u64,
        max_output_tokens: Option<usize>,
        terminate: bool,
    ) -> Result<FunctionToolOutput, FunctionCallError> {
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
        let message = if terminate {
            HostToNodeMessage::Terminate {
                request_id: request_id.clone(),
                session_id,
            }
        } else {
            HostToNodeMessage::Poll {
                request_id: request_id.clone(),
                session_id,
                yield_time_ms,
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
                session_id,
                message,
                Some(max_output_tokens),
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
}

#[async_trait]
impl ToolHandler for CodeModeHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Custom { .. })
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
            ToolPayload::Custom { input } if tool_name == PUBLIC_TOOL_NAME => {
                self.execute(session, turn, tracker, input).await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} expects raw JavaScript source text"
            ))),
        }
    }
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
                self.wait(
                    session,
                    turn,
                    tracker,
                    args.session_id,
                    args.yield_time_ms,
                    args.max_tokens,
                    args.terminate,
                )
                .await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{WAIT_TOOL_NAME} expects JSON arguments"
            ))),
        }
    }
}
