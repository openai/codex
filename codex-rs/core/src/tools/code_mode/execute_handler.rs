use async_trait::async_trait;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::CodeModeSessionProgress;
use super::ExecContext;
use super::PUBLIC_TOOL_NAME;
use super::build_enabled_tools;
use super::handle_node_message;
use super::protocol::HostToNodeMessage;
use super::protocol::build_source;

pub struct CodeModeExecuteHandler;

impl CodeModeExecuteHandler {
    async fn execute(
        &self,
        session: std::sync::Arc<Session>,
        turn: std::sync::Arc<TurnContext>,
        code: String,
    ) -> Result<FunctionToolOutput, FunctionCallError> {
        let exec = ExecContext { session, turn };
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

#[async_trait]
impl ToolHandler for CodeModeExecuteHandler {
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
            tool_name,
            payload,
            ..
        } = invocation;

        match payload {
            ToolPayload::Custom { input } if tool_name == PUBLIC_TOOL_NAME => {
                self.execute(session, turn, input).await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} expects raw JavaScript source text"
            ))),
        }
    }
}
