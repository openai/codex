use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_protocol::protocol::CollabAgentInteractionBeginEvent;
use codex_protocol::protocol::CollabAgentInteractionEndEvent;
use codex_protocol::protocol::CollabAgentSpawnEndEvent;
use codex_protocol::protocol::CollabCloseBeginEvent;
use codex_protocol::protocol::CollabCloseEndEvent;
use codex_protocol::protocol::InterAgentCommunication;

use super::super::TraceReducer;
use crate::model::ConversationItem;
use crate::model::ConversationItemKind;
use crate::model::ConversationPart;
use crate::model::ConversationRole;
use crate::model::InteractionEdge;
use crate::model::InteractionEdgeKind;
use crate::model::ToolCallKind;
use crate::model::TraceAnchor;
use crate::payload::RawPayloadRef;

/// Agent delivery edge waiting for the recipient-side conversation item.
///
/// Multi-agent v2 records the sender tool before the target thread necessarily
/// includes the delivered mailbox message in a model-visible request. The edge
/// stays pending so it can target that exact conversation item when possible.
pub(in crate::reducer) struct PendingAgentInteractionEdge {
    pub(in crate::reducer) edge_id: String,
    pub(in crate::reducer) kind: InteractionEdgeKind,
    pub(in crate::reducer) source: TraceAnchor,
    pub(in crate::reducer) target_thread_id: String,
    pub(in crate::reducer) message_content: String,
    /// Spawn-only fallback for children that fail before their task message is model-visible.
    pub(in crate::reducer) unresolved_spawn_thread_id: Option<String>,
    pub(in crate::reducer) started_at_unix_ms: i64,
    pub(in crate::reducer) ended_at_unix_ms: Option<i64>,
    pub(in crate::reducer) carried_raw_payload_ids: Vec<String>,
}

/// Typed reducer input for a multi-agent v2 child completion notification.
///
/// Child results are observed outside the normal tool lifecycle, but they still
/// carry a parent-thread notification. This wrapper keeps the dispatcher from
/// passing a positional bundle of thread and turn ids.
pub(in crate::reducer) struct ObservedAgentResultEdge {
    pub(in crate::reducer) wall_time_unix_ms: i64,
    pub(in crate::reducer) edge_id: String,
    pub(in crate::reducer) child_thread_id: String,
    pub(in crate::reducer) child_codex_turn_id: String,
    pub(in crate::reducer) parent_thread_id: String,
    pub(in crate::reducer) message: String,
    pub(in crate::reducer) carried_payload: Option<RawPayloadRef>,
}

/// Builds the stable edge id for the spawn relationship between two threads.
pub(in crate::reducer) fn spawn_edge_id(parent_thread_id: &str, child_thread_id: &str) -> String {
    format!("edge:spawn:{parent_thread_id}:{child_thread_id}")
}

impl TraceReducer {
    /// Starts a multi-agent edge from a runtime begin payload, when the tool kind supports one.
    pub(super) fn start_agent_interaction_from_runtime(
        &mut self,
        tool_call_id: &str,
        runtime_payload: &RawPayloadRef,
    ) -> Result<()> {
        let kind = self
            .rollout
            .tool_calls
            .get(tool_call_id)
            .with_context(|| format!("agent edge referenced unknown tool call {tool_call_id}"))?
            .kind
            .clone();
        match kind {
            ToolCallKind::AssignAgentTask => {
                let payload: CollabAgentInteractionBeginEvent =
                    serde_json::from_value(self.read_payload_json(runtime_payload)?)?;
                self.queue_message_agent_interaction(
                    tool_call_id,
                    InteractionEdgeKind::AssignAgentTask,
                    payload.receiver_thread_id.to_string(),
                    payload.prompt,
                    /*ended_at_unix_ms*/ None,
                )
            }
            ToolCallKind::SendMessage => {
                let payload: CollabAgentInteractionBeginEvent =
                    serde_json::from_value(self.read_payload_json(runtime_payload)?)?;
                self.queue_message_agent_interaction(
                    tool_call_id,
                    InteractionEdgeKind::SendMessage,
                    payload.receiver_thread_id.to_string(),
                    payload.prompt,
                    /*ended_at_unix_ms*/ None,
                )
            }
            ToolCallKind::CloseAgent => {
                let payload: CollabCloseBeginEvent =
                    serde_json::from_value(self.read_payload_json(runtime_payload)?)?;
                self.upsert_close_agent_interaction(
                    tool_call_id,
                    payload.receiver_thread_id.to_string(),
                    /*ended_at_unix_ms*/ None,
                )
            }
            ToolCallKind::ExecCommand
            | ToolCallKind::WriteStdin
            | ToolCallKind::ApplyPatch
            | ToolCallKind::Mcp { .. }
            | ToolCallKind::Web
            | ToolCallKind::ImageGeneration
            | ToolCallKind::SpawnAgent
            | ToolCallKind::WaitAgent
            | ToolCallKind::Other { .. } => Ok(()),
        }
    }

    /// Ends or enriches a multi-agent edge from a runtime end payload.
    pub(super) fn end_agent_interaction_from_runtime(
        &mut self,
        wall_time_unix_ms: i64,
        tool_call_id: &str,
        runtime_payload: &RawPayloadRef,
    ) -> Result<()> {
        let kind = self.rollout.tool_calls[tool_call_id].kind.clone();
        match kind {
            ToolCallKind::SpawnAgent => {
                let payload: CollabAgentSpawnEndEvent =
                    serde_json::from_value(self.read_payload_json(runtime_payload)?)?;
                self.end_spawn_agent_interaction(wall_time_unix_ms, tool_call_id, &payload)
            }
            ToolCallKind::AssignAgentTask => {
                let payload: CollabAgentInteractionEndEvent =
                    serde_json::from_value(self.read_payload_json(runtime_payload)?)?;
                self.end_message_agent_interaction(
                    wall_time_unix_ms,
                    tool_call_id,
                    InteractionEdgeKind::AssignAgentTask,
                    &payload,
                )
            }
            ToolCallKind::SendMessage => {
                let payload: CollabAgentInteractionEndEvent =
                    serde_json::from_value(self.read_payload_json(runtime_payload)?)?;
                self.end_message_agent_interaction(
                    wall_time_unix_ms,
                    tool_call_id,
                    InteractionEdgeKind::SendMessage,
                    &payload,
                )
            }
            ToolCallKind::CloseAgent => {
                let payload: CollabCloseEndEvent =
                    serde_json::from_value(self.read_payload_json(runtime_payload)?)?;
                self.upsert_close_agent_interaction(
                    tool_call_id,
                    payload.receiver_thread_id.to_string(),
                    Some(wall_time_unix_ms),
                )
            }
            ToolCallKind::ExecCommand
            | ToolCallKind::WriteStdin
            | ToolCallKind::ApplyPatch
            | ToolCallKind::Mcp { .. }
            | ToolCallKind::Web
            | ToolCallKind::ImageGeneration
            | ToolCallKind::WaitAgent
            | ToolCallKind::Other { .. } => Ok(()),
        }
    }

    /// Adds the canonical tool result payload to an already reduced multi-agent edge.
    pub(super) fn attach_agent_interaction_tool_result(
        &mut self,
        tool_call_id: &str,
        result_payload: Option<&RawPayloadRef>,
    ) -> Result<()> {
        let Some(result_payload) = result_payload else {
            return Ok(());
        };
        if let Some(edge) = self
            .rollout
            .interaction_edges
            .values_mut()
            .find(|edge| tool_call_source_matches(&edge.source, tool_call_id))
        {
            push_unique(
                &mut edge.carried_raw_payload_ids,
                &result_payload.raw_payload_id,
            );
            return Ok(());
        }

        // Agent delivery edges intentionally wait for the recipient-side
        // conversation item. Tool end can arrive before that item is
        // reduced, so preserve the response payload on the pending edge rather
        // than dropping evidence until the delivery materializes.
        if let Some(pending) = self
            .pending_agent_interaction_edges
            .iter_mut()
            .find(|pending| tool_call_source_matches(&pending.source, tool_call_id))
        {
            push_unique(
                &mut pending.carried_raw_payload_ids,
                &result_payload.raw_payload_id,
            );
        }
        Ok(())
    }

    fn end_spawn_agent_interaction(
        &mut self,
        wall_time_unix_ms: i64,
        tool_call_id: &str,
        payload: &CollabAgentSpawnEndEvent,
    ) -> Result<()> {
        let Some(child_thread_id) = payload.new_thread_id else {
            return Ok(());
        };
        let tool_call = &self.rollout.tool_calls[tool_call_id];
        let child_thread_id = child_thread_id.to_string();
        let edge_id = spawn_edge_id(&payload.sender_thread_id.to_string(), &child_thread_id);

        self.queue_or_resolve_agent_interaction_edge(PendingAgentInteractionEdge {
            edge_id,
            kind: InteractionEdgeKind::SpawnAgent,
            source: TraceAnchor::ToolCall {
                tool_call_id: tool_call_id.to_string(),
            },
            target_thread_id: child_thread_id.clone(),
            message_content: payload.prompt.clone(),
            unresolved_spawn_thread_id: Some(child_thread_id),
            started_at_unix_ms: tool_call.execution.started_at_unix_ms,
            ended_at_unix_ms: Some(wall_time_unix_ms),
            carried_raw_payload_ids: self.agent_tool_payload_ids(tool_call_id)?,
        })
    }

    fn end_message_agent_interaction(
        &mut self,
        wall_time_unix_ms: i64,
        tool_call_id: &str,
        edge_kind: InteractionEdgeKind,
        payload: &CollabAgentInteractionEndEvent,
    ) -> Result<()> {
        self.queue_message_agent_interaction(
            tool_call_id,
            edge_kind,
            payload.receiver_thread_id.to_string(),
            payload.prompt.clone(),
            Some(wall_time_unix_ms),
        )
    }

    fn queue_message_agent_interaction(
        &mut self,
        tool_call_id: &str,
        kind: InteractionEdgeKind,
        target_thread_id: String,
        message_content: String,
        ended_at_unix_ms: Option<i64>,
    ) -> Result<()> {
        let tool_call = &self.rollout.tool_calls[tool_call_id];
        self.queue_or_resolve_agent_interaction_edge(PendingAgentInteractionEdge {
            edge_id: tool_edge_id(tool_call_id),
            kind,
            source: TraceAnchor::ToolCall {
                tool_call_id: tool_call_id.to_string(),
            },
            target_thread_id,
            message_content,
            unresolved_spawn_thread_id: None,
            started_at_unix_ms: tool_call.execution.started_at_unix_ms,
            ended_at_unix_ms,
            carried_raw_payload_ids: self.agent_tool_payload_ids(tool_call_id)?,
        })
    }

    fn agent_tool_payload_ids(&self, tool_call_id: &str) -> Result<Vec<String>> {
        let tool_call =
            self.rollout.tool_calls.get(tool_call_id).with_context(|| {
                format!("agent edge referenced unknown tool call {tool_call_id}")
            })?;
        let mut payload_ids = Vec::new();
        if let Some(payload_id) = &tool_call.raw_invocation_payload_id {
            push_unique(&mut payload_ids, payload_id);
        }
        for payload_id in &tool_call.raw_runtime_payload_ids {
            push_unique(&mut payload_ids, payload_id);
        }
        if let Some(payload_id) = &tool_call.raw_result_payload_id {
            push_unique(&mut payload_ids, payload_id);
        }
        Ok(payload_ids)
    }

    fn upsert_close_agent_interaction(
        &mut self,
        tool_call_id: &str,
        target_thread_id: String,
        ended_at_unix_ms: Option<i64>,
    ) -> Result<()> {
        if !self.rollout.threads.contains_key(&target_thread_id) {
            // A failed close can name a thread that never participated in this
            // trace. Keep that evidence on the ToolCall raw payloads rather
            // than creating an anchor to a non-existent reduced object.
            return Ok(());
        }
        let started_at_unix_ms = self
            .rollout
            .tool_calls
            .get(tool_call_id)
            .with_context(|| format!("close edge referenced unknown tool call {tool_call_id}"))?
            .execution
            .started_at_unix_ms;
        let carried_raw_payload_ids = self.agent_tool_payload_ids(tool_call_id)?;
        self.upsert_interaction_edge(InteractionEdge {
            edge_id: tool_edge_id(tool_call_id),
            kind: InteractionEdgeKind::CloseAgent,
            source: TraceAnchor::ToolCall {
                tool_call_id: tool_call_id.to_string(),
            },
            target: TraceAnchor::Thread {
                thread_id: target_thread_id,
            },
            started_at_unix_ms,
            ended_at_unix_ms,
            carried_item_ids: Vec::new(),
            carried_raw_payload_ids,
        })
    }

    /// Queues or resolves the edge from a child completion to its parent notification.
    pub(in crate::reducer) fn queue_agent_result_interaction_edge(
        &mut self,
        observed: ObservedAgentResultEdge,
    ) -> Result<()> {
        let source = if let Some(source_item_id) = self.latest_assistant_message_item_for_turn(
            &observed.child_thread_id,
            &observed.child_codex_turn_id,
        ) {
            TraceAnchor::ConversationItem {
                item_id: source_item_id,
            }
        } else {
            // Child completion is delivered from AgentStatus, not from transcript
            // content. Failed or cancelled children can therefore notify the parent
            // without producing a final assistant message. Anchor those edges to
            // the child thread so the trace keeps the valid delivery instead of
            // inventing a missing conversation item.
            TraceAnchor::Thread {
                thread_id: observed.child_thread_id,
            }
        };

        self.queue_or_resolve_agent_interaction_edge(PendingAgentInteractionEdge {
            edge_id: observed.edge_id,
            kind: InteractionEdgeKind::AgentResult,
            source,
            target_thread_id: observed.parent_thread_id,
            message_content: observed.message,
            unresolved_spawn_thread_id: None,
            started_at_unix_ms: observed.wall_time_unix_ms,
            ended_at_unix_ms: Some(observed.wall_time_unix_ms),
            carried_raw_payload_ids: observed
                .carried_payload
                .map(|payload| vec![payload.raw_payload_id])
                .unwrap_or_default(),
        })
    }

    /// Resolves pending agent edges whose target is the newly reduced conversation item.
    pub(in crate::reducer) fn resolve_pending_agent_edges_for_item(
        &mut self,
        item_id: &str,
    ) -> Result<()> {
        let Some((thread_id, message_content)) = self.inter_agent_message_item(item_id) else {
            return Ok(());
        };
        let Some(pending_index) = self
            .pending_agent_interaction_edges
            .iter()
            .position(|pending| {
                pending.target_thread_id == thread_id && pending.message_content == message_content
            })
        else {
            return Ok(());
        };
        let pending = self.pending_agent_interaction_edges.remove(pending_index);
        self.upsert_agent_interaction_edge_for_item(pending, item_id.to_string())
    }

    fn queue_or_resolve_agent_interaction_edge(
        &mut self,
        pending: PendingAgentInteractionEdge,
    ) -> Result<()> {
        if let Some(item_id) = self.find_unlinked_inter_agent_message_item(
            &pending.target_thread_id,
            &pending.message_content,
        ) {
            return self.upsert_agent_interaction_edge_for_item(pending, item_id);
        }

        if let Some(existing) = self
            .pending_agent_interaction_edges
            .iter_mut()
            .find(|existing| existing.edge_id == pending.edge_id)
        {
            if existing.kind != pending.kind
                || existing.source != pending.source
                || existing.target_thread_id != pending.target_thread_id
                || existing.message_content != pending.message_content
                || existing.unresolved_spawn_thread_id != pending.unresolved_spawn_thread_id
            {
                bail!(
                    "pending interaction edge {} was observed with conflicting delivery data",
                    pending.edge_id
                );
            }
            existing.started_at_unix_ms =
                existing.started_at_unix_ms.min(pending.started_at_unix_ms);
            existing.ended_at_unix_ms = match (existing.ended_at_unix_ms, pending.ended_at_unix_ms)
            {
                (Some(existing_ended), Some(pending_ended)) => {
                    Some(existing_ended.max(pending_ended))
                }
                (None, ended) | (ended, None) => ended,
            };
            extend_unique(
                &mut existing.carried_raw_payload_ids,
                pending.carried_raw_payload_ids,
            );
            return Ok(());
        }

        self.pending_agent_interaction_edges.push(pending);
        Ok(())
    }

    /// Materializes unresolved spawn edges that have a valid child-thread fallback target.
    pub(in crate::reducer) fn resolve_pending_spawn_edge_fallbacks(&mut self) -> Result<()> {
        let pending_edges = std::mem::take(&mut self.pending_agent_interaction_edges);
        for pending in pending_edges {
            let Some(child_thread_id) = pending.unresolved_spawn_thread_id else {
                continue;
            };
            if pending.kind != InteractionEdgeKind::SpawnAgent {
                bail!(
                    "non-spawn interaction edge {} carried a spawn fallback target",
                    pending.edge_id
                );
            }
            if !self.rollout.threads.contains_key(&child_thread_id) {
                continue;
            }

            // Spawn normally resolves to the child task message because that is
            // where the delegated work first becomes model-visible. A child can
            // fail before that transcript item exists, but the spawned thread is
            // still real and the spawning tool still created it. Preserve that
            // relationship with the thread fallback instead of dropping the edge.
            self.upsert_interaction_edge(InteractionEdge {
                edge_id: pending.edge_id,
                kind: pending.kind,
                source: pending.source,
                target: TraceAnchor::Thread {
                    thread_id: child_thread_id,
                },
                started_at_unix_ms: pending.started_at_unix_ms,
                ended_at_unix_ms: pending.ended_at_unix_ms,
                carried_item_ids: Vec::new(),
                carried_raw_payload_ids: pending.carried_raw_payload_ids,
            })?;
        }
        Ok(())
    }

    fn upsert_agent_interaction_edge_for_item(
        &mut self,
        pending: PendingAgentInteractionEdge,
        target_item_id: String,
    ) -> Result<()> {
        self.upsert_interaction_edge(InteractionEdge {
            edge_id: pending.edge_id,
            kind: pending.kind,
            source: pending.source,
            target: TraceAnchor::ConversationItem {
                item_id: target_item_id.clone(),
            },
            started_at_unix_ms: pending.started_at_unix_ms,
            ended_at_unix_ms: pending.ended_at_unix_ms,
            carried_item_ids: vec![target_item_id],
            carried_raw_payload_ids: pending.carried_raw_payload_ids,
        })
    }

    fn upsert_interaction_edge(&mut self, edge: InteractionEdge) -> Result<()> {
        if let Some(existing) = self.rollout.interaction_edges.get_mut(&edge.edge_id) {
            if existing.kind != edge.kind
                || existing.source != edge.source
                || existing.target != edge.target
            {
                bail!(
                    "interaction edge {} was observed with conflicting endpoints",
                    edge.edge_id
                );
            }
            existing.started_at_unix_ms = existing.started_at_unix_ms.min(edge.started_at_unix_ms);
            existing.ended_at_unix_ms = match (existing.ended_at_unix_ms, edge.ended_at_unix_ms) {
                (Some(existing_ended), Some(edge_ended)) => Some(existing_ended.max(edge_ended)),
                (None, ended) | (ended, None) => ended,
            };
            extend_unique(&mut existing.carried_item_ids, edge.carried_item_ids);
            extend_unique(
                &mut existing.carried_raw_payload_ids,
                edge.carried_raw_payload_ids,
            );
            return Ok(());
        }

        self.rollout
            .interaction_edges
            .insert(edge.edge_id.clone(), edge);
        Ok(())
    }

    fn find_unlinked_inter_agent_message_item(
        &self,
        thread_id: &str,
        message_content: &str,
    ) -> Option<String> {
        self.rollout
            .threads
            .get(thread_id)?
            .conversation_item_ids
            .iter()
            .find(|item_id| {
                !self.is_interaction_edge_target_item(item_id)
                    && self
                        .inter_agent_message_item(item_id)
                        .is_some_and(|(_, content)| content == message_content)
            })
            .cloned()
    }

    fn inter_agent_message_item(&self, item_id: &str) -> Option<(String, String)> {
        let item = self.rollout.conversation_items.get(item_id)?;
        let (recipient_agent_path, message_content) = inter_agent_message_fields(item)?;
        let thread = self.rollout.threads.get(&item.thread_id)?;
        if recipient_agent_path != thread.agent_path {
            return None;
        }
        Some((item.thread_id.clone(), message_content))
    }

    fn is_interaction_edge_target_item(&self, item_id: &str) -> bool {
        self.rollout
            .interaction_edges
            .values()
            .any(|edge| matches!(&edge.target, TraceAnchor::ConversationItem { item_id: target } if target == item_id))
    }

    fn latest_assistant_message_item_for_turn(
        &self,
        thread_id: &str,
        codex_turn_id: &str,
    ) -> Option<String> {
        self.rollout
            .conversation_items
            .values()
            .filter(|item| {
                item.thread_id == thread_id
                    && item.codex_turn_id.as_deref() == Some(codex_turn_id)
                    && item.role == ConversationRole::Assistant
                    && item.kind == ConversationItemKind::Message
            })
            .max_by_key(|item| item.first_seen_at_unix_ms)
            .map(|item| item.item_id.clone())
    }
}

fn extend_unique(items: &mut Vec<String>, new_items: Vec<String>) {
    for item in new_items {
        if !items.iter().any(|existing| existing == &item) {
            items.push(item);
        }
    }
}

fn tool_edge_id(tool_call_id: &str) -> String {
    format!("edge:tool:{tool_call_id}")
}

fn tool_call_source_matches(anchor: &TraceAnchor, tool_call_id: &str) -> bool {
    matches!(anchor, TraceAnchor::ToolCall { tool_call_id: source } if source == tool_call_id)
}

fn push_unique(items: &mut Vec<String>, item: &str) {
    if !items.iter().any(|existing| existing == item) {
        items.push(item.to_string());
    }
}

fn inter_agent_message_fields(item: &ConversationItem) -> Option<(String, String)> {
    // Multi-agent v2 injects mailbox deliveries as assistant messages whose
    // text is serialized `InterAgentCommunication`. Treat only that exact
    // transport shape as an edge target; ordinary assistant JSON must not be
    // mistaken for cross-thread delivery.
    if item.role != ConversationRole::Assistant || item.kind != ConversationItemKind::Message {
        return None;
    }
    let [ConversationPart::Text { text }] = item.body.parts.as_slice() else {
        return None;
    };
    let communication = serde_json::from_str::<InterAgentCommunication>(text).ok()?;
    Some((communication.recipient.to_string(), communication.content))
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
