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
        if let Err(err) = context.writer.enqueue_event(
            RawTraceEventContext::turn(&context.thread_id, &context.codex_turn_id),
            RawTraceEventPayload::CodeCellStarted {
                runtime_cell_id: context.runtime_cell_id.clone(),
                model_visible_call_id: model_visible_call_id.into(),
                source_js: source_js.into(),
            },
        ) {
            warn!("failed to append rollout trace event: {err:#}");
        }
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
        let runtime_cell_id = context.runtime_cell_id.clone();
        let status = code_cell_status_for_runtime_response(response);
        let _ = context.writer.event_with_payload(
            RawTraceEventContext::turn(&context.thread_id, &context.codex_turn_id),
            RawPayloadKind::ToolResult,
            &CodeCellResponseTracePayload { response },
            move |reference| RawTraceEventPayload::CodeCellInitialResponse {
                runtime_cell_id,
                status,
                response_payload: Some(reference),
            },
        );
    }

    /// Records the terminal lifecycle point for a code-mode runtime cell.
    pub fn record_ended(&self, response: &RuntimeResponse) {
        let CodeCellTraceContextState::Enabled(context) = &self.state else {
            return;
        };
        let runtime_cell_id = context.runtime_cell_id.clone();
        let status = code_cell_status_for_runtime_response(response);
        let _ = context.writer.event_with_payload(
            RawTraceEventContext::turn(&context.thread_id, &context.codex_turn_id),
            RawPayloadKind::ToolResult,
            &CodeCellResponseTracePayload { response },
            move |reference| RawTraceEventPayload::CodeCellEnded {
                runtime_cell_id,
                status,
                response_payload: Some(reference),
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
