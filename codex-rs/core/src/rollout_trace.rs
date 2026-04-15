//! Opt-in producer for the rollout trace bundle.
//!
//! This module is the deliberately thin bridge from `codex-core` into
//! `codex-rollout-trace`. Core emits raw observations; the trace crate's
//! offline reducer owns the semantic graph.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::agent::AgentStatus;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus;
use codex_protocol::protocol::PatchApplyStatus;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TurnAbortReason;
use codex_rollout_trace::AgentThreadId;
use codex_rollout_trace::CodeCellRuntimeStatus;
use codex_rollout_trace::CodeModeRuntimeToolId;
use codex_rollout_trace::CompactionTraceContext;
use codex_rollout_trace::ExecutionStatus;
use codex_rollout_trace::InferenceTraceContext;
use codex_rollout_trace::ModelVisibleCallId;
use codex_rollout_trace::RawPayloadKind;
use codex_rollout_trace::RawPayloadRef;
use codex_rollout_trace::RawToolCallRequester;
use codex_rollout_trace::RawTraceEventContext;
use codex_rollout_trace::RawTraceEventPayload;
use codex_rollout_trace::RolloutStatus;
use codex_rollout_trace::ToolCallKind;
use codex_rollout_trace::ToolCallSummary;
use codex_rollout_trace::TraceWriter;
use serde::Serialize;
use tracing::debug;
use tracing::warn;
use uuid::Uuid;

/// Environment variable that enables local trace-bundle recording.
///
/// The value is a root directory. Each independent root session gets one child
/// bundle directory. Spawned child threads share their root session's bundle so
/// one reduced `state.json` describes the whole multi-agent rollout tree.
pub(crate) const CODEX_ROLLOUT_TRACE_ROOT_ENV: &str = "CODEX_ROLLOUT_TRACE_ROOT";

/// Lightweight handle stored in `SessionServices`.
///
/// Cloning the handle is cheap; all sequencing and file ownership remains
/// inside `TraceWriter`.
#[derive(Clone, Debug)]
pub(crate) struct RolloutTraceRecorder {
    writer: Arc<TraceWriter>,
    root_thread_id: AgentThreadId,
}

/// Metadata captured once at thread/session start.
///
/// This payload is intentionally operational rather than reduced: it is a raw
/// payload that later reducers can mine as the reduced thread model evolves.
#[derive(Serialize)]
pub(crate) struct ThreadStartedTraceMetadata {
    pub(crate) thread_id: String,
    pub(crate) agent_path: String,
    pub(crate) task_name: Option<String>,
    pub(crate) nickname: Option<String>,
    pub(crate) agent_role: Option<String>,
    pub(crate) session_source: SessionSource,
    pub(crate) cwd: PathBuf,
    pub(crate) rollout_path: Option<PathBuf>,
    pub(crate) model: String,
    pub(crate) provider_name: String,
    pub(crate) approval_policy: String,
    pub(crate) sandbox_policy: String,
}

/// History replacement checkpoint persisted when compaction installs new live history.
///
/// The checkpoint keeps compaction separate from ordinary sampling snapshots:
/// `input_history` is the live thread history selected for compaction, while
/// `replacement_history` is what future prompts may carry after the checkpoint.
#[derive(Serialize)]
pub(crate) struct CompactionCheckpointTracePayload<'a> {
    pub(crate) input_history: &'a [ResponseItem],
    pub(crate) replacement_history: &'a [ResponseItem],
}

/// Raw invocation payload for the canonical Codex tool boundary.
///
/// Protocol events may add runtime detail later, but this envelope preserves
/// the caller-facing request for both direct model calls and code-mode nested
/// calls.
#[derive(Serialize)]
struct DispatchedToolTraceRequest<'a> {
    tool_name: &'a str,
    tool_namespace: Option<&'a str>,
    payload: serde_json::Value,
}

/// Raw response payload for dispatch-level tool trace events.
#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum DispatchedToolTraceResponse<'a> {
    DirectResponse {
        response_item: &'a ResponseInputItem,
    },
    CodeModeResponse {
        value: serde_json::Value,
    },
    Error {
        error: &'a str,
    },
}

/// Raw code-mode response captured at the runtime boundary.
///
/// The reducer keeps the graph small and uses this payload as evidence for
/// future viewers that need exact content items or stored-value details.
#[derive(Serialize)]
struct CodeCellResponseTracePayload<'a> {
    response: &'a codex_code_mode::RuntimeResponse,
}

/// Trace-only payload for the notification a finished child sends back to its parent.
#[derive(Serialize)]
struct AgentResultTracePayload<'a> {
    child_agent_path: &'a str,
    message: &'a str,
    status: &'a AgentStatus,
}

impl RolloutTraceRecorder {
    /// Creates and starts a trace bundle if `CODEX_ROLLOUT_TRACE_ROOT` is set.
    ///
    /// Trace startup is best-effort. A tracing failure must not make the Codex
    /// session unusable, because traces are diagnostic and can be enabled while
    /// debugging unrelated production failures.
    pub(crate) fn maybe_create(
        thread_id: ThreadId,
        metadata: ThreadStartedTraceMetadata,
    ) -> Option<Self> {
        let root = std::env::var_os(CODEX_ROLLOUT_TRACE_ROOT_ENV)?;
        let root = PathBuf::from(root);
        match Self::create_in_root(root.as_path(), thread_id, metadata) {
            Ok(recorder) => Some(recorder),
            Err(err) => {
                warn!("failed to initialize rollout trace recorder: {err:#}");
                None
            }
        }
    }

    fn create_in_root(
        root: &Path,
        thread_id: ThreadId,
        metadata: ThreadStartedTraceMetadata,
    ) -> anyhow::Result<Self> {
        let trace_id = Uuid::new_v4().to_string();
        let thread_id = thread_id.to_string();
        let bundle_dir = root.join(format!("trace-{trace_id}-{thread_id}"));
        let writer = TraceWriter::create(
            &bundle_dir,
            trace_id.clone(),
            thread_id.clone(),
            thread_id.clone(),
        )?;
        let recorder = Self {
            writer: Arc::new(writer),
            root_thread_id: thread_id.clone(),
        };

        recorder.append_best_effort(RawTraceEventPayload::RolloutStarted {
            trace_id,
            root_thread_id: thread_id,
        });

        recorder.record_thread_started(metadata);

        debug!("recording rollout trace at {}", bundle_dir.display());
        Ok(recorder)
    }

    #[cfg(test)]
    pub(crate) fn create_in_root_for_test(
        root: &Path,
        thread_id: ThreadId,
        metadata: ThreadStartedTraceMetadata,
    ) -> anyhow::Result<Self> {
        Self::create_in_root(root, thread_id, metadata)
    }

    /// Wraps selected UI/protocol events in the trace bundle.
    ///
    /// We intentionally skip high-volume stream deltas here. Inference/tool
    /// hooks emit typed raw events; protocol wrappers are debug breadcrumbs, not
    /// the canonical transcript.
    pub(crate) fn record_protocol_event(&self, event: &EventMsg) {
        let Some(event_type) = wrapped_protocol_event_type(event) else {
            return;
        };
        let event_payload =
            match self.write_json_payload_best_effort(RawPayloadKind::ProtocolEvent, event) {
                Some(event_payload) => event_payload,
                None => return,
            };
        self.append_best_effort(RawTraceEventPayload::ProtocolEventObserved {
            event_type: event_type.to_string(),
            event_payload,
        });
    }

    /// Emits the lifecycle event and metadata for one thread in this rollout tree.
    ///
    /// Root sessions call this immediately after `RolloutStarted`; spawned
    /// child sessions call it on the inherited recorder. Keeping children in
    /// the root bundle preserves one raw payload namespace and one reduced
    /// `RolloutTrace` for the whole multi-agent task.
    pub(crate) fn record_thread_started(&self, metadata: ThreadStartedTraceMetadata) {
        let metadata_payload =
            self.write_json_payload_best_effort(RawPayloadKind::SessionMetadata, &metadata);
        self.append_best_effort(RawTraceEventPayload::ThreadStarted {
            thread_id: metadata.thread_id,
            agent_path: metadata.agent_path,
            metadata_payload,
        });
    }

    /// Emits typed turn lifecycle events from the UI/protocol lifecycle.
    pub(crate) fn record_codex_turn_event(
        &self,
        thread_id: AgentThreadId,
        default_turn_id: &str,
        event: &EventMsg,
    ) {
        match event {
            EventMsg::TurnStarted(event) => {
                self.append_with_context_best_effort(
                    thread_id.clone(),
                    event.turn_id.clone(),
                    RawTraceEventPayload::CodexTurnStarted {
                        codex_turn_id: event.turn_id.clone(),
                        thread_id,
                    },
                );
            }
            EventMsg::TurnComplete(event) => {
                self.append_with_context_best_effort(
                    thread_id,
                    event.turn_id.clone(),
                    RawTraceEventPayload::CodexTurnEnded {
                        codex_turn_id: event.turn_id.clone(),
                        status: ExecutionStatus::Completed,
                    },
                );
            }
            EventMsg::TurnAborted(event) => {
                let turn_id = event
                    .turn_id
                    .clone()
                    .unwrap_or_else(|| default_turn_id.to_string());
                self.append_with_context_best_effort(
                    thread_id,
                    turn_id.clone(),
                    RawTraceEventPayload::CodexTurnEnded {
                        codex_turn_id: turn_id,
                        status: execution_status_for_abort_reason(&event.reason),
                    },
                );
            }
            _ => {}
        }
    }

    /// Emits typed runtime tool events from existing protocol lifecycle events.
    ///
    /// The protocol event stays separate from the caller-facing invocation and
    /// result payloads. Reducers attach it to `ToolCall.raw_runtime_payload_ids`
    /// and can also use it to build richer objects such as terminal operations.
    pub(crate) fn record_tool_call_event(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        event: &EventMsg,
    ) {
        let Some(payload) = self.tool_call_trace_payload(event) else {
            return;
        };
        self.append_with_context_best_effort(thread_id, codex_turn_id, payload);
    }

    /// Emits the parent runtime object for one model-authored code-mode cell.
    ///
    /// This must run before JavaScript starts because the runtime can request
    /// nested tools before the initial custom-tool response is available.
    pub(crate) fn record_code_cell_started(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        runtime_cell_id: &str,
        model_visible_call_id: &str,
        source_js: &str,
    ) {
        self.append_with_context_best_effort(
            thread_id,
            codex_turn_id,
            RawTraceEventPayload::CodeCellStarted {
                runtime_cell_id: runtime_cell_id.to_string(),
                model_visible_call_id: model_visible_call_id.to_string(),
                source_js: source_js.to_string(),
            },
        );
    }

    /// Emits the first response returned by the public code-mode `exec` tool.
    ///
    /// A yielded response returns control to the model while the cell keeps
    /// running. A terminal response is followed by `CodeCellEnded` so the
    /// reducer can distinguish "first model-visible output" from runtime end.
    pub(crate) fn record_code_cell_initial_response(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        response: &codex_code_mode::RuntimeResponse,
    ) {
        let response_payload = self.code_cell_response_payload(response);
        self.append_with_context_best_effort(
            thread_id,
            codex_turn_id,
            RawTraceEventPayload::CodeCellInitialResponse {
                runtime_cell_id: code_cell_runtime_id(response).to_string(),
                status: code_cell_status_for_runtime_response(response),
                response_payload,
            },
        );
    }

    /// Emits the terminal lifecycle point for a code-mode cell.
    pub(crate) fn record_code_cell_ended(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        response: &codex_code_mode::RuntimeResponse,
    ) {
        let response_payload = self.code_cell_response_payload(response);
        self.append_with_context_best_effort(
            thread_id,
            codex_turn_id,
            RawTraceEventPayload::CodeCellEnded {
                runtime_cell_id: code_cell_runtime_id(response).to_string(),
                status: code_cell_status_for_runtime_response(response),
                response_payload,
            },
        );
    }

    /// Emits a generic lifecycle start for direct/code-mode tools without a
    /// richer protocol-backed lifecycle.
    ///
    /// The registry calls this after it has resolved a concrete handler. At that
    /// point we know the tool call is valid, but we are still before
    /// approval/pre-use hooks, so blocked tools are represented as failed tool
    /// executions instead of disappearing from the trace.
    pub(crate) fn record_dispatched_tool_call_started(&self, invocation: &ToolInvocation) {
        let request = DispatchedToolTraceRequest {
            tool_name: invocation.tool_name.name.as_str(),
            tool_namespace: invocation.tool_name.namespace.as_deref(),
            payload: dispatched_tool_payload(&invocation.payload),
        };
        let request_payload =
            self.write_json_payload_best_effort(RawPayloadKind::ToolInvocation, &request);
        let (model_visible_call_id, code_mode_runtime_tool_id, requester) =
            dispatched_tool_requester_fields(invocation);

        self.append_with_context_best_effort(
            invocation.session.conversation_id.to_string(),
            invocation.turn.sub_id.clone(),
            RawTraceEventPayload::ToolCallStarted {
                tool_call_id: invocation.call_id.clone(),
                model_visible_call_id,
                code_mode_runtime_tool_id,
                requester,
                kind: dispatched_tool_kind(invocation),
                summary: ToolCallSummary::Generic {
                    label: dispatched_tool_label(invocation),
                    input_preview: Some(truncate_preview(&invocation.payload.log_payload())),
                    output_preview: None,
                },
                invocation_payload: request_payload,
            },
        );
    }

    /// Emits the caller-facing result for a dispatch-level tool lifecycle.
    pub(crate) fn record_dispatched_tool_call_ended(
        &self,
        invocation: &ToolInvocation,
        status: ExecutionStatus,
        result: &dyn ToolOutput,
        response_call_id: &str,
        tool_payload: &ToolPayload,
    ) {
        let direct_response_item;
        let response = match invocation.source {
            ToolCallSource::Direct => {
                direct_response_item = result.to_response_item(response_call_id, tool_payload);
                DispatchedToolTraceResponse::DirectResponse {
                    response_item: &direct_response_item,
                }
            }
            ToolCallSource::CodeMode { .. } => DispatchedToolTraceResponse::CodeModeResponse {
                value: result.code_mode_result(tool_payload),
            },
            ToolCallSource::JsRepl => return,
        };
        self.append_dispatched_tool_call_ended(invocation, status, &response);
    }

    /// Emits a failed end event for a dispatch-level tool lifecycle.
    pub(crate) fn record_dispatched_tool_call_failed(
        &self,
        invocation: &ToolInvocation,
        error: &str,
    ) {
        let response = DispatchedToolTraceResponse::Error { error };
        self.append_dispatched_tool_call_ended(invocation, ExecutionStatus::Failed, &response);
    }

    fn append_dispatched_tool_call_ended(
        &self,
        invocation: &ToolInvocation,
        status: ExecutionStatus,
        response: &DispatchedToolTraceResponse<'_>,
    ) {
        let response_payload =
            self.write_json_payload_best_effort(RawPayloadKind::ToolResult, response);
        self.append_with_context_best_effort(
            invocation.session.conversation_id.to_string(),
            invocation.turn.sub_id.clone(),
            RawTraceEventPayload::ToolCallEnded {
                tool_call_id: invocation.call_id.clone(),
                status,
                result_payload: response_payload,
            },
        );
    }

    /// Builds reusable inference trace context for one Codex turn.
    ///
    /// The returned context is intentionally not "an inference call" yet.
    /// Transport code owns retry/fallback attempts and calls `start_attempt`
    /// only after it has built the concrete request payload for that attempt.
    pub(crate) fn inference_trace_context(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        model: String,
        provider_name: String,
    ) -> InferenceTraceContext {
        InferenceTraceContext::enabled(
            Arc::clone(&self.writer),
            thread_id,
            codex_turn_id,
            model,
            provider_name,
        )
    }

    /// Builds remote-compaction trace context for one checkpoint.
    ///
    /// Rollout tracing currently has a first-class checkpoint model only for remote compaction.
    /// The compact endpoint is a model-facing request whose output replaces live history, so it
    /// needs both request/response attempt events and a later checkpoint event when processed
    /// replacement history is installed.
    pub(crate) fn compaction_trace_context(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        compaction_id: String,
        model: String,
        provider_name: String,
    ) -> CompactionTraceContext {
        CompactionTraceContext::enabled(
            Arc::clone(&self.writer),
            thread_id,
            codex_turn_id,
            compaction_id,
            model,
            provider_name,
        )
    }

    /// Emits the checkpoint where remote-compacted history replaces live thread history.
    ///
    /// This checkpoint is deliberately separate from the compact endpoint response: Codex filters
    /// and reinjects context before replacement history becomes live. The reducer uses this event
    /// to connect the pre-compaction history to the processed replacement items without treating
    /// repeated developer/context prefix items as part of the replacement itself.
    pub(crate) fn record_compaction_installed(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        compaction_id: String,
        checkpoint: &CompactionCheckpointTracePayload<'_>,
    ) {
        let Some(checkpoint_payload) =
            self.write_json_payload_best_effort(RawPayloadKind::CompactionCheckpoint, checkpoint)
        else {
            return;
        };
        self.append_with_context_best_effort(
            thread_id,
            codex_turn_id,
            RawTraceEventPayload::CompactionInstalled {
                compaction_id,
                checkpoint_payload,
            },
        );
    }

    /// Emits the v2 child-to-parent completion message as an explicit graph edge.
    ///
    /// This notification is not a tool call in the child: it is runtime delivery
    /// from the completed child turn into the parent's mailbox. Without a
    /// trace-owned edge the viewer would have to infer the relationship from a
    /// later parent prompt snapshot, which loses the runtime timing and source.
    pub(crate) fn record_agent_result_interaction(
        &self,
        child_thread_id: AgentThreadId,
        child_codex_turn_id: String,
        parent_thread_id: AgentThreadId,
        child_agent_path: &str,
        message: &str,
        status: &AgentStatus,
    ) {
        let carried_payload = self.write_json_payload_best_effort(
            RawPayloadKind::AgentResult,
            &AgentResultTracePayload {
                child_agent_path,
                message,
                status,
            },
        );
        self.append_with_context_best_effort(
            child_thread_id.clone(),
            child_codex_turn_id.clone(),
            RawTraceEventPayload::AgentResultObserved {
                edge_id: format!(
                    "edge:agent_result:{child_thread_id}:{child_codex_turn_id}:{parent_thread_id}"
                ),
                child_thread_id,
                child_codex_turn_id,
                parent_thread_id,
                message: message.to_string(),
                carried_payload,
            },
        );
    }

    /// Emits terminal trace events for graceful session shutdown.
    ///
    /// Child agent sessions share their root recorder, so ending a child thread
    /// must not close the whole rollout. Only the root thread's shutdown emits
    /// `RolloutEnded`.
    pub(crate) fn record_thread_ended(&self, thread_id: AgentThreadId, status: RolloutStatus) {
        self.append_best_effort(RawTraceEventPayload::ThreadEnded {
            thread_id: thread_id.clone(),
            status: status.clone(),
        });
        if thread_id == self.root_thread_id {
            self.append_best_effort(RawTraceEventPayload::RolloutEnded { status });
        }
    }

    fn write_json_payload_best_effort(
        &self,
        kind: RawPayloadKind,
        payload: &impl Serialize,
    ) -> Option<codex_rollout_trace::RawPayloadRef> {
        match self.writer.write_json_payload(kind, payload) {
            Ok(payload_ref) => Some(payload_ref),
            Err(err) => {
                warn!("failed to write rollout trace payload: {err:#}");
                None
            }
        }
    }

    fn append_best_effort(&self, payload: RawTraceEventPayload) {
        if let Err(err) = self.writer.append(payload) {
            warn!("failed to append rollout trace event: {err:#}");
        }
    }

    fn append_with_context_best_effort(
        &self,
        thread_id: AgentThreadId,
        codex_turn_id: String,
        payload: RawTraceEventPayload,
    ) {
        let context = RawTraceEventContext {
            thread_id: Some(thread_id),
            codex_turn_id: Some(codex_turn_id),
        };
        if let Err(err) = self.writer.append_with_context(context, payload) {
            warn!("failed to append rollout trace event: {err:#}");
        }
    }

    fn tool_call_trace_payload(&self, event: &EventMsg) -> Option<RawTraceEventPayload> {
        match event {
            EventMsg::ExecCommandBegin(event) if event.source != ExecCommandSource::UserShell => {
                self.tool_runtime_started_payload(&event.call_id, event)
            }
            EventMsg::ExecCommandEnd(event) if event.source != ExecCommandSource::UserShell => self
                .tool_runtime_ended_payload(
                    &event.call_id,
                    execution_status_for_exec_status(&event.status),
                    event,
                ),
            EventMsg::PatchApplyBegin(event) => {
                self.tool_runtime_started_payload(&event.call_id, event)
            }
            EventMsg::PatchApplyEnd(event) => self.tool_runtime_ended_payload(
                &event.call_id,
                execution_status_for_patch_status(&event.status),
                event,
            ),
            EventMsg::McpToolCallBegin(event) => {
                self.tool_runtime_started_payload(&event.call_id, event)
            }
            EventMsg::McpToolCallEnd(event) => self.tool_runtime_ended_payload(
                &event.call_id,
                if event.result.is_ok() {
                    ExecutionStatus::Completed
                } else {
                    ExecutionStatus::Failed
                },
                event,
            ),
            EventMsg::CollabAgentSpawnBegin(event) => {
                self.tool_runtime_started_payload(&event.call_id, event)
            }
            EventMsg::CollabAgentSpawnEnd(event) => self.tool_runtime_ended_payload(
                &event.call_id,
                if event.new_thread_id.is_some() {
                    ExecutionStatus::Completed
                } else {
                    ExecutionStatus::Failed
                },
                event,
            ),
            EventMsg::CollabAgentInteractionBegin(event) => {
                self.tool_runtime_started_payload(&event.call_id, event)
            }
            EventMsg::CollabAgentInteractionEnd(event) => {
                self.tool_runtime_ended_payload(&event.call_id, ExecutionStatus::Completed, event)
            }
            EventMsg::CollabWaitingBegin(event) => {
                self.tool_runtime_started_payload(&event.call_id, event)
            }
            EventMsg::CollabWaitingEnd(event) => {
                self.tool_runtime_ended_payload(&event.call_id, ExecutionStatus::Completed, event)
            }
            EventMsg::CollabCloseBegin(event) => {
                self.tool_runtime_started_payload(&event.call_id, event)
            }
            EventMsg::CollabCloseEnd(event) => {
                self.tool_runtime_ended_payload(&event.call_id, ExecutionStatus::Completed, event)
            }
            _ => None,
        }
    }

    fn tool_runtime_started_payload(
        &self,
        tool_call_id: &str,
        event: &impl Serialize,
    ) -> Option<RawTraceEventPayload> {
        let runtime_payload =
            self.write_json_payload_best_effort(RawPayloadKind::ToolRuntimeEvent, event)?;
        Some(RawTraceEventPayload::ToolCallRuntimeStarted {
            tool_call_id: tool_call_id.to_string(),
            runtime_payload,
        })
    }

    fn tool_runtime_ended_payload(
        &self,
        tool_call_id: &str,
        status: ExecutionStatus,
        event: &impl Serialize,
    ) -> Option<RawTraceEventPayload> {
        let runtime_payload =
            self.write_json_payload_best_effort(RawPayloadKind::ToolRuntimeEvent, event)?;
        Some(RawTraceEventPayload::ToolCallRuntimeEnded {
            tool_call_id: tool_call_id.to_string(),
            status,
            runtime_payload,
        })
    }

    fn code_cell_response_payload(
        &self,
        response: &codex_code_mode::RuntimeResponse,
    ) -> Option<RawPayloadRef> {
        self.write_json_payload_best_effort(
            RawPayloadKind::ToolResult,
            &CodeCellResponseTracePayload { response },
        )
    }
}

fn execution_status_for_abort_reason(reason: &TurnAbortReason) -> ExecutionStatus {
    match reason {
        TurnAbortReason::Interrupted | TurnAbortReason::Replaced | TurnAbortReason::ReviewEnded => {
            ExecutionStatus::Cancelled
        }
    }
}

fn execution_status_for_exec_status(status: &ExecCommandStatus) -> ExecutionStatus {
    match status {
        ExecCommandStatus::Completed => ExecutionStatus::Completed,
        ExecCommandStatus::Failed => ExecutionStatus::Failed,
        ExecCommandStatus::Declined => ExecutionStatus::Cancelled,
    }
}

fn execution_status_for_patch_status(status: &PatchApplyStatus) -> ExecutionStatus {
    match status {
        PatchApplyStatus::Completed => ExecutionStatus::Completed,
        PatchApplyStatus::Failed => ExecutionStatus::Failed,
        PatchApplyStatus::Declined => ExecutionStatus::Cancelled,
    }
}

fn code_cell_runtime_id(response: &codex_code_mode::RuntimeResponse) -> &str {
    match response {
        codex_code_mode::RuntimeResponse::Yielded { cell_id, .. }
        | codex_code_mode::RuntimeResponse::Terminated { cell_id, .. }
        | codex_code_mode::RuntimeResponse::Result { cell_id, .. } => cell_id,
    }
}

fn code_cell_status_for_runtime_response(
    response: &codex_code_mode::RuntimeResponse,
) -> CodeCellRuntimeStatus {
    match response {
        codex_code_mode::RuntimeResponse::Yielded { .. } => CodeCellRuntimeStatus::Yielded,
        codex_code_mode::RuntimeResponse::Terminated { .. } => CodeCellRuntimeStatus::Terminated,
        codex_code_mode::RuntimeResponse::Result { error_text, .. } => {
            if error_text.is_some() {
                CodeCellRuntimeStatus::Failed
            } else {
                CodeCellRuntimeStatus::Completed
            }
        }
    }
}

fn dispatched_tool_requester_fields(
    invocation: &ToolInvocation,
) -> (
    Option<ModelVisibleCallId>,
    Option<CodeModeRuntimeToolId>,
    RawToolCallRequester,
) {
    match &invocation.source {
        ToolCallSource::Direct => (
            Some(invocation.call_id.clone()),
            None,
            RawToolCallRequester::Model,
        ),
        ToolCallSource::CodeMode {
            cell_id,
            runtime_tool_call_id,
        } => (
            None,
            Some(runtime_tool_call_id.clone()),
            RawToolCallRequester::CodeCell {
                runtime_cell_id: cell_id.clone(),
            },
        ),
        ToolCallSource::JsRepl => (None, None, RawToolCallRequester::Model),
    }
}

fn dispatched_tool_kind(invocation: &ToolInvocation) -> ToolCallKind {
    if let ToolPayload::Mcp { server, tool, .. } = &invocation.payload {
        return ToolCallKind::Mcp {
            server: server.clone(),
            tool: tool.clone(),
        };
    }

    match invocation.tool_name.name.as_str() {
        "exec_command" | "local_shell" | "shell" | "shell_command" => ToolCallKind::ExecCommand,
        "write_stdin" => ToolCallKind::WriteStdin,
        "apply_patch" => ToolCallKind::ApplyPatch,
        "web_search" | "web_search_preview" => ToolCallKind::Web,
        "image_generation" | "image_query" => ToolCallKind::ImageGeneration,
        "spawn_agent" => ToolCallKind::SpawnAgent,
        "send_message" => ToolCallKind::SendMessage,
        "followup_task" => ToolCallKind::AssignAgentTask,
        "wait_agent" => ToolCallKind::WaitAgent,
        "close_agent" => ToolCallKind::CloseAgent,
        other => ToolCallKind::Other {
            name: other.to_string(),
        },
    }
}

fn dispatched_tool_label(invocation: &ToolInvocation) -> String {
    if let ToolPayload::Mcp { server, tool, .. } = &invocation.payload {
        return format!("mcp:{server}:{tool}");
    }

    invocation.tool_name.to_string()
}

fn dispatched_tool_payload(payload: &ToolPayload) -> serde_json::Value {
    match payload {
        ToolPayload::Function { arguments } => serde_json::json!({
            "type": "function",
            "arguments": arguments,
        }),
        ToolPayload::ToolSearch { arguments } => serde_json::json!({
            "type": "tool_search",
            "arguments": arguments,
        }),
        ToolPayload::Custom { input } => serde_json::json!({
            "type": "custom",
            "input": input,
        }),
        ToolPayload::LocalShell { params } => serde_json::json!({
            "type": "local_shell",
            "command": params.command,
            "workdir": params.workdir,
            "timeout_ms": params.timeout_ms,
        }),
        ToolPayload::Mcp {
            server,
            tool,
            raw_arguments,
        } => serde_json::json!({
            "type": "mcp",
            "server": server,
            "tool": tool,
            "raw_arguments": raw_arguments,
        }),
    }
}

fn truncate_preview(value: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 160;
    let mut preview = value.chars().take(MAX_PREVIEW_CHARS).collect::<String>();
    if value.chars().count() > MAX_PREVIEW_CHARS {
        preview.push_str("...");
    }
    preview
}

fn wrapped_protocol_event_type(event: &EventMsg) -> Option<&'static str> {
    match event {
        EventMsg::SessionConfigured(_) => Some("session_configured"),
        EventMsg::TurnStarted(_) => Some("turn_started"),
        EventMsg::TurnComplete(_) => Some("turn_complete"),
        EventMsg::TurnAborted(_) => Some("turn_aborted"),
        EventMsg::ThreadNameUpdated(_) => Some("thread_name_updated"),
        EventMsg::ThreadRolledBack(_) => Some("thread_rolled_back"),
        EventMsg::Error(_) => Some("error"),
        EventMsg::Warning(_) => Some("warning"),
        EventMsg::ShutdownComplete => Some("shutdown_complete"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "rollout_trace_tests.rs"]
mod tests;
