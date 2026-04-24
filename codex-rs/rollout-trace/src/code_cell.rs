//! Hot-path helpers for recording code-mode runtime cell lifecycles.
//!
//! The public `exec` tool is reduced as a first-class `CodeCell` instead of a
//! generic tool call. This module keeps the runtime response serialization and
//! lifecycle event policy inside the trace crate while core carries a compact,
//! no-op capable handle through execution and waits.

use std::sync::Arc;

use codex_code_mode::RuntimeResponse;
use serde::Serialize;
use tracing::warn;

use crate::model::AgentThreadId;
use crate::model::CodeCellRuntimeStatus;
use crate::model::CodexTurnId;
use crate::model::ModelVisibleCallId;
use crate::payload::RawPayloadKind;
use crate::payload::RawPayloadRef;
use crate::raw_event::RawTraceEventContext;
use crate::raw_event::RawTraceEventPayload;
use crate::writer::TraceWriter;

/// No-op capable trace handle for one code-mode runtime cell.
#[derive(Clone, Debug)]
pub struct CodeCellTraceContext {
    state: CodeCellTraceContextState,
}

#[derive(Clone, Debug)]
enum CodeCellTraceContextState {
    Disabled,
    Enabled(EnabledCodeCellTraceContext),
}

#[derive(Clone, Debug)]
struct EnabledCodeCellTraceContext {
    writer: Arc<TraceWriter>,
    thread_id: AgentThreadId,
    codex_turn_id: CodexTurnId,
    runtime_cell_id: String,
}

/// Raw code-mode response captured at the runtime boundary.
///
/// This is not the model-visible custom-tool output. The reducer links that
/// output through `CodeCell.output_item_ids` once the conversation item appears.
/// Keeping the raw runtime payload here preserves stored-value and lifecycle
/// evidence without duplicating the model-facing transcript.
#[derive(Serialize)]
struct CodeCellResponseTracePayload<'a> {
    response: &'a RuntimeResponse,
}

impl CodeCellTraceContext {
    /// Builds a context that accepts trace calls and records nothing.
    pub(crate) fn disabled() -> Self {
        Self {
            state: CodeCellTraceContextState::Disabled,
        }
    }

    /// Builds a context for an already-known code-mode runtime cell.
    pub(crate) fn enabled(
        writer: Arc<TraceWriter>,
        thread_id: impl Into<AgentThreadId>,
        codex_turn_id: impl Into<CodexTurnId>,
        runtime_cell_id: impl Into<String>,
    ) -> Self {
        Self {
            state: CodeCellTraceContextState::Enabled(EnabledCodeCellTraceContext {
                writer,
                thread_id: thread_id.into(),
                codex_turn_id: codex_turn_id.into(),
                runtime_cell_id: runtime_cell_id.into(),
            }),
        }
    }

    /// Records the parent runtime object before JavaScript can issue nested tool calls.
    pub fn record_started(
        &self,
        model_visible_call_id: impl Into<ModelVisibleCallId>,
        source_js: impl Into<String>,
    ) {
        let CodeCellTraceContextState::Enabled(context) = &self.state else {
            return;
        };
        append_with_context_best_effort(
            context,
            RawTraceEventPayload::CodeCellStarted {
                runtime_cell_id: context.runtime_cell_id.clone(),
                model_visible_call_id: model_visible_call_id.into(),
                source_js: source_js.into(),
            },
        );
    }

    /// Records the first response returned by the public code-mode `exec` tool.
    ///
    /// A yielded response returns control to the model while the cell keeps
    /// running. Terminal initial responses should be followed by `record_ended`
    /// by the caller so the reducer can distinguish model-visible output from
    /// runtime completion.
    pub fn record_initial_response(&self, response: &RuntimeResponse) {
        let CodeCellTraceContextState::Enabled(context) = &self.state else {
            return;
        };
        append_with_context_best_effort(
            context,
            RawTraceEventPayload::CodeCellInitialResponse {
                runtime_cell_id: context.runtime_cell_id.clone(),
                status: code_cell_status_for_runtime_response(response),
                response_payload: code_cell_response_payload(context, response),
            },
        );
    }

    /// Records the terminal lifecycle point for a code-mode runtime cell.
    pub fn record_ended(&self, response: &RuntimeResponse) {
        let CodeCellTraceContextState::Enabled(context) = &self.state else {
            return;
        };
        append_with_context_best_effort(
            context,
            RawTraceEventPayload::CodeCellEnded {
                runtime_cell_id: context.runtime_cell_id.clone(),
                status: code_cell_status_for_runtime_response(response),
                response_payload: code_cell_response_payload(context, response),
            },
        );
    }
}

fn code_cell_status_for_runtime_response(response: &RuntimeResponse) -> CodeCellRuntimeStatus {
    match response {
        RuntimeResponse::Yielded { .. } => CodeCellRuntimeStatus::Yielded,
        RuntimeResponse::Terminated { .. } => CodeCellRuntimeStatus::Terminated,
        RuntimeResponse::Result { error_text, .. } => {
            if error_text.is_some() {
                CodeCellRuntimeStatus::Failed
            } else {
                CodeCellRuntimeStatus::Completed
            }
        }
    }
}

fn code_cell_response_payload(
    context: &EnabledCodeCellTraceContext,
    response: &RuntimeResponse,
) -> Option<RawPayloadRef> {
    write_json_payload_best_effort(
        &context.writer,
        RawPayloadKind::ToolResult,
        &CodeCellResponseTracePayload { response },
    )
}

fn write_json_payload_best_effort(
    writer: &TraceWriter,
    kind: RawPayloadKind,
    payload: &impl Serialize,
) -> Option<RawPayloadRef> {
    match writer.write_json_payload(kind, payload) {
        Ok(payload_ref) => Some(payload_ref),
        Err(err) => {
            warn!("failed to write rollout trace payload: {err:#}");
            None
        }
    }
}

fn append_with_context_best_effort(
    context: &EnabledCodeCellTraceContext,
    payload: RawTraceEventPayload,
) {
    let event_context = RawTraceEventContext {
        thread_id: Some(context.thread_id.clone()),
        codex_turn_id: Some(context.codex_turn_id.clone()),
    };
    if let Err(err) = context.writer.append_with_context(event_context, payload) {
        warn!("failed to append rollout trace event: {err:#}");
    }
}
