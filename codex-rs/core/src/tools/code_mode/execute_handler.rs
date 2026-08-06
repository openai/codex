use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use std::sync::Arc;

use super::ExecContext;
use super::PUBLIC_TOOL_NAME;
use super::handle_runtime_response;
use super::is_exec_tool_name;
use super::telemetry::CodeModeToolCallGuard;

type CodeModeNestedTool = (Arc<ToolSpec>, Option<Arc<dyn CoreToolRuntime>>);

pub struct CodeModeExecuteHandler {
    spec: ToolSpec,
    nested_tool_specs: Vec<CodeModeNestedTool>,
}

impl CodeModeExecuteHandler {
    pub(crate) fn new(spec: ToolSpec, nested_tool_specs: Vec<CodeModeNestedTool>) -> Self {
        Self {
            spec,
            nested_tool_specs,
        }
    }

    async fn execute(
        &self,
        session: std::sync::Arc<crate::session::session::Session>,
        turn: std::sync::Arc<crate::session::turn_context::TurnContext>,
        call_id: String,
        code: String,
        telemetry: &mut CodeModeToolCallGuard,
    ) -> Result<FunctionToolOutput, FunctionCallError> {
        let args =
            codex_code_mode::parse_exec_source(&code).map_err(FunctionCallError::RespondToModel)?;
        let exec = ExecContext { session, turn };
        let mut enabled_tools = Vec::with_capacity(self.nested_tool_specs.len());
        for (spec, cached_runtime) in &self.nested_tool_specs {
            if let Some(cached_definitions) = cached_runtime
                .as_ref()
                .and_then(|runtime| runtime.cached_code_mode_definitions())
            {
                enabled_tools.extend_from_slice(cached_definitions);
                continue;
            }

            let definitions =
                codex_tools::collect_code_mode_tool_definitions(std::iter::once(spec.as_ref()));
            enabled_tools.extend(definitions.into_iter().map(|mut definition| {
                definition.input_schema = None;
                definition.output_schema = None;
                definition
            }));
        }
        enabled_tools.sort_by(|left, right| left.name.cmp(&right.name));
        enabled_tools.dedup_by(|left, right| left.name == right.name);
        let started_at = std::time::Instant::now();
        let started_cell = exec
            .session
            .services
            .code_mode_service
            .execute(codex_code_mode::ExecuteRequest {
                tool_call_id: call_id.clone(),
                enabled_tools,
                source: args.code.clone(),
                yield_time_ms: args.yield_time_ms,
                max_output_tokens: args.max_output_tokens,
            })
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        let cell_id = started_cell.cell_id.clone();
        telemetry.cell_id = Some(cell_id.to_string());
        exec.session
            .services
            .analytics_events_client
            .track_code_mode_tool_call(codex_analytics::CodeModeToolCallFact::CellStarted {
                thread_id: exec.session.thread_id.to_string(),
                turn_id: exec.turn.sub_id.clone(),
                call_id: call_id.clone(),
                cell_id: cell_id.to_string(),
            });
        if let Some(executed_tool_calls) = exec.session.services.executed_tool_calls.as_ref() {
            executed_tool_calls.register_cell(&cell_id, &call_id);
        }
        let runtime_cell_id = cell_id.to_string();
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
        exec.session
            .services
            .code_mode_service
            .mark_cell_ready_for_dispatch(&cell_id);
        let response = started_cell
            .initial_response()
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
            exec.session
                .services
                .code_mode_service
                .finish_cell_dispatch(&cell_id);
            exec.session
                .services
                .analytics_events_client
                .track_code_mode_tool_call(codex_analytics::CodeModeToolCallFact::CellClosed {
                    thread_id: exec.session.thread_id.to_string(),
                    turn_id: exec.turn.sub_id.clone(),
                    cell_id: cell_id.to_string(),
                });
        }
        exec.session.services.elicitations.wait_until_clear().await;
        handle_runtime_response(&exec, response, args.max_output_tokens, started_at)
            .await
            .map_err(FunctionCallError::RespondToModel)
    }
}

impl ToolExecutor<ToolInvocation> for CodeModeExecuteHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(PUBLIC_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(invocation))
    }
}

impl CodeModeExecuteHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let mut telemetry = CodeModeToolCallGuard::new(
            session.services.analytics_events_client.clone(),
            session.thread_id.to_string(),
            turn.sub_id.clone(),
            call_id.clone(),
            PUBLIC_TOOL_NAME,
        );
        let result = match payload {
            ToolPayload::Custom { input } if is_exec_tool_name(&tool_name) => self
                .execute(session, turn, call_id, input, &mut telemetry)
                .await
                .map(boxed_tool_output),
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} expects raw JavaScript source text"
            ))),
        };
        telemetry.finish(
            result
                .as_ref()
                .is_ok_and(|output| output.success_for_logging()),
        );
        result
    }
}

impl CoreToolRuntime for CodeModeExecuteHandler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Custom { .. })
    }
}
