use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

use super::ExecContext;
use super::PUBLIC_TOOL_NAME;
use super::build_enabled_tools;
use super::handle_runtime_response;

pub struct CodeModeExecuteHandler;

impl CodeModeExecuteHandler {
    async fn execute(
        &self,
        session: std::sync::Arc<crate::codex::Session>,
        turn: std::sync::Arc<crate::codex::TurnContext>,
        call_id: String,
        code: String,
    ) -> Result<FunctionToolOutput, FunctionCallError> {
        let args =
            codex_code_mode::parse_exec_source(&code).map_err(FunctionCallError::RespondToModel)?;
        let exec = ExecContext { session, turn };
        let enabled_tools = build_enabled_tools(&exec).await;
        let stored_values = exec
            .session
            .services
            .code_mode_service
            .stored_values()
            .await;
        let started_at = std::time::Instant::now();
        let response = exec
            .session
            .services
            .code_mode_service
            .execute(codex_code_mode::ExecuteRequest {
                tool_call_id: call_id,
                enabled_tools,
                source: args.code,
                stored_values,
                yield_time_ms: args.yield_time_ms,
                max_output_tokens: args.max_output_tokens,
            })
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        handle_runtime_response(&exec, response, args.max_output_tokens, started_at)
            .await
            .map_err(FunctionCallError::RespondToModel)
    }
}

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
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        match payload {
            ToolPayload::Custom { input } if tool_name.name() == PUBLIC_TOOL_NAME => {
                self.execute(session, turn, call_id, input).await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} expects raw JavaScript source text"
            ))),
        }
    }
}
