//! Code-mode reduction.
//!
//! A code cell is the runtime parent for model-authored `exec`
//! JavaScript. Nested tools, waits, and terminal operations hang off this
//! object so viewers can inspect runtime work without flattening it into the
//! model-visible conversation.
//!
//! The reducer has to reconcile two clocks:
//! - model-visible items come from inference request/response payloads;
//! - runtime work starts as soon as Codex dispatches the tool.
//!
//! In real traces `CodeCellStarted` can arrive before the inference completion
//! payload that contains the `custom_tool_call` item. We therefore queue starts
//! until their source conversation item exists, then attach runtime edges.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde_json::Value;

use super::TraceReducer;
use crate::model::CodeCell;
use crate::model::CodeCellId;
use crate::model::CodeCellRuntimeStatus;
use crate::model::ConversationItemKind;
use crate::model::ExecutionStatus;
use crate::model::ExecutionWindow;
use crate::model::ProducerRef;
use crate::model::ToolCallId;
use crate::model::ToolCallRequester;
use crate::payload::RawPayloadRef;
use crate::raw_event::RawEventSeq;
use crate::raw_event::RawToolCallRequester;

/// Runtime start payload for one model-authored code-mode exec call.
///
/// The reduced id is already derived from the model-visible call id before this
/// reaches the code-cell reducer, so the reducer can reconcile runtime lifecycle
/// events against a stable graph identity.
pub(super) struct StartedCodeCell {
    pub(super) code_cell_id: CodeCellId,
    pub(super) runtime_cell_id: String,
    pub(super) model_visible_call_id: crate::model::ModelVisibleCallId,
    pub(super) source_js: String,
}

/// Queued code-cell start waiting for its model-visible source item.
///
/// Code execution can begin before inference stream completion records the
/// custom-tool call item that authored it. This wrapper keeps the original
/// event timing intact until that source item exists.
pub(super) struct PendingCodeCellStart {
    pub(super) seq: RawEventSeq,
    pub(super) wall_time_unix_ms: i64,
    pub(super) thread_id: String,
    pub(super) codex_turn_id: Option<String>,
    pub(super) started: StartedCodeCell,
}

/// Lifecycle event observed before a queued code cell has materialized.
///
/// These events are replayed after the start is resolved so failed or very fast
/// cells do not lose runtime status while preserving source-item ownership.
pub(super) struct PendingCodeCellLifecycleEvent {
    pub(super) seq: RawEventSeq,
    pub(super) wall_time_unix_ms: i64,
    pub(super) kind: PendingCodeCellLifecycleEventKind,
}

/// Runtime lifecycle transitions that can arrive while a code-cell start is queued.
pub(super) enum PendingCodeCellLifecycleEventKind {
    InitialResponse {
        runtime_cell_id: String,
        status: CodeCellRuntimeStatus,
    },
    Ended {
        status: CodeCellRuntimeStatus,
    },
}

impl TraceReducer {
    /// Starts a code cell once its model-visible source item exists.
    ///
    /// Runtime events are allowed to arrive before stream completion has
    /// reduced the model output that requested `exec`. Queueing preserves the
    /// event order while still requiring every final `CodeCell` to point at the
    /// exact conversation item that authored its JavaScript.
    pub(super) fn start_or_queue_code_cell(&mut self, pending: PendingCodeCellStart) -> Result<()> {
        let code_cell_id = pending.started.code_cell_id.clone();
        if self
            .source_item_id_for_pending_code_cell(&pending)?
            .is_none()
        {
            if self.rollout.code_cells.contains_key(&code_cell_id)
                || self.pending_code_cell_starts.contains_key(&code_cell_id)
            {
                bail!("duplicate code cell start for {code_cell_id}");
            }
            self.pending_code_cell_starts.insert(code_cell_id, pending);
            return Ok(());
        }

        self.start_code_cell(pending)
    }

    /// Materializes any queued code-cell starts unlocked by newly reduced conversation items.
    ///
    /// This is called after inference and compaction conversation reduction,
    /// because those are the only paths that create model-visible items today.
    pub(super) fn flush_pending_code_cell_starts(&mut self) -> Result<()> {
        let mut ready_ids = Vec::new();
        for (code_cell_id, pending) in &self.pending_code_cell_starts {
            if self
                .source_item_id_for_pending_code_cell(pending)?
                .is_some()
            {
                ready_ids.push(code_cell_id.clone());
            }
        }

        for code_cell_id in ready_ids {
            let Some(pending) = self.pending_code_cell_starts.remove(&code_cell_id) else {
                continue;
            };
            self.start_code_cell(pending)?;
        }
        Ok(())
    }

    /// Inserts the reduced `CodeCell` once source ownership can be proven.
    fn start_code_cell(&mut self, pending: PendingCodeCellStart) -> Result<()> {
        let PendingCodeCellStart {
            seq,
            wall_time_unix_ms,
            thread_id,
            codex_turn_id,
            started,
        } = pending;
        if self.rollout.code_cells.contains_key(&started.code_cell_id) {
            bail!("duplicate code cell start for {}", started.code_cell_id);
        }

        let Some(codex_turn_id) = codex_turn_id else {
            bail!(
                "code cell start {} did not include a Codex turn id",
                started.code_cell_id
            );
        };
        self.validate_code_cell_turn(&thread_id, &codex_turn_id)?;

        let source_item_id = self.source_item_id_for_code_cell_start(
            &thread_id,
            &started.code_cell_id,
            &started.model_visible_call_id,
        )?;
        let output_item_ids = self.model_visible_code_cell_item_ids(
            &thread_id,
            &started.model_visible_call_id,
            ConversationItemKind::CustomToolCallOutput,
        );
        // Runtime events may also have arrived while the start was queued.
        // Seed these reverse links from already-reduced tool calls so replay is
        // order-insensitive within the known trace causality.
        let requester = ToolCallRequester::CodeCell {
            code_cell_id: started.code_cell_id.clone(),
        };
        let nested_tool_call_ids = self
            .rollout
            .tool_calls
            .values()
            .filter(|tool_call| tool_call.requester == requester)
            .map(|tool_call| tool_call.tool_call_id.clone())
            .collect();

        self.rollout.code_cells.insert(
            started.code_cell_id.clone(),
            CodeCell {
                code_cell_id: started.code_cell_id.clone(),
                model_visible_call_id: started.model_visible_call_id,
                thread_id: thread_id.clone(),
                codex_turn_id,
                source_item_id,
                output_item_ids: output_item_ids.clone(),
                runtime_cell_id: Some(started.runtime_cell_id),
                execution: ExecutionWindow {
                    started_at_unix_ms: wall_time_unix_ms,
                    started_seq: seq,
                    ended_at_unix_ms: None,
                    ended_seq: None,
                    status: ExecutionStatus::Running,
                },
                runtime_status: CodeCellRuntimeStatus::Starting,
                initial_response_at_unix_ms: None,
                initial_response_seq: None,
                yielded_at_unix_ms: None,
                yielded_seq: None,
                source_js: started.source_js,
                nested_tool_call_ids,
                wait_tool_call_ids: Vec::new(),
            },
        );

        self.thread_mut(&thread_id)?;

        for item_id in output_item_ids {
            self.add_code_cell_output_item(&started.code_cell_id, &item_id)?;
        }
        self.flush_pending_code_cell_lifecycle_events(&started.code_cell_id)?;

        Ok(())
    }

    /// Returns the source item if the model-visible `exec` call has been reduced.
    fn source_item_id_for_pending_code_cell(
        &self,
        pending: &PendingCodeCellStart,
    ) -> Result<Option<String>> {
        Ok(self
            .model_visible_code_cell_item_ids(
                &pending.thread_id,
                &pending.started.model_visible_call_id,
                ConversationItemKind::CustomToolCall,
            )
            .into_iter()
            .next())
    }

    /// Records the runtime's first response for a code cell, or waits for its source item.
    ///
    /// Code-mode execution can start and fail before the inference response payload
    /// that introduced the model-visible `exec` call has been reduced. In that
    /// case the cell start is already pending; keep the lifecycle event beside it
    /// instead of weakening the invariant that every reduced cell has a source
    /// conversation item.
    pub(super) fn record_or_queue_code_cell_initial_response(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        code_cell_id: CodeCellId,
        runtime_cell_id: String,
        status: CodeCellRuntimeStatus,
    ) -> Result<()> {
        if !self.rollout.code_cells.contains_key(&code_cell_id) {
            if self.pending_code_cell_starts.contains_key(&code_cell_id) {
                self.queue_code_cell_lifecycle_event(
                    code_cell_id,
                    PendingCodeCellLifecycleEvent {
                        seq,
                        wall_time_unix_ms,
                        kind: PendingCodeCellLifecycleEventKind::InitialResponse {
                            runtime_cell_id,
                            status,
                        },
                    },
                );
                return Ok(());
            }
            bail!("code cell initial response referenced unknown cell {code_cell_id}");
        }
        self.record_code_cell_initial_response(
            seq,
            wall_time_unix_ms,
            code_cell_id,
            runtime_cell_id,
            status,
        )
    }

    fn record_code_cell_initial_response(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        code_cell_id: CodeCellId,
        runtime_cell_id: String,
        status: CodeCellRuntimeStatus,
    ) -> Result<()> {
        let Some(cell) = self.rollout.code_cells.get_mut(&code_cell_id) else {
            bail!("code cell initial response referenced unknown cell {code_cell_id}");
        };

        cell.runtime_cell_id = Some(runtime_cell_id);
        if cell.initial_response_at_unix_ms.is_none() {
            cell.initial_response_at_unix_ms = Some(wall_time_unix_ms);
            cell.initial_response_seq = Some(seq);
        }
        if status == CodeCellRuntimeStatus::Yielded {
            cell.yielded_at_unix_ms = Some(wall_time_unix_ms);
            cell.yielded_seq = Some(seq);
        }
        cell.runtime_status = status;
        Ok(())
    }

    /// Ends a code cell, or waits until its queued start can materialize.
    ///
    /// This mirrors `record_or_queue_code_cell_initial_response`: the reducer is
    /// strict about unknown cells, but a cell whose start is pending on the
    /// model-visible source item is known and just needs its lifecycle replayed
    /// after the source item appears.
    pub(super) fn end_or_queue_code_cell(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        code_cell_id: CodeCellId,
        status: CodeCellRuntimeStatus,
    ) -> Result<()> {
        if !self.rollout.code_cells.contains_key(&code_cell_id) {
            if self.pending_code_cell_starts.contains_key(&code_cell_id) {
                self.queue_code_cell_lifecycle_event(
                    code_cell_id,
                    PendingCodeCellLifecycleEvent {
                        seq,
                        wall_time_unix_ms,
                        kind: PendingCodeCellLifecycleEventKind::Ended { status },
                    },
                );
                return Ok(());
            }
            bail!("code cell end referenced unknown cell {code_cell_id}");
        }
        self.end_code_cell(seq, wall_time_unix_ms, code_cell_id, status)
    }

    fn end_code_cell(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        code_cell_id: CodeCellId,
        status: CodeCellRuntimeStatus,
    ) -> Result<()> {
        let Some(cell) = self.rollout.code_cells.get_mut(&code_cell_id) else {
            bail!("code cell end referenced unknown cell {code_cell_id}");
        };

        if cell.initial_response_at_unix_ms.is_none() {
            cell.initial_response_at_unix_ms = Some(wall_time_unix_ms);
            cell.initial_response_seq = Some(seq);
        }
        cell.execution.ended_at_unix_ms = Some(wall_time_unix_ms);
        cell.execution.ended_seq = Some(seq);
        cell.execution.status = execution_status_for_code_cell(&status);
        cell.runtime_status = status;
        Ok(())
    }

    /// Closes unfinished code cells when their owning turn is interrupted.
    ///
    /// A yielded code cell can outlive a completed turn and be resumed by a
    /// later `wait`, so normal turn completion must not imply cell completion.
    /// Cancellation/failure is different: the model-visible JS frame has been
    /// abandoned even if nested terminal work reports late runtime events. In
    /// that case leaving the cell `running` makes a completed trace look live.
    pub(super) fn terminate_running_code_cells_for_turn_end(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        codex_turn_id: &str,
        turn_status: &ExecutionStatus,
    ) -> Result<()> {
        let runtime_status = match turn_status {
            ExecutionStatus::Running | ExecutionStatus::Completed => return Ok(()),
            ExecutionStatus::Failed => CodeCellRuntimeStatus::Failed,
            ExecutionStatus::Cancelled | ExecutionStatus::Aborted => {
                CodeCellRuntimeStatus::Terminated
            }
        };
        let code_cell_ids: Vec<_> = self
            .rollout
            .code_cells
            .values()
            .filter(|cell| {
                cell.codex_turn_id == codex_turn_id
                    && cell.execution.status == ExecutionStatus::Running
            })
            .map(|cell| cell.code_cell_id.clone())
            .collect();

        for code_cell_id in code_cell_ids {
            self.end_code_cell(seq, wall_time_unix_ms, code_cell_id, runtime_status.clone())?;
        }
        Ok(())
    }

    fn queue_code_cell_lifecycle_event(
        &mut self,
        code_cell_id: CodeCellId,
        event: PendingCodeCellLifecycleEvent,
    ) {
        let events = self
            .pending_code_cell_lifecycle_events
            .entry(code_cell_id)
            .or_default();
        events.push(event);
        events.sort_by_key(|event| event.seq);
    }

    fn flush_pending_code_cell_lifecycle_events(&mut self, code_cell_id: &str) -> Result<()> {
        let Some(events) = self.pending_code_cell_lifecycle_events.remove(code_cell_id) else {
            return Ok(());
        };
        for event in events {
            match event.kind {
                PendingCodeCellLifecycleEventKind::InitialResponse {
                    runtime_cell_id,
                    status,
                } => self.record_code_cell_initial_response(
                    event.seq,
                    event.wall_time_unix_ms,
                    code_cell_id.to_string(),
                    runtime_cell_id,
                    status,
                )?,
                PendingCodeCellLifecycleEventKind::Ended { status } => self.end_code_cell(
                    event.seq,
                    event.wall_time_unix_ms,
                    code_cell_id.to_string(),
                    status,
                )?,
            }
        }
        Ok(())
    }

    /// Links a nested tool call back to its parent code cell.
    ///
    /// If the parent cell is still queued, the link is recovered later from already
    /// reduced tool calls when the cell materializes.
    pub(super) fn link_tool_call_to_code_cell(
        &mut self,
        tool_call_id: &ToolCallId,
        requester: &ToolCallRequester,
    ) -> Result<()> {
        let ToolCallRequester::CodeCell { code_cell_id } = requester else {
            return Ok(());
        };
        let Some(cell) = self.rollout.code_cells.get_mut(code_cell_id) else {
            // The cell start may still be queued behind the inference payload
            // that contains its model-visible source item. `start_code_cell`
            // backfills these already-reduced nested calls once the source
            // ownership can be proven.
            return Ok(());
        };
        push_unique(&mut cell.nested_tool_call_ids, tool_call_id);
        Ok(())
    }

    /// Records that a model-visible wait call is waiting on a runtime code cell.
    ///
    /// Wait calls are not nested JavaScript tools, so the relationship is inferred
    /// from the runtime cell id inside the function arguments.
    pub(super) fn link_wait_tool_call_from_request_payload(
        &mut self,
        thread_id: &str,
        tool_call_id: &ToolCallId,
        request_payload: Option<&RawPayloadRef>,
    ) -> Result<()> {
        let Some(request_payload) = request_payload else {
            return Ok(());
        };
        let payload = self.read_payload_json(request_payload)?;
        if payload.get("tool_name").and_then(Value::as_str) != Some("wait") {
            return Ok(());
        }
        // `wait` is a normal model-visible function call, not a nested JS tool
        // request. The only stable edge back to the code cell is the runtime
        // `cell_id` inside the function arguments.
        let Some(arguments) = payload
            .get("payload")
            .and_then(|payload| payload.get("arguments"))
            .and_then(Value::as_str)
        else {
            bail!(
                "wait tool request payload {} did not contain function arguments",
                request_payload.raw_payload_id
            );
        };
        let arguments: Value = serde_json::from_str(arguments).with_context(|| {
            format!(
                "wait tool request payload {} had invalid JSON arguments",
                request_payload.raw_payload_id
            )
        })?;
        let Some(runtime_cell_id) = arguments.get("cell_id").and_then(Value::as_str) else {
            bail!(
                "wait tool request payload {} did not contain cell_id",
                request_payload.raw_payload_id
            );
        };
        let Some(code_cell_id) =
            self.code_cell_id_for_runtime_cell_id_if_known(thread_id, runtime_cell_id)
        else {
            return Ok(());
        };
        let Some(cell) = self.rollout.code_cells.get_mut(&code_cell_id) else {
            return Ok(());
        };
        push_unique(&mut cell.wait_tool_call_ids, tool_call_id);
        Ok(())
    }

    /// Attaches a later-observed model-visible output item to its code cell.
    ///
    /// This is used when an inference request carries a custom-tool output after
    /// the runtime cell already exists.
    pub(super) fn attach_model_visible_code_cell_item(
        &mut self,
        item_id: &str,
        call_id: Option<&str>,
        kind: &ConversationItemKind,
    ) -> Result<()> {
        let Some(call_id) = call_id else {
            return Ok(());
        };
        if *kind != ConversationItemKind::CustomToolCallOutput {
            return Ok(());
        }
        // The output item can be observed after the CodeCell was created, e.g.
        // when a later inference request carries the custom-tool result back to
        // the model. Add the reverse ProducerRef at that later observation
        // point instead of copying runtime bytes into the conversation model.
        let code_cell_id = self.reduced_code_cell_id_for_model_visible_call(call_id);
        if !self.rollout.code_cells.contains_key(&code_cell_id) {
            return Ok(());
        }
        self.add_code_cell_output_item(&code_cell_id, item_id)
    }

    /// Resolves the owning thread for a code-cell runtime event.
    ///
    /// Runtime events should carry a thread id, but older/raw paths may only have
    /// the turn id. The fallback keeps replay strict while avoiding duplicate logic
    /// in every code-cell event arm.
    pub(super) fn code_cell_event_thread_id(
        &self,
        thread_id: Option<String>,
        codex_turn_id: Option<&str>,
        runtime_cell_id: &str,
        event_name: &str,
    ) -> Result<String> {
        if let Some(thread_id) = thread_id {
            return Ok(thread_id);
        }
        let Some(codex_turn_id) = codex_turn_id else {
            bail!("{event_name} {runtime_cell_id} did not include a thread id");
        };
        self.rollout
            .codex_turns
            .get(codex_turn_id)
            .map(|turn| turn.thread_id.clone())
            .with_context(|| {
                format!(
                    "{event_name} {runtime_cell_id} referenced unknown Codex turn {codex_turn_id}"
                )
            })
    }

    /// Derives the stable reduced code-cell id from the model-visible exec call id.
    pub(super) fn reduced_code_cell_id_for_model_visible_call(
        &self,
        model_visible_call_id: &str,
    ) -> CodeCellId {
        // The model-visible `exec` call is the durable source identity. The
        // runtime `cell_id` is only a thread-local handle used for later waits
        // and nested tool calls.
        format!("code_cell:{model_visible_call_id}")
    }

    /// Records the thread-local runtime cell id to reduced code-cell id mapping.
    ///
    /// Runtime ids can repeat across threads, so callers must provide the owning
    /// thread id when creating or resolving this bridge.
    pub(super) fn record_runtime_code_cell_id(
        &mut self,
        thread_id: &str,
        runtime_cell_id: &str,
        code_cell_id: &str,
    ) -> Result<()> {
        let key = runtime_code_cell_key(thread_id, runtime_cell_id);
        if let Some(existing) = self.code_cell_ids_by_runtime.get(&key) {
            if existing == code_cell_id {
                return Ok(());
            }
            bail!(
                "runtime code cell {runtime_cell_id} in thread {thread_id} mapped to both \
                 {existing} and {code_cell_id}"
            );
        }
        self.code_cell_ids_by_runtime
            .insert(key, code_cell_id.to_string());
        Ok(())
    }

    /// Resolves a runtime cell id to the reduced code-cell id for the given thread.
    pub(super) fn code_cell_id_for_runtime_cell_id(
        &self,
        thread_id: &str,
        runtime_cell_id: &str,
        event_name: &str,
    ) -> Result<CodeCellId> {
        self.code_cell_id_for_runtime_cell_id_if_known(thread_id, runtime_cell_id)
            .with_context(|| {
                format!(
                    "{event_name} referenced unknown runtime cell {runtime_cell_id} \
                     in thread {thread_id}"
                )
            })
    }

    fn code_cell_id_for_runtime_cell_id_if_known(
        &self,
        thread_id: &str,
        runtime_cell_id: &str,
    ) -> Option<CodeCellId> {
        self.code_cell_ids_by_runtime
            .get(&runtime_code_cell_key(thread_id, runtime_cell_id))
            .cloned()
    }

    /// Converts a raw tool requester into the reduced graph requester.
    ///
    /// Code-mode tool requests arrive with a runtime cell id, so this method is
    /// the boundary that turns that runtime handle into a stable code-cell anchor.
    pub(super) fn reduce_tool_call_requester(
        &self,
        thread_id: &str,
        requester: RawToolCallRequester,
    ) -> Result<ToolCallRequester> {
        match requester {
            RawToolCallRequester::Model => Ok(ToolCallRequester::Model),
            RawToolCallRequester::CodeCell { runtime_cell_id } => Ok(ToolCallRequester::CodeCell {
                code_cell_id: self.code_cell_id_for_runtime_cell_id(
                    thread_id,
                    &runtime_cell_id,
                    "code-mode nested tool",
                )?,
            }),
        }
    }

    fn validate_code_cell_turn(&self, thread_id: &str, codex_turn_id: &str) -> Result<()> {
        if !self.rollout.threads.contains_key(thread_id) {
            bail!("code cell start referenced unknown thread {thread_id}");
        }
        let Some(turn) = self.rollout.codex_turns.get(codex_turn_id) else {
            bail!("code cell start referenced unknown Codex turn {codex_turn_id}");
        };
        if turn.thread_id != thread_id {
            bail!(
                "code cell start used thread {thread_id}, but Codex turn {codex_turn_id} belongs \
                 to {}",
                turn.thread_id
            );
        }
        Ok(())
    }

    fn model_visible_code_cell_item_ids(
        &self,
        thread_id: &str,
        call_id: &str,
        kind: ConversationItemKind,
    ) -> Vec<String> {
        self.rollout
            .conversation_items
            .values()
            .filter(|item| {
                item.thread_id == thread_id
                    && item.call_id.as_deref() == Some(call_id)
                    && item.kind == kind
            })
            .map(|item| item.item_id.clone())
            .collect()
    }

    fn source_item_id_for_code_cell_start(
        &self,
        thread_id: &str,
        code_cell_id: &str,
        model_visible_call_id: &str,
    ) -> Result<String> {
        self.model_visible_code_cell_item_ids(
            thread_id,
            model_visible_call_id,
            ConversationItemKind::CustomToolCall,
        )
        .into_iter()
        .next()
        .with_context(|| {
            format!(
                "code cell {code_cell_id} referenced model-visible call {model_visible_call_id}, \
                 but no custom tool call item was observed"
            )
        })
    }

    fn add_code_cell_output_item(&mut self, code_cell_id: &str, item_id: &str) -> Result<()> {
        let Some(cell) = self.rollout.code_cells.get_mut(code_cell_id) else {
            bail!("code cell {code_cell_id} disappeared during output linking");
        };
        push_unique(&mut cell.output_item_ids, item_id);

        let Some(item) = self.rollout.conversation_items.get_mut(item_id) else {
            bail!("conversation item {item_id} disappeared during code-cell output linking");
        };
        let producer = ProducerRef::CodeCell {
            code_cell_id: code_cell_id.to_string(),
        };
        if !item.produced_by.contains(&producer) {
            item.produced_by.push(producer);
        }
        Ok(())
    }
}

fn execution_status_for_code_cell(status: &CodeCellRuntimeStatus) -> ExecutionStatus {
    match status {
        CodeCellRuntimeStatus::Starting
        | CodeCellRuntimeStatus::Running
        | CodeCellRuntimeStatus::Yielded => ExecutionStatus::Running,
        CodeCellRuntimeStatus::Completed => ExecutionStatus::Completed,
        CodeCellRuntimeStatus::Failed => ExecutionStatus::Failed,
        CodeCellRuntimeStatus::Terminated => ExecutionStatus::Cancelled,
    }
}

fn push_unique(items: &mut Vec<String>, item_id: &str) {
    if !items.iter().any(|existing| existing == item_id) {
        items.push(item_id.to_string());
    }
}

fn runtime_code_cell_key(thread_id: &str, runtime_cell_id: &str) -> (String, String) {
    (thread_id.to_string(), runtime_cell_id.to_string())
}

#[cfg(test)]
#[path = "code_cell_tests.rs"]
mod tests;
