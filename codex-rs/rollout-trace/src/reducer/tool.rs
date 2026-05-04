use anyhow::Context;
use anyhow::Result;
use anyhow::bail;

use super::TraceReducer;
use crate::model::CodeModeRuntimeToolId;
use crate::model::ConversationItemKind;
use crate::model::ExecutionStatus;
use crate::model::ExecutionWindow;
use crate::model::ModelVisibleCallId;
use crate::model::ProducerRef;
use crate::model::ToolCall;
use crate::model::ToolCallId;
use crate::model::ToolCallKind;
use crate::model::ToolCallSummary;
use crate::payload::RawPayloadRef;
use crate::raw_event::RawEventSeq;
use crate::raw_event::RawToolCallRequester;

mod agents;
mod terminal;

pub(super) use agents::ObservedAgentResultEdge;
pub(super) use agents::PendingAgentInteractionEdge;
pub(super) use agents::spawn_edge_id;

/// Raw tool-start fields after dispatch has stripped the common event envelope.
///
/// Tool starts carry several optional identity namespaces: model-visible calls,
/// code-mode runtime tools, and canonical invocation payloads. Grouping them keeps
/// the reducer callsite readable and avoids positional argument mistakes.
pub(super) struct ToolCallStarted {
    pub(super) tool_call_id: ToolCallId,
    pub(super) model_visible_call_id: Option<ModelVisibleCallId>,
    pub(super) code_mode_runtime_tool_id: Option<CodeModeRuntimeToolId>,
    pub(super) requester: RawToolCallRequester,
    pub(super) kind: ToolCallKind,
    pub(super) summary: ToolCallSummary,
    pub(super) invocation_payload: Option<RawPayloadRef>,
}

impl TraceReducer {
    /// Starts a tool call and links it to model-visible items or runtime parents when available.
    ///
    /// Some tools also create richer domain objects, such as terminal operations, from
    /// the same invocation payload. The generic ToolCall remains the common index.
    pub(super) fn start_tool_call(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        thread_id: Option<String>,
        codex_turn_id: Option<String>,
        started: ToolCallStarted,
    ) -> Result<()> {
        let tool_call_id = started.tool_call_id.clone();
        if self.rollout.tool_calls.contains_key(&tool_call_id) {
            bail!("duplicate tool call start for {tool_call_id}");
        }
        self.ensure_unique_model_visible_tool_call(
            started.model_visible_call_id.as_deref(),
            &tool_call_id,
        )?;

        let thread_id = self.tool_thread_id(thread_id, codex_turn_id.as_deref())?;
        self.validate_tool_turn(&thread_id, codex_turn_id.as_deref())?;

        let model_visible_call_id = started.model_visible_call_id.clone();
        let requester = self.reduce_tool_call_requester(&thread_id, started.requester.clone())?;
        let model_visible_call_item_ids = model_visible_call_id
            .as_deref()
            .map(|call_id| {
                self.model_visible_tool_item_ids(
                    &thread_id,
                    call_id,
                    &[
                        ConversationItemKind::FunctionCall,
                        ConversationItemKind::CustomToolCall,
                    ],
                )
            })
            .unwrap_or_default();
        let model_visible_output_item_ids = model_visible_call_id
            .as_deref()
            .map(|call_id| {
                self.model_visible_tool_item_ids(
                    &thread_id,
                    call_id,
                    &[
                        ConversationItemKind::FunctionCallOutput,
                        ConversationItemKind::CustomToolCallOutput,
                    ],
                )
            })
            .unwrap_or_default();

        self.thread_mut(&thread_id)?;

        // Some terminal-like tools, notably write_stdin, do not emit a richer
        // runtime begin event. For those tools the canonical invocation is the
        // only place to recover the terminal/session join key.
        let terminal_operation_id = self.start_terminal_operation_from_invocation(
            seq,
            wall_time_unix_ms,
            &thread_id,
            &tool_call_id,
            &started.kind,
            started.invocation_payload.as_ref(),
        )?;
        // Terminal-backed tools should render through the richer terminal
        // operation instead of the generic tool summary captured by producers.
        let summary = terminal_operation_id
            .as_ref()
            .map(|operation_id| ToolCallSummary::Terminal {
                operation_id: operation_id.clone(),
            })
            .unwrap_or(started.summary);
        let raw_invocation_payload_id = started
            .invocation_payload
            .as_ref()
            .map(|payload| payload.raw_payload_id.clone());
        self.link_wait_tool_call_from_request_payload(
            &thread_id,
            &tool_call_id,
            started.invocation_payload.as_ref(),
        )?;

        self.rollout.tool_calls.insert(
            tool_call_id.clone(),
            ToolCall {
                tool_call_id: tool_call_id.clone(),
                model_visible_call_id,
                code_mode_runtime_tool_id: started.code_mode_runtime_tool_id,
                thread_id,
                started_by_codex_turn_id: codex_turn_id,
                execution: ExecutionWindow {
                    started_at_unix_ms: wall_time_unix_ms,
                    started_seq: seq,
                    ended_at_unix_ms: None,
                    ended_seq: None,
                    status: ExecutionStatus::Running,
                },
                requester: requester.clone(),
                kind: started.kind,
                model_visible_call_item_ids,
                model_visible_output_item_ids: Vec::new(),
                terminal_operation_id,
                summary,
                raw_invocation_payload_id,
                raw_result_payload_id: None,
                raw_runtime_payload_ids: Vec::new(),
            },
        );

        self.link_tool_call_to_code_cell(&tool_call_id, &requester)?;
        self.link_tool_to_inference_response(&tool_call_id);
        // Output items need the reverse ProducerRef edge as well, so attach
        // them after insertion through the same helper used by the transcript
        // reducer when the output is observed after the tool start.
        for item_id in model_visible_output_item_ids {
            self.add_tool_output_item(&tool_call_id, &item_id)?;
        }
        // The call/output items may have been observed before this tool start.
        // Re-sync after insertion so terminal observations get both directions
        // of the model-visible link.
        self.sync_terminal_model_observation(&tool_call_id)?;
        Ok(())
    }

    /// Completes the canonical tool call and any terminal operation driven by dispatch output.
    ///
    /// Protocol-backed terminal tools end from runtime events; direct tools
    /// may only have the canonical result payload, so this method handles both paths.
    pub(super) fn end_tool_call(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        tool_call_id: ToolCallId,
        status: ExecutionStatus,
        result_payload: Option<RawPayloadRef>,
    ) -> Result<()> {
        let (terminal_operation_id, thread_id, end_terminal_from_result) = {
            let Some(tool_call) = self.rollout.tool_calls.get_mut(&tool_call_id) else {
                bail!("tool call end referenced unknown call {tool_call_id}");
            };
            tool_call.execution.ended_at_unix_ms = Some(wall_time_unix_ms);
            tool_call.execution.ended_seq = Some(seq);
            tool_call.execution.status = status.clone();
            tool_call.raw_result_payload_id = result_payload
                .as_ref()
                .map(|payload| payload.raw_payload_id.clone());
            (
                tool_call.terminal_operation_id.clone(),
                tool_call.thread_id.clone(),
                // Protocol-backed tools end terminal operations from
                // runtime observations. Dispatch result payloads are still kept
                // on ToolCall, but they are caller-facing and may be transformed
                // relative to the raw terminal output.
                tool_call.raw_runtime_payload_ids.is_empty(),
            )
        };
        if end_terminal_from_result && let Some(operation_id) = terminal_operation_id {
            self.end_terminal_operation(
                seq,
                wall_time_unix_ms,
                &thread_id,
                &operation_id,
                status,
                result_payload.as_ref(),
            )?;
        }
        self.attach_agent_interaction_tool_result(&tool_call_id, result_payload.as_ref())?;
        Ok(())
    }

    /// Records a runtime-begin observation for an already started tool call.
    ///
    /// Runtime observations enrich the generic tool with protocol facts and may
    /// create domain-specific children such as terminal operations or agent edges.
    pub(super) fn start_tool_runtime_observation(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        tool_call_id: ToolCallId,
        runtime_payload: RawPayloadRef,
    ) -> Result<()> {
        let (thread_id, _requester, kind, existing_terminal_operation_id) = {
            let Some(tool_call) = self.rollout.tool_calls.get_mut(&tool_call_id) else {
                bail!("tool runtime start referenced unknown call {tool_call_id}");
            };
            push_unique(
                &mut tool_call.raw_runtime_payload_ids,
                &runtime_payload.raw_payload_id,
            );
            (
                tool_call.thread_id.clone(),
                tool_call.requester.clone(),
                tool_call.kind.clone(),
                tool_call.terminal_operation_id.clone(),
            )
        };
        if existing_terminal_operation_id.is_some()
            && matches!(kind, ToolCallKind::ExecCommand | ToolCallKind::WriteStdin)
        {
            bail!("tool runtime start would create a second terminal operation for {tool_call_id}");
        }

        // Protocol begin events carry runtime facts such as process ids and
        // cwd. These facts should create terminal rows, but they must not
        // replace the canonical invocation payload captured at dispatch.
        let terminal_operation_id = self.start_terminal_operation_from_runtime(
            seq,
            wall_time_unix_ms,
            &thread_id,
            &tool_call_id,
            &kind,
            &runtime_payload,
        )?;

        if let Some(operation_id) = &terminal_operation_id {
            let Some(tool_call) = self.rollout.tool_calls.get_mut(&tool_call_id) else {
                bail!("tool call {tool_call_id} disappeared during runtime start reduction");
            };
            if tool_call.terminal_operation_id.is_none() {
                tool_call.terminal_operation_id = Some(operation_id.clone());
                tool_call.summary = ToolCallSummary::Terminal {
                    operation_id: operation_id.clone(),
                };
            }
        }

        if terminal_operation_id.is_some() {
            self.sync_terminal_model_observation(&tool_call_id)?;
        }
        self.start_agent_interaction_from_runtime(&tool_call_id, &runtime_payload)?;
        Ok(())
    }

    /// Records a runtime-end observation for an already started tool call.
    pub(super) fn end_tool_runtime_observation(
        &mut self,
        seq: RawEventSeq,
        wall_time_unix_ms: i64,
        tool_call_id: ToolCallId,
        status: ExecutionStatus,
        runtime_payload: RawPayloadRef,
    ) -> Result<()> {
        let (thread_id, terminal_operation_id) = {
            let Some(tool_call) = self.rollout.tool_calls.get_mut(&tool_call_id) else {
                bail!("tool runtime end referenced unknown call {tool_call_id}");
            };
            push_unique(
                &mut tool_call.raw_runtime_payload_ids,
                &runtime_payload.raw_payload_id,
            );
            (
                tool_call.thread_id.clone(),
                tool_call.terminal_operation_id.clone(),
            )
        };

        if let Some(operation_id) = terminal_operation_id {
            self.end_terminal_operation(
                seq,
                wall_time_unix_ms,
                &thread_id,
                &operation_id,
                status,
                Some(&runtime_payload),
            )?;
        }
        self.end_agent_interaction_from_runtime(
            wall_time_unix_ms,
            &tool_call_id,
            &runtime_payload,
        )?;
        Ok(())
    }

    /// Attaches a conversation item observed after the tool call was reduced.
    ///
    /// Inference request/response ordering can expose call/output items after the
    /// runtime tool object exists, so transcript reduction calls back here to add
    /// reverse links without duplicating matching logic.
    pub(super) fn attach_model_visible_tool_item(
        &mut self,
        item_id: &str,
        call_id: Option<&str>,
        kind: &ConversationItemKind,
    ) -> Result<()> {
        let Some(call_id) = call_id else {
            return Ok(());
        };
        match kind {
            ConversationItemKind::FunctionCall | ConversationItemKind::CustomToolCall => {
                if let Some(tool_call_id) = self.single_tool_for_model_visible_call(call_id)? {
                    self.add_tool_call_item(&tool_call_id, item_id)?;
                    self.link_tool_to_inference_response(&tool_call_id);
                    self.sync_terminal_model_observation(&tool_call_id)?;
                }
            }
            ConversationItemKind::FunctionCallOutput
            | ConversationItemKind::CustomToolCallOutput => {
                if let Some(tool_call_id) = self.single_tool_for_model_visible_call(call_id)? {
                    self.add_tool_output_item(&tool_call_id, item_id)?;
                    self.sync_terminal_model_observation(&tool_call_id)?;
                }
            }
            ConversationItemKind::Message
            | ConversationItemKind::Reasoning
            | ConversationItemKind::CompactionMarker => {}
        }
        Ok(())
    }

    fn tool_thread_id(
        &self,
        thread_id: Option<String>,
        codex_turn_id: Option<&str>,
    ) -> Result<String> {
        if let Some(thread_id) = thread_id {
            return Ok(thread_id);
        }
        let Some(codex_turn_id) = codex_turn_id else {
            bail!("tool call start did not include thread or Codex turn context");
        };
        self.rollout
            .codex_turns
            .get(codex_turn_id)
            .map(|turn| turn.thread_id.clone())
            .with_context(|| {
                format!("tool call start referenced unknown Codex turn {codex_turn_id}")
            })
    }

    fn validate_tool_turn(&self, thread_id: &str, codex_turn_id: Option<&str>) -> Result<()> {
        if !self.rollout.threads.contains_key(thread_id) {
            bail!("tool call start referenced unknown thread {thread_id}");
        }
        if let Some(codex_turn_id) = codex_turn_id {
            let Some(turn) = self.rollout.codex_turns.get(codex_turn_id) else {
                bail!("tool call start referenced unknown Codex turn {codex_turn_id}");
            };
            if turn.thread_id != thread_id {
                bail!(
                    "tool call start used thread {thread_id}, but Codex turn {codex_turn_id} \
                     belongs to {}",
                    turn.thread_id
                );
            }
        }
        Ok(())
    }

    fn ensure_unique_model_visible_tool_call(
        &self,
        model_visible_call_id: Option<&str>,
        tool_call_id: &str,
    ) -> Result<()> {
        let Some(model_visible_call_id) = model_visible_call_id else {
            return Ok(());
        };
        if let Some(existing) = self.single_tool_for_model_visible_call(model_visible_call_id)?
            && existing != tool_call_id
        {
            bail!("duplicate tool call for model-visible call id {model_visible_call_id}");
        }
        Ok(())
    }

    fn single_tool_for_model_visible_call(
        &self,
        model_visible_call_id: &str,
    ) -> Result<Option<ToolCallId>> {
        let mut matching = self
            .rollout
            .tool_calls
            .values()
            .filter(|tool| tool.model_visible_call_id.as_deref() == Some(model_visible_call_id))
            .map(|tool| tool.tool_call_id.clone());
        let first = matching.next();
        if matching.next().is_some() {
            bail!("multiple tool calls matched model-visible call id {model_visible_call_id}");
        }
        Ok(first)
    }

    fn model_visible_tool_item_ids(
        &self,
        thread_id: &str,
        call_id: &str,
        kinds: &[ConversationItemKind],
    ) -> Vec<String> {
        self.rollout
            .conversation_items
            .values()
            .filter(|item| {
                item.thread_id == thread_id
                    && item.call_id.as_deref() == Some(call_id)
                    && kinds.contains(&item.kind)
            })
            .map(|item| item.item_id.clone())
            .collect::<Vec<_>>()
    }

    fn add_tool_call_item(&mut self, tool_call_id: &str, item_id: &str) -> Result<()> {
        let Some(tool_call) = self.rollout.tool_calls.get_mut(tool_call_id) else {
            bail!("tool call {tool_call_id} disappeared during conversation linking");
        };
        push_unique(&mut tool_call.model_visible_call_item_ids, item_id);
        Ok(())
    }

    fn add_tool_output_item(&mut self, tool_call_id: &str, item_id: &str) -> Result<()> {
        let Some(tool_call) = self.rollout.tool_calls.get_mut(tool_call_id) else {
            bail!("tool call {tool_call_id} disappeared during output linking");
        };
        push_unique(&mut tool_call.model_visible_output_item_ids, item_id);

        let Some(item) = self.rollout.conversation_items.get_mut(item_id) else {
            bail!("conversation item {item_id} disappeared during output linking");
        };
        let producer = ProducerRef::Tool {
            tool_call_id: tool_call_id.to_string(),
        };
        if !item.produced_by.contains(&producer) {
            item.produced_by.push(producer);
        }
        Ok(())
    }

    fn link_tool_to_inference_response(&mut self, tool_call_id: &str) {
        let Some(tool_call) = self.rollout.tool_calls.get(tool_call_id) else {
            return;
        };
        let call_item_ids = tool_call.model_visible_call_item_ids.clone();
        if call_item_ids.is_empty() {
            return;
        }
        for inference in self.rollout.inference_calls.values_mut() {
            if inference
                .response_item_ids
                .iter()
                .any(|item_id| call_item_ids.contains(item_id))
                && !inference
                    .tool_call_ids_started_by_response
                    .contains(&tool_call_id.to_string())
            {
                inference
                    .tool_call_ids_started_by_response
                    .push(tool_call_id.to_string());
            }
        }
    }
}

fn push_unique(items: &mut Vec<String>, item_id: &str) {
    if !items.iter().any(|existing| existing == item_id) {
        items.push(item_id.to_string());
    }
}
