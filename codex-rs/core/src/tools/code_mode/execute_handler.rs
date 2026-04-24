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
use super::is_exec_tool_name;

pub struct CodeModeExecuteHandler;

impl CodeModeExecuteHandler {
    async fn execute(
        &self,
        session: std::sync::Arc<crate::session::session::Session>,
        turn: std::sync::Arc<crate::session::turn_context::TurnContext>,
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
        // Allocate before starting V8 so the trace can create the parent
        // CodeCell before model-authored JavaScript issues nested tool calls.
        let runtime_cell_id = exec.session.services.code_mode_service.allocate_cell_id();
        let code_cell_trace = exec
            .session
            .services
            .rollout_thread_trace
            .start_code_cell_trace(
                exec.turn.sub_id.as_str(),
                runtime_cell_id.as_str(),
                call_id.as_str(),
                args.code.as_str(),
            );
        let started_at = std::time::Instant::now();
        let response = exec
            .session
            .services
            .code_mode_service
            .execute(codex_code_mode::ExecuteRequest {
                cell_id: runtime_cell_id,
                tool_call_id: call_id,
                enabled_tools,
                source: args.code,
                stored_values,
                yield_time_ms: args.yield_time_ms,
                max_output_tokens: args.max_output_tokens,
            })
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        // Record the raw runtime boundary. The model-visible custom-tool output
        // is produced by `handle_runtime_response` and later linked through
        // `CodeCell.output_item_ids` in the reduced trace.
        code_cell_trace.record_initial_response(&response);
        // Yielded cells keep running, so terminal lifecycle is only emitted
        // here when the first response also ended the runtime.
        if !matches!(response, codex_code_mode::RuntimeResponse::Yielded { .. }) {
            code_cell_trace.record_ended(&response);
        }
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
            ToolPayload::Custom { input } if is_exec_tool_name(&tool_name) => {
                self.execute(session, turn, call_id, input).await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} expects raw JavaScript source text"
            ))),
        }
    }
}
