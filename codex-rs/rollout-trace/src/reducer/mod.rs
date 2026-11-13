//! Deterministic replay from raw trace events to `RolloutTrace`.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;

use crate::bundle::MANIFEST_FILE_NAME;
use crate::bundle::RAW_EVENT_LOG_FILE_NAME;
use crate::bundle::TRACE_MANIFEST_SCHEMA_VERSION;
use crate::bundle::TraceBundleManifest;
use crate::model::ExecutionStatus;
use crate::model::RolloutTrace;
use crate::shard::PayloadReader;

use crate::raw_event::RAW_TRACE_EVENT_SCHEMA_VERSION;
use crate::raw_event::RawTraceEvent;
use crate::raw_event::RawTraceEventPayload;

mod code_cell;
mod compaction;
mod conversation;
mod inference;
#[cfg(test)]
pub(crate) mod test_support;
mod thread;
mod tool;

#[cfg(test)]
#[path = "acceptance_tests.rs"]
mod acceptance_tests;

use self::code_cell::PendingCodeCellLifecycleEvent;
use self::code_cell::PendingCodeCellStart;
use self::code_cell::StartedCodeCell;
use self::compaction::StartedCompactionRequest;
use self::inference::StartedInferenceCall;
use self::tool::ObservedAgentResultEdge;
use self::tool::PendingAgentInteractionEdge;
use self::tool::ToolCallStarted;
use crate::RecordingStatus;

/// Replays a trace bundle written by this executable into a reduced rollout graph.
/// A valid prefix has `RecordingStatus::Unclosed`. Malformed events or missing
/// or invalid referenced payloads return an error.
pub fn replay_bundle(bundle_dir: impl AsRef<Path>) -> Result<RolloutTrace> {
    let bundle_dir = bundle_dir.as_ref();
    let manifest: TraceBundleManifest =
        serde_json::from_reader(File::open(bundle_dir.join(MANIFEST_FILE_NAME))?)
            .with_context(|| format!("read {}", bundle_dir.join(MANIFEST_FILE_NAME).display()))?;
    ensure!(
        manifest.schema_version == TRACE_MANIFEST_SCHEMA_VERSION,
        "unsupported manifest schema {} (expected {TRACE_MANIFEST_SCHEMA_VERSION}). Run the exact Codex executable that recorded this bundle",
        manifest.schema_version
    );
    let mut reducer = TraceReducer {
        rollout: RolloutTrace::new(
            manifest.trace_id,
            manifest.rollout_id,
            manifest.root_thread_id,
            manifest.started_at_unix_ms,
        ),
        payload_reader: PayloadReader::new(bundle_dir),
        next_conversation_item_ordinal: 1,
        next_terminal_operation_ordinal: 1,
        terminal_ids_by_process: BTreeMap::new(),
        process_ids_by_terminal: BTreeMap::new(),
        thread_conversation_snapshots: BTreeMap::new(),
        pending_compaction_replacement_item_ids: BTreeMap::new(),
        code_cell_ids_by_runtime: BTreeMap::new(),
        last_thread_resume_seq: BTreeMap::new(),
        pending_code_cell_starts: BTreeMap::new(),
        pending_code_cell_lifecycle_events: BTreeMap::new(),
        pending_agent_interaction_edges: Vec::new(),
        first_acceptances: BTreeMap::new(),
        receipt_only_items: BTreeMap::new(),
    };

    let event_log_path = bundle_dir.join(RAW_EVENT_LOG_FILE_NAME);
    let event_log = File::open(&event_log_path)
        .with_context(|| format!("open trace event log {}", event_log_path.display()))?;
    let mut event_log = BufReader::new(event_log);
    let mut event_count = 0;
    let mut line = String::new();
    for line_number in 1.. {
        line.clear();
        if event_log
            .read_line(&mut line)
            .with_context(|| format!("read trace event line {line_number}"))?
            == 0
        {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        ensure!(
            reducer.rollout.recording_status == RecordingStatus::Unclosed,
            "trace event after recording completed"
        );
        let event: RawTraceEvent = serde_json::from_str(&line)
            .with_context(|| format!("parse trace event line {line_number}"))?;
        ensure!(
            event.schema_version == RAW_TRACE_EVENT_SCHEMA_VERSION,
            "unsupported trace event schema {} at line {line_number} (expected {RAW_TRACE_EVENT_SCHEMA_VERSION}). Run the exact Codex executable that recorded this bundle",
            event.schema_version,
        );
        event_count += 1;
        ensure!(
            event.seq == event_count,
            "missing or reordered trace event {event_count}"
        );
        ensure!(
            event.rollout_id == reducer.rollout.rollout_id,
            "trace event belongs to another rollout"
        );
        // A short write or copy can leave the entire marker except its final newline.
        // Only the writer's complete record establishes clean closure.
        if matches!(event.payload, RawTraceEventPayload::RecordingCompleted)
            && !line.ends_with('\n')
        {
            break;
        }
        reducer.apply_event(event)?;
    }
    reducer.payload_reader.validate(
        &reducer.rollout.raw_payloads,
        reducer.rollout.recording_status != RecordingStatus::Unclosed,
    )?;
    // Spawn edges prefer the child task message as their target, but a child can
    // fail before that message is ever reduced. Only after replaying the whole
    // bundle do we know which spawn deliveries need the child-thread fallback.
    reducer.resolve_pending_spawn_edge_fallbacks()?;
    // Cell starts can be queued until their model output arrives. Link only
    // materialized cells, after both sides of the dispatch have been replayed.
    for tool in reducer.rollout.tool_calls.values_mut() {
        if tool.requester == crate::ToolCallRequester::Model
            && matches!(&tool.kind, crate::ToolCallKind::Other { name } if name == codex_code_mode::PUBLIC_TOOL_NAME)
            && let Some(call_id) = &tool.model_visible_call_id
            && let Some(cell) = reducer.rollout.code_cells.get(
                &TraceReducer::reduced_code_cell_id_for_model_visible_call(call_id),
            )
            && cell.thread_id == tool.thread_id
            && tool
                .started_by_codex_turn_id
                .as_ref()
                .is_none_or(|turn| turn == &cell.codex_turn_id)
        {
            tool.started_code_cell_id = Some(cell.code_cell_id.clone());
        }
    }

    // Acceptance and response records can arrive in either order. Select history
    // after replay, retaining discarded attempts elsewhere in the graph.
    // Request/checkpoint items include inherited context without local acceptance.
    for item in reducer.rollout.conversation_items.values_mut() {
        if let Some(&(at_unix_ms, seq)) = item
            .responses_item_id
            .as_ref()
            .and_then(|id| reducer.first_acceptances.get(&item.thread_id)?.get(id))
        {
            item.accepted_at_unix_ms = Some(at_unix_ms);
            item.accepted_seq = Some(seq);
        }
    }
    let context_item_ids: BTreeSet<_> = reducer
        .rollout
        .inference_calls
        .values()
        .flat_map(|call| call.request_item_ids.iter())
        .chain(reducer.rollout.compactions.values().flat_map(|checkpoint| {
            checkpoint
                .input_item_ids
                .iter()
                .chain(&checkpoint.replacement_item_ids)
                .chain(std::iter::once(&checkpoint.marker_item_id))
        }))
        .collect();
    for thread in reducer.rollout.threads.values_mut() {
        let accepted_ids = reducer
            .first_acceptances
            .remove(&thread.thread_id)
            .unwrap_or_default();
        thread.conversation_item_ids.retain(|id| {
            let item = &reducer.rollout.conversation_items[id];
            context_item_ids.contains(id)
                || item
                    .responses_item_id
                    .as_ref()
                    .is_some_and(|source_id| accepted_ids.contains_key(source_id))
        });
    }

    Ok(reducer.rollout)
}

struct TraceReducer {
    rollout: RolloutTrace,
    payload_reader: PayloadReader,
    /// First (wall time, sequence) by receiving thread and Responses item ID.
    first_acceptances: BTreeMap<String, BTreeMap<String, (i64, crate::RawEventSeq)>>,
    /// Delivered items awaiting their first model-visible sighting, keyed by thread and wire ID.
    receipt_only_items: BTreeMap<(String, String), String>,
    next_conversation_item_ordinal: u64,
    next_terminal_operation_ordinal: u64,
    /// Latest session for each thread-local runtime handle; each exec replaces it.
    terminal_ids_by_process: BTreeMap<(String, String), String>,
    /// Retains the begin/end consistency check when a handle is later reused.
    process_ids_by_terminal: BTreeMap<String, String>,
    /// Latest request and its response items, used to reconcile the next request.
    thread_conversation_snapshots: BTreeMap<String, Vec<String>>,
    /// Replacement snapshot installed by compaction but not yet seen in a sampling request.
    ///
    /// The first full request after compaction should compare against the installed replacement
    /// history, not against the pre-compaction request. That keeps repeated prefix/context messages
    /// as fresh post-compaction conversation items while still reusing the summary/replacement
    /// items that actually became live history.
    pending_compaction_replacement_item_ids: BTreeMap<String, Vec<String>>,
    /// Runtime cell ids indexed by thread-local code-mode handle.
    ///
    /// Reduced `CodeCellId`s are based on the model-visible `exec` call id
    /// because that is the durable source identity. Runtime lifecycle, nested
    /// tools, and `wait` calls arrive with the runtime-local `cell_id`, so this
    /// index is the one intentional bridge between those namespaces.
    code_cell_ids_by_runtime: BTreeMap<(String, String), String>,
    /// A late observation from an older dispatch cannot identify a cell in the new runtime.
    last_thread_resume_seq: BTreeMap<String, crate::RawEventSeq>,
    /// Code-cell starts whose model-visible `custom_tool_call` item has not
    /// been reduced yet.
    ///
    /// Core begins executing tools before the stream-completion hook records
    /// the response payload that requested them. Queueing keeps replay strict
    /// about eventual source-item ownership without requiring trace producers
    /// to reorder runtime events behind inference completion.
    pending_code_cell_starts: BTreeMap<String, PendingCodeCellStart>,
    /// Initial/end events that arrived while the matching start was queued.
    ///
    /// Fast cells can return before the inference response payload that proves
    /// the model-visible `exec` source item has been reduced. The start remains
    /// queued for ownership validation; these lifecycle events wait with it and
    /// are replayed in raw sequence order once the cell materializes.
    pending_code_cell_lifecycle_events: BTreeMap<String, Vec<PendingCodeCellLifecycleEvent>>,
    /// Multi-agent deliveries whose recipient-side transcript item has not been observed yet.
    ///
    /// V2 agent tools enqueue mailbox messages in the target thread. The trace event for the
    /// sending tool arrives before the recipient inference request materializes that mailbox item
    /// as a `ConversationItem`, so the reducer keeps the delivery edge pending until it can point
    /// at the exact model-visible item instead of a coarse thread.
    pending_agent_interaction_edges: Vec<PendingAgentInteractionEdge>,
}

impl TraceReducer {
    fn apply_event(&mut self, event: RawTraceEvent) -> Result<()> {
        // Raw payload refs are reducer-wide evidence, not owned by a single
        // semantic arm. Keep this bookkeeping separate so typed reduction can
        // stay strict without duplicating payload insertion in every case.
        for payload in event
            .payload
            .raw_payload_refs()
            .into_iter()
            .flat_map(crate::RawPayloadRef::references)
        {
            let old = self
                .rollout
                .raw_payloads
                .insert(payload.raw_payload_id.clone(), payload.clone());
            ensure!(
                old.as_ref().is_none_or(|old| old == payload),
                "conflicting references for payload {}",
                payload.raw_payload_id
            );
        }

        match event.payload {
            RawTraceEventPayload::RecordingCompleted => {
                self.rollout.recording_status = if self
                    .rollout
                    .raw_payloads
                    .values()
                    .all(|p| p.capture.is_complete())
                {
                    RecordingStatus::Complete
                } else {
                    RecordingStatus::ClosedWithPartialPayloads
                };
            }
            RawTraceEventPayload::RolloutStarted {
                trace_id,
                root_thread_id,
            } => {
                self.rollout.trace_id = trace_id;
                self.rollout.root_thread_id = root_thread_id;
            }
            RawTraceEventPayload::RolloutEnded { status } => {
                self.rollout.status = status;
                self.rollout.ended_at_unix_ms = Some(event.wall_time_unix_ms);
            }
            RawTraceEventPayload::ThreadStarted {
                thread_id,
                agent_path,
                metadata_payload,
            } => {
                self.start_thread(
                    event.seq,
                    event.wall_time_unix_ms,
                    thread_id,
                    agent_path,
                    metadata_payload,
                )?;
            }
            RawTraceEventPayload::ThreadEnded { thread_id, status } => {
                self.end_thread(event.seq, event.wall_time_unix_ms, thread_id, status)?;
            }
            RawTraceEventPayload::ThreadResumed { thread_id } => {
                self.resume_thread(&thread_id, event.seq)?;
            }
            RawTraceEventPayload::CodexTurnStarted {
                codex_turn_id,
                thread_id,
            } => {
                self.start_codex_turn(
                    event.seq,
                    event.wall_time_unix_ms,
                    codex_turn_id,
                    thread_id,
                )?;
            }
            RawTraceEventPayload::CodexTurnEnded {
                codex_turn_id,
                status,
            } => {
                self.end_codex_turn(
                    event.seq,
                    event.wall_time_unix_ms,
                    event.thread_id,
                    codex_turn_id,
                    status,
                )?;
            }
            RawTraceEventPayload::HistoryItemAccepted {
                item_id,
                agent_message_payload,
            } => {
                let thread_id = event
                    .thread_id
                    .context("history acceptance has no thread")?;
                self.thread_mut(&thread_id)?;
                if let Some(payload) = agent_message_payload {
                    self.record_agent_receipt(
                        &thread_id,
                        event.codex_turn_id,
                        event.wall_time_unix_ms,
                        &item_id,
                        &payload,
                    )?;
                }
                self.first_acceptances
                    .entry(thread_id)
                    .or_default()
                    .entry(item_id)
                    .or_insert((event.wall_time_unix_ms, event.seq));
            }
            RawTraceEventPayload::InferenceStarted {
                inference_call_id,
                thread_id,
                codex_turn_id,
                model,
                provider_name,
                request_payload,
                prepared_prompt_payload,
                warmup_context_payload,
            } => {
                self.start_inference_call(
                    event.seq,
                    event.wall_time_unix_ms,
                    StartedInferenceCall {
                        inference_call_id,
                        thread_id,
                        codex_turn_id,
                        model,
                        provider_name,
                        request_payload,
                        prepared_prompt_payload,
                        warmup_context_payload,
                    },
                )?;
            }
            payload @ (RawTraceEventPayload::InferenceCompleted { .. }
            | RawTraceEventPayload::InferenceFailed { .. }
            | RawTraceEventPayload::InferenceCancelled { .. }) => {
                self.complete_inference_call(event.seq, event.wall_time_unix_ms, payload)?;
            }
            RawTraceEventPayload::ProtocolEventObserved { .. } => {
                // Protocol wrappers are raw debug breadcrumbs. Typed hooks own
                // the reduced graph, so these payload refs are retained without
                // creating semantic objects.
            }
            RawTraceEventPayload::ToolCallStarted {
                tool_call_id,
                model_visible_call_id,
                code_mode_runtime_tool_id,
                requester,
                kind,
                summary,
                invocation_payload,
            } => {
                self.start_tool_call(
                    event.seq,
                    event.wall_time_unix_ms,
                    event.thread_id,
                    event.codex_turn_id,
                    ToolCallStarted {
                        tool_call_id,
                        model_visible_call_id,
                        code_mode_runtime_tool_id,
                        requester,
                        kind,
                        summary,
                        invocation_payload,
                    },
                )?;
            }
            RawTraceEventPayload::McpToolCallCorrelationAssigned {
                tool_call_id,
                mcp_call_id,
            } => {
                self.assign_mcp_tool_call_correlation(tool_call_id, mcp_call_id)?;
            }
            RawTraceEventPayload::StdinTargetResolved {
                tool_call_id,
                exec_tool_call_id,
            } => {
                self.resolve_stdin_target(&tool_call_id, &exec_tool_call_id)?;
            }
            RawTraceEventPayload::CodeCellWaitTargetResolved {
                tool_call_id,
                runtime_cell_id,
            } => {
                self.link_confirmed_wait(
                    event
                        .thread_id
                        .as_deref()
                        .context("confirmed wait missing thread")?,
                    event
                        .codex_turn_id
                        .as_deref()
                        .context("confirmed wait missing turn")?,
                    tool_call_id,
                    runtime_cell_id,
                )?;
            }
            RawTraceEventPayload::ToolCallRuntimeStarted {
                tool_call_id,
                kind,
                runtime_payload,
            } => {
                self.start_tool_runtime_observation(
                    event.seq,
                    event.wall_time_unix_ms,
                    tool_call_id,
                    kind,
                    runtime_payload,
                )?;
            }
            RawTraceEventPayload::ToolCallRuntimeEnded {
                tool_call_id,
                status,
                runtime_payload,
            } => {
                self.end_tool_runtime_observation(
                    event.seq,
                    event.wall_time_unix_ms,
                    tool_call_id,
                    status,
                    runtime_payload,
                )?;
            }
            RawTraceEventPayload::ToolCallEnded {
                tool_call_id,
                status,
                result_payload,
            } => {
                self.end_tool_call(
                    event.seq,
                    event.wall_time_unix_ms,
                    tool_call_id,
                    status,
                    result_payload,
                )?;
            }
            RawTraceEventPayload::CodeCellStarted {
                runtime_cell_id,
                model_visible_call_id,
                source_js,
            } => {
                let thread_id = event.thread_id.with_context(|| {
                    format!("code cell start {runtime_cell_id} did not include a thread id")
                })?;
                let reduced_code_cell_id =
                    Self::reduced_code_cell_id_for_model_visible_call(&model_visible_call_id);
                self.record_runtime_code_cell_id(
                    &thread_id,
                    &runtime_cell_id,
                    &reduced_code_cell_id,
                )?;
                self.start_or_queue_code_cell(PendingCodeCellStart {
                    seq: event.seq,
                    wall_time_unix_ms: event.wall_time_unix_ms,
                    thread_id,
                    codex_turn_id: event.codex_turn_id,
                    wait_tool_call_ids: Vec::new(),
                    started: StartedCodeCell {
                        code_cell_id: reduced_code_cell_id,
                        runtime_cell_id,
                        model_visible_call_id,
                        source_js,
                    },
                })?;
            }
            RawTraceEventPayload::CodeCellInitialResponse {
                runtime_cell_id,
                status,
                response_payload,
            } => {
                let thread_id = event.thread_id.with_context(|| {
                    format!(
                        "code cell initial response {runtime_cell_id} did not include a thread id"
                    )
                })?;
                let code_cell_id = self.code_cell_id_for_runtime_cell_id(
                    &thread_id,
                    &runtime_cell_id,
                    "code cell initial response",
                )?;
                self.record_or_queue_code_cell_initial_response(
                    event.seq,
                    event.wall_time_unix_ms,
                    code_cell_id,
                    runtime_cell_id,
                    status,
                    response_payload,
                )?;
            }
            RawTraceEventPayload::CodeCellEnded {
                runtime_cell_id,
                status,
                response_payload,
            } => {
                let thread_id = event.thread_id.with_context(|| {
                    format!("code cell end {runtime_cell_id} did not include a thread id")
                })?;
                let code_cell_id = self.code_cell_id_for_runtime_cell_id(
                    &thread_id,
                    &runtime_cell_id,
                    "code cell end",
                )?;
                self.end_or_queue_code_cell(
                    event.seq,
                    event.wall_time_unix_ms,
                    code_cell_id,
                    status,
                    response_payload,
                )?;
            }
            RawTraceEventPayload::CompactionRequestStarted {
                compaction_id,
                compaction_request_id,
                thread_id,
                codex_turn_id,
                model,
                provider_name,
                request_payload,
            } => {
                self.start_compaction_request(
                    event.seq,
                    event.wall_time_unix_ms,
                    StartedCompactionRequest {
                        compaction_id,
                        compaction_request_id,
                        thread_id,
                        codex_turn_id,
                        model,
                        provider_name,
                        request_payload,
                    },
                )?;
            }
            RawTraceEventPayload::CompactionRequestCompleted {
                compaction_id,
                compaction_request_id,
                response_payload,
            } => {
                self.complete_compaction_request(
                    event.seq,
                    event.wall_time_unix_ms,
                    compaction_id,
                    compaction_request_id,
                    ExecutionStatus::Completed,
                    Some(response_payload),
                )?;
            }
            RawTraceEventPayload::CompactionRequestFailed {
                compaction_id,
                compaction_request_id,
                ..
            } => {
                self.complete_compaction_request(
                    event.seq,
                    event.wall_time_unix_ms,
                    compaction_id,
                    compaction_request_id,
                    ExecutionStatus::Failed,
                    /*response_payload*/ None,
                )?;
            }
            RawTraceEventPayload::CompactionInstalled {
                compaction_id,
                checkpoint_payload,
            } => {
                let Some(thread_id) = event.thread_id else {
                    bail!("compaction installed event {compaction_id} did not include a thread id");
                };
                let Some(codex_turn_id) = event.codex_turn_id else {
                    bail!(
                        "compaction installed event {compaction_id} did not include a codex turn id"
                    );
                };
                self.reduce_compaction_installed_event(
                    event.wall_time_unix_ms,
                    thread_id,
                    codex_turn_id,
                    compaction_id,
                    checkpoint_payload,
                )?;
            }
            RawTraceEventPayload::AgentResultObserved {
                edge_id,
                child_thread_id,
                child_codex_turn_id,
                parent_thread_id,
                message,
                carried_payload,
            } => {
                self.queue_agent_result_interaction_edge(ObservedAgentResultEdge {
                    wall_time_unix_ms: event.wall_time_unix_ms,
                    edge_id,
                    child_thread_id,
                    child_codex_turn_id,
                    parent_thread_id,
                    message,
                    carried_payload,
                })?;
            }
            RawTraceEventPayload::Other { .. } => {
                bail!("raw trace event has no reducer implementation");
            }
        }

        // Compaction appends input and replacement histories separately. Pending cells
        // must see both before finding their source and outputs in the ordered history.
        self.flush_pending_code_cell_starts()
    }
}
