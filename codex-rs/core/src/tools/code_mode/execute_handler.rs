use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use serde_json::json;

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
        // Allocate the runtime cell id before starting V8. Nested tools can be
        // requested before the model-visible `exec` item has been reduced, so
        // the trace needs this explicit bridge up front.
        let runtime_cell_id = exec.session.services.code_mode_service.allocate_cell_id();
        let invocation_payload = codex_trace::write_payload(
            "code_cell_invocation",
            &json!({
                "cell_id": runtime_cell_id,
                "model_visible_call_id": call_id,
                "source_js": args.code,
                "yield_time_ms": args.yield_time_ms,
                "max_output_tokens": args.max_output_tokens,
            }),
        );
        emit_code_cell_started(
            &exec,
            &runtime_cell_id,
            &call_id,
            &args.code,
            invocation_payload.as_ref(),
        );
        let started_at = std::time::Instant::now();
        let response = match exec
            .session
            .services
            .code_mode_service
            .execute(codex_code_mode::ExecuteRequest {
                cell_id: Some(runtime_cell_id.clone()),
                tool_call_id: call_id,
                enabled_tools,
                source: args.code,
                stored_values,
                yield_time_ms: args.yield_time_ms,
                max_output_tokens: args.max_output_tokens,
            })
            .await
        {
            Ok(response) => response,
            Err(err) => {
                let result_payload = codex_trace::write_payload(
                    "code_cell_result",
                    &json!({ "cell_id": runtime_cell_id, "error_text": err }),
                );
                emit_code_cell_failed(&exec, &runtime_cell_id, result_payload.as_ref());
                return Err(FunctionCallError::RespondToModel(err));
            }
        };
        let result_payload =
            codex_trace::write_payload("code_cell_result", &code_cell_response_payload(&response));
        // A yielded response is the end of the model-visible custom `exec`
        // call, not the end of the runtime cell. Emit it anyway so the trace
        // can distinguish "the model got a cell id" from "the JS finished".
        emit_code_cell_ended(
            &exec,
            &runtime_cell_id,
            &response,
            /*model_visible_wait_call_id*/ None,
            result_payload.as_ref(),
        );
        handle_runtime_response(&exec, response, args.max_output_tokens, started_at)
            .await
            .map_err(FunctionCallError::RespondToModel)
    }
}

fn emit_code_cell_started(
    exec: &ExecContext,
    runtime_cell_id: &str,
    model_visible_call_id: &str,
    source_js: &str,
    invocation_payload: Option<&codex_trace::RawPayloadRef>,
) {
    tracing::event!(
        target: codex_otel::OTEL_TRACE_SAFE_TARGET,
        tracing::Level::INFO,
        event.name = %"codex.code_cell.started",
        thread.id = %exec.session.conversation_id,
        turn.id = %exec.turn.sub_id,
        code_cell.runtime_id = %runtime_cell_id,
        model_visible_call.id = %model_visible_call_id,
        code_cell.source_js = %source_js,
        raw_payload.invocation.id = %invocation_payload.map(|payload| payload.raw_payload_id.as_str()).unwrap_or(""),
        raw_payload.invocation.path = %invocation_payload.map(|payload| payload.path.as_str()).unwrap_or(""),
        raw_payload.invocation.kind = %invocation_payload.map(|payload| payload.kind.as_str()).unwrap_or(""),
    );
}

pub(super) fn emit_code_cell_ended(
    exec: &ExecContext,
    runtime_cell_id: &str,
    response: &codex_code_mode::RuntimeResponse,
    model_visible_wait_call_id: Option<&str>,
    result_payload: Option<&codex_trace::RawPayloadRef>,
) {
    tracing::event!(
        target: codex_otel::OTEL_TRACE_SAFE_TARGET,
        tracing::Level::INFO,
        event.name = %"codex.code_cell.ended",
        thread.id = %exec.session.conversation_id,
        turn.id = %exec.turn.sub_id,
        code_cell.runtime_id = %runtime_cell_id,
        status = %code_cell_status(response),
        model_visible_wait_call.id = %model_visible_wait_call_id.unwrap_or(""),
        raw_payload.result.id = %result_payload.map(|payload| payload.raw_payload_id.as_str()).unwrap_or(""),
        raw_payload.result.path = %result_payload.map(|payload| payload.path.as_str()).unwrap_or(""),
        raw_payload.result.kind = %result_payload.map(|payload| payload.kind.as_str()).unwrap_or(""),
    );
}

fn emit_code_cell_failed(
    exec: &ExecContext,
    runtime_cell_id: &str,
    result_payload: Option<&codex_trace::RawPayloadRef>,
) {
    tracing::event!(
        target: codex_otel::OTEL_TRACE_SAFE_TARGET,
        tracing::Level::INFO,
        event.name = %"codex.code_cell.ended",
        thread.id = %exec.session.conversation_id,
        turn.id = %exec.turn.sub_id,
        code_cell.runtime_id = %runtime_cell_id,
        status = %"failed",
        raw_payload.result.id = %result_payload.map(|payload| payload.raw_payload_id.as_str()).unwrap_or(""),
        raw_payload.result.path = %result_payload.map(|payload| payload.path.as_str()).unwrap_or(""),
        raw_payload.result.kind = %result_payload.map(|payload| payload.kind.as_str()).unwrap_or(""),
    );
}

fn code_cell_status(response: &codex_code_mode::RuntimeResponse) -> &'static str {
    match response {
        codex_code_mode::RuntimeResponse::Yielded { .. } => "yielded",
        codex_code_mode::RuntimeResponse::Terminated { .. } => "terminated",
        codex_code_mode::RuntimeResponse::Result { error_text, .. } => {
            if error_text.is_some() {
                "failed"
            } else {
                "completed"
            }
        }
    }
}

fn code_cell_response_payload(response: &codex_code_mode::RuntimeResponse) -> serde_json::Value {
    match response {
        codex_code_mode::RuntimeResponse::Yielded {
            cell_id,
            content_items,
        } => json!({
            "cell_id": cell_id,
            "status": "yielded",
            "content_items": content_items,
        }),
        codex_code_mode::RuntimeResponse::Terminated {
            cell_id,
            content_items,
        } => json!({
            "cell_id": cell_id,
            "status": "terminated",
            "content_items": content_items,
        }),
        codex_code_mode::RuntimeResponse::Result {
            cell_id,
            content_items,
            stored_values,
            error_text,
        } => json!({
            "cell_id": cell_id,
            "status": if error_text.is_some() { "failed" } else { "completed" },
            "content_items": content_items,
            "stored_values": stored_values,
            "error_text": error_text,
        }),
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

    fn uses_first_class_trace_object(&self, invocation: &ToolInvocation) -> bool {
        // Code-mode `exec` is represented by the first-class CodeCell runtime
        // lifecycle. Emitting the generic dispatch ToolCall as well would give
        // viewers two top-level objects for the same model-visible JS cell.
        //
        // Only suppress the generic ToolCall once parsing is known to succeed.
        // Parse errors return before a CodeCell id exists, so those failures
        // still need the ordinary tool trace object.
        invocation.tool_name.namespace.is_none()
            && invocation.tool_name.name.as_str() == PUBLIC_TOOL_NAME
            && matches!(
                &invocation.payload,
                ToolPayload::Custom { input }
                    if codex_code_mode::parse_exec_source(input).is_ok()
            )
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
            ToolPayload::Custom { input }
                if tool_name.namespace.is_none() && tool_name.name.as_str() == PUBLIC_TOOL_NAME =>
            {
                self.execute(session, turn, call_id, input).await
            }
            _ => Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} expects raw JavaScript source text"
            ))),
        }
    }
}
