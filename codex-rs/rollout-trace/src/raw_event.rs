//! Append-only raw trace events.

use crate::model::AgentThreadId;
use crate::model::CodeCellRuntimeStatus;
use crate::model::CodexTurnId;
use crate::model::CompactionId;
use crate::model::CompactionRequestId;
use crate::model::EdgeId;
use crate::model::ExecutionStatus;
use crate::model::InferenceCallId;
use crate::model::ModelVisibleCallId;
use crate::model::RolloutStatus;
use crate::model::ToolCallId;
use crate::model::ToolCallKind;
use crate::model::ToolCallSummary;
use crate::payload::RawPayloadRef;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// Monotonic sequence number assigned by the raw trace writer.
pub type RawEventSeq = u64;

/// Current raw event envelope schema version.
pub(crate) const RAW_TRACE_EVENT_SCHEMA_VERSION: u32 = 1;

/// One append-only raw trace event.
///
/// Every event uses the same envelope so partial replay and corruption checks
/// can run before the reducer understands the event-specific payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTraceEvent {
    pub schema_version: u32,
    /// Contiguous writer-assigned order inside one rollout event log.
    pub seq: RawEventSeq,
    /// Unix wall-clock timestamp in milliseconds. Use for display/latency.
    pub wall_time_unix_ms: i64,
    pub rollout_id: String,
    pub thread_id: Option<AgentThreadId>,
    pub codex_turn_id: Option<CodexTurnId>,
    pub payload: RawTraceEventPayload,
}

/// Writer-supplied context that appears in the raw event envelope.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawTraceEventContext {
    pub thread_id: Option<AgentThreadId>,
    pub codex_turn_id: Option<CodexTurnId>,
}

/// Runtime requester as observed at the raw tool boundary.
///
/// This intentionally uses runtime-local identifiers. The reducer is the only
/// place that maps these handles to graph identities such as `CodeCellId`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum RawToolCallRequester {
    Model,
    CodeCell {
        /// Runtime-local code-mode cell handle.
        runtime_cell_id: String,
    },
}

/// Typed payload for a raw trace event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum RawTraceEventPayload {
    RolloutStarted {
        trace_id: String,
        root_thread_id: AgentThreadId,
    },
    RolloutEnded {
        status: RolloutStatus,
    },
    ThreadStarted {
        thread_id: AgentThreadId,
        /// Stable agent path.
        agent_path: String,
        metadata_payload: Option<RawPayloadRef>,
    },
    ThreadEnded {
        thread_id: AgentThreadId,
        status: RolloutStatus,
    },
    CodexTurnStarted {
        codex_turn_id: CodexTurnId,
        thread_id: AgentThreadId,
    },
    CodexTurnEnded {
        codex_turn_id: CodexTurnId,
        status: ExecutionStatus,
    },
    InferenceStarted {
        inference_call_id: InferenceCallId,
        thread_id: AgentThreadId,
        codex_turn_id: CodexTurnId,
        model: String,
        provider_name: String,
        request_payload: RawPayloadRef,
    },
    InferenceCompleted {
        inference_call_id: InferenceCallId,
        response_id: Option<String>,
        response_payload: RawPayloadRef,
    },
    InferenceFailed {
        inference_call_id: InferenceCallId,
        error: String,
        /// Partial response payload, when stream events arrived before failure.
        partial_response_payload: Option<RawPayloadRef>,
    },
    InferenceCancelled {
        inference_call_id: InferenceCallId,
        /// Why Codex stopped consuming the provider stream before a terminal response event.
        reason: String,
        /// Completed output items observed before cancellation, if any.
        partial_response_payload: Option<RawPayloadRef>,
    },
    ToolCallStarted {
        tool_call_id: ToolCallId,
        /// Protocol/model call ID when this runtime call came from model output.
        model_visible_call_id: Option<String>,
        /// Code-mode runtime bridge ID when model-authored code issued this call.
        code_mode_runtime_tool_id: Option<String>,
        /// Runtime requester that caused this tool lifecycle.
        requester: RawToolCallRequester,
        kind: ToolCallKind,
        summary: ToolCallSummary,
        invocation_payload: Option<RawPayloadRef>,
    },
    ToolCallRuntimeStarted {
        tool_call_id: ToolCallId,
        /// Runtime/protocol observation for how Codex began executing the tool.
        runtime_payload: RawPayloadRef,
    },
    ToolCallRuntimeEnded {
        tool_call_id: ToolCallId,
        status: ExecutionStatus,
        /// Runtime/protocol observation for how Codex finished executing the tool.
        runtime_payload: RawPayloadRef,
    },
    ToolCallEnded {
        tool_call_id: ToolCallId,
        status: ExecutionStatus,
        result_payload: Option<RawPayloadRef>,
    },
    CodeCellStarted {
        /// Runtime-local handle allocated by code mode for waits and nested tools.
        runtime_cell_id: String,
        /// Custom tool call id on the model-visible `exec` item.
        model_visible_call_id: ModelVisibleCallId,
        /// JavaScript source after the public `exec` wrapper has been parsed.
        source_js: String,
    },
    CodeCellInitialResponse {
        /// Runtime-local handle, matching `CodeCellStarted`.
        runtime_cell_id: String,
        status: CodeCellRuntimeStatus,
        response_payload: Option<RawPayloadRef>,
    },
    CodeCellEnded {
        /// Runtime-local handle, matching `CodeCellStarted`.
        runtime_cell_id: String,
        status: CodeCellRuntimeStatus,
        response_payload: Option<RawPayloadRef>,
    },
    CompactionRequestStarted {
        compaction_id: CompactionId,
        compaction_request_id: CompactionRequestId,
        thread_id: AgentThreadId,
        codex_turn_id: CodexTurnId,
        model: String,
        provider_name: String,
        request_payload: RawPayloadRef,
    },
    CompactionRequestCompleted {
        compaction_id: CompactionId,
        compaction_request_id: CompactionRequestId,
        response_payload: RawPayloadRef,
    },
    CompactionRequestFailed {
        compaction_id: CompactionId,
        compaction_request_id: CompactionRequestId,
        error: String,
    },
    /// Checkpoint installation event for remote-compacted replacement history.
    CompactionInstalled {
        compaction_id: CompactionId,
        /// Trace-only checkpoint payload. Do not route this through public UI protocol.
        checkpoint_payload: RawPayloadRef,
    },
    /// Multi-agent v2 child-to-parent completion delivery.
    AgentResultObserved {
        edge_id: EdgeId,
        child_thread_id: AgentThreadId,
        child_codex_turn_id: CodexTurnId,
        parent_thread_id: AgentThreadId,
        message: String,
        /// Raw notification payload. This is evidence for the runtime delivery,
        /// not the parent-side model-visible item.
        carried_payload: Option<RawPayloadRef>,
    },
    /// Existing UI/protocol event wrapped into trace format.
    ProtocolEventObserved {
        event_type: String,
        event_payload: RawPayloadRef,
    },
    /// Structured payload for early instrumentation before a dedicated variant exists.
    Other {
        kind: String,
        summary: String,
        payloads: Vec<RawPayloadRef>,
        /// Small structured metadata. Large data belongs in `payloads`.
        metadata: Value,
    },
}

impl RawTraceEventPayload {
    /// Raw payload refs that must exist before this raw event is appended.
    pub(crate) fn raw_payload_refs(&self) -> Vec<&RawPayloadRef> {
        match self {
            RawTraceEventPayload::RolloutStarted { .. }
            | RawTraceEventPayload::RolloutEnded { .. }
            | RawTraceEventPayload::ThreadEnded { .. }
            | RawTraceEventPayload::CodexTurnStarted { .. }
            | RawTraceEventPayload::CodexTurnEnded { .. }
            | RawTraceEventPayload::CompactionRequestFailed { .. }
            | RawTraceEventPayload::CodeCellStarted { .. }
            | RawTraceEventPayload::AgentResultObserved {
                carried_payload: None,
                ..
            } => Vec::new(),
            RawTraceEventPayload::ThreadStarted {
                metadata_payload, ..
            } => metadata_payload.iter().collect(),
            RawTraceEventPayload::InferenceStarted {
                request_payload, ..
            }
            | RawTraceEventPayload::InferenceCompleted {
                response_payload: request_payload,
                ..
            }
            | RawTraceEventPayload::CompactionRequestStarted {
                request_payload, ..
            }
            | RawTraceEventPayload::CompactionRequestCompleted {
                response_payload: request_payload,
                ..
            }
            | RawTraceEventPayload::CompactionInstalled {
                checkpoint_payload: request_payload,
                ..
            }
            | RawTraceEventPayload::ProtocolEventObserved {
                event_payload: request_payload,
                ..
            } => vec![request_payload],
            RawTraceEventPayload::InferenceFailed {
                partial_response_payload,
                ..
            }
            | RawTraceEventPayload::InferenceCancelled {
                partial_response_payload,
                ..
            }
            | RawTraceEventPayload::ToolCallStarted {
                invocation_payload: partial_response_payload,
                ..
            }
            | RawTraceEventPayload::ToolCallEnded {
                result_payload: partial_response_payload,
                ..
            }
            | RawTraceEventPayload::CodeCellInitialResponse {
                response_payload: partial_response_payload,
                ..
            }
            | RawTraceEventPayload::CodeCellEnded {
                response_payload: partial_response_payload,
                ..
            } => partial_response_payload.iter().collect(),
            RawTraceEventPayload::AgentResultObserved {
                carried_payload: Some(carried_payload),
                ..
            } => vec![carried_payload],
            RawTraceEventPayload::ToolCallRuntimeStarted {
                runtime_payload, ..
            }
            | RawTraceEventPayload::ToolCallRuntimeEnded {
                runtime_payload, ..
            } => vec![runtime_payload],
            RawTraceEventPayload::Other { payloads, .. } => payloads.iter().collect(),
        }
    }
}
