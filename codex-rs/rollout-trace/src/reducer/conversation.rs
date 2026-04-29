//! Conversation reduction from model-facing payload snapshots.
//!
//! Inference request inputs and response outputs are both part of the logical
//! conversation because they are the payloads exchanged with the model. Runtime
//! observations, such as local tool output, stay outside the transcript until a
//! later model-facing payload carries their content.

use anyhow::Context;
use anyhow::Ok;
use anyhow::Result;
use anyhow::bail;
use serde_json::Value;

use self::normalize::NormalizedConversationItem;
use super::TraceReducer;
use crate::model::CompactionId;
use crate::model::ConversationBody;
use crate::model::ConversationItem;
use crate::model::ConversationItemKind;
use crate::model::ConversationPart;
use crate::model::ConversationRole;
use crate::model::InferenceCallId;
use crate::model::ProducerRef;
use crate::payload::RawPayloadRef;

mod normalize;

impl TraceReducer {
    /// Reduces an inference request input snapshot into model-visible conversation items.
    ///
    /// Request snapshots are reconciled by position against the previous model-visible
    /// snapshot for the thread so repeated history reuses ids while newly inserted
    /// items remain distinct.
    pub(super) fn reduce_inference_request(
        &mut self,
        wall_time_unix_ms: i64,
        inference_call_id: &InferenceCallId,
        thread_id: &str,
        codex_turn_id: &str,
        request_payload: &RawPayloadRef,
    ) -> Result<Vec<String>> {
        let payload = self.read_payload_json(request_payload)?;
        let Some(input) = payload.get("input") else {
            bail!(
                "inference request payload {} did not contain input",
                request_payload.raw_payload_id
            );
        };
        let Some(request_items) = input.as_array() else {
            bail!(
                "inference request payload {} had non-array input",
                request_payload.raw_payload_id
            );
        };

        let items = normalize::normalize_model_items(request_items, request_payload)?;

        let previous_response_id = payload.get("previous_response_id").and_then(Value::as_str);
        // After compaction, the next full request is compared against the installed replacement
        // history, not the pre-compaction prompt. Any repeated developer/context prefix that Codex
        // reinjects must therefore become a fresh post-compaction conversation item.
        let post_compaction_snapshot = if previous_response_id.is_none() {
            self.pending_compaction_replacement_item_ids
                .get(thread_id)
                .cloned()
        } else {
            None
        };
        let request_item_ids = if let Some(previous_response_id) = previous_response_id {
            // Streaming follow-up requests can send only the new input plus a
            // `previous_response_id`. The trace model still exposes the full
            // model-visible input, so rebuild the omitted prefix from the
            // previous request and response before reducing this delta.
            let previous_items = self
                .rollout
                .inference_calls
                .values()
                .find(|inference| {
                    inference.thread_id == thread_id
                        && inference.response_id.as_deref() == Some(previous_response_id)
                })
                .map(|inference| {
                    let mut ids = inference.request_item_ids.clone();
                    ids.extend(inference.response_item_ids.clone());
                    ids
                });
            let Some(mut item_ids) = previous_items else {
                bail!(
                    "incremental inference request {inference_call_id} referenced unknown previous_response_id {previous_response_id}"
                );
            };
            let delta_item_ids = self.reconcile_conversation_items(
                items,
                ReconcileItems {
                    thread_id,
                    codex_turn_id,
                    wall_time_unix_ms,
                    produced_by: Vec::new(),
                    start_index: item_ids.len(),
                    mode: ReconcileMode::AppendOnly,
                    snapshot_override: None,
                },
            )?;
            item_ids.extend(delta_item_ids);
            item_ids
        } else {
            self.reconcile_conversation_items(
                items,
                ReconcileItems {
                    thread_id,
                    codex_turn_id,
                    wall_time_unix_ms,
                    produced_by: Vec::new(),
                    start_index: 0,
                    mode: ReconcileMode::FullSnapshot,
                    snapshot_override: post_compaction_snapshot.as_deref(),
                },
            )?
        };

        self.append_thread_conversation_items(thread_id, &request_item_ids)?;
        if post_compaction_snapshot.is_some() {
            self.pending_compaction_replacement_item_ids
                .remove(thread_id);
        }
        self.thread_conversation_snapshots
            .insert(thread_id.to_string(), request_item_ids.clone());
        Ok(request_item_ids)
    }

    /// Reduces an inference response payload into conversation items produced by the call.
    pub(super) fn reduce_inference_response(
        &mut self,
        wall_time_unix_ms: i64,
        inference_call_id: &InferenceCallId,
        response_payload: &RawPayloadRef,
    ) -> Result<Vec<String>> {
        let payload = self.read_payload_json(response_payload)?;
        let Some(output_items) = payload.get("output_items").and_then(Value::as_array) else {
            bail!(
                "inference response payload {} did not contain output_items",
                response_payload.raw_payload_id
            );
        };

        let Some((thread_id, codex_turn_id)) = self
            .rollout
            .inference_calls
            .get(inference_call_id)
            .map(|inference| (inference.thread_id.clone(), inference.codex_turn_id.clone()))
        else {
            bail!("inference response referenced unknown call {inference_call_id}");
        };

        let items = normalize::normalize_model_items(output_items, response_payload)?;
        // Response output is appended immediately: it was produced by the model,
        // so it is conversation even before a later request carries it forward.
        let append_at = self
            .thread_conversation_snapshots
            .get(&thread_id)
            .map_or(0, Vec::len);
        let response_item_ids = self.reconcile_conversation_items(
            items,
            ReconcileItems {
                thread_id: &thread_id,
                codex_turn_id: &codex_turn_id,
                wall_time_unix_ms,
                produced_by: vec![ProducerRef::Inference {
                    inference_call_id: inference_call_id.clone(),
                }],
                start_index: append_at,
                mode: ReconcileMode::AppendOnly,
                snapshot_override: None,
            },
        )?;
        self.append_thread_conversation_items(&thread_id, &response_item_ids)?;
        self.thread_conversation_snapshots
            .entry(thread_id)
            .or_default()
            .extend(response_item_ids.clone());

        if let Some(usage) = payload
            .get("token_usage")
            .and_then(normalize::token_usage_from_value)
            && let Some(inference) = self.rollout.inference_calls.get_mut(inference_call_id)
        {
            inference.usage = Some(usage);
        }

        Ok(response_item_ids)
    }

    fn reconcile_conversation_items(
        &mut self,
        items: Vec<NormalizedConversationItem>,
        context: ReconcileItems<'_>,
    ) -> Result<Vec<String>> {
        let previous_snapshot = context.snapshot_override.map_or_else(
            || {
                self.thread_conversation_snapshots
                    .get(context.thread_id)
                    .cloned()
                    .unwrap_or_default()
            },
            <[_]>::to_vec,
        );
        let mut item_ids = Vec::with_capacity(items.len());

        for (offset, item) in items.into_iter().enumerate() {
            let index = context.start_index + offset;
            let tool_link_item = item.clone();
            self.ensure_call_id_consistency(context.thread_id, &item)?;
            let item_id = if let Some(previous_item_id) = previous_snapshot.get(index) {
                if self.item_matches(previous_item_id, &item) {
                    previous_item_id.clone()
                } else if matches!(context.mode, ReconcileMode::FullSnapshot) {
                    self.find_matching_snapshot_item(&previous_snapshot, &item_ids, &item)
                        .unwrap_or_else(|| {
                            self.create_conversation_item(
                                context.thread_id,
                                Some(context.codex_turn_id.to_string()),
                                context.wall_time_unix_ms,
                                item,
                                context.produced_by.clone(),
                            )
                        })
                } else {
                    let codex_turn_id = context.codex_turn_id;
                    let thread_id = context.thread_id;
                    bail!(
                        "model conversation mismatch while reducing turn {codex_turn_id} for \
                         thread {thread_id} at item index {index}: existing item \
                         {previous_item_id} does not match the current model payload item"
                    );
                }
            } else if matches!(context.mode, ReconcileMode::FullSnapshot) {
                self.find_matching_snapshot_item(&previous_snapshot, &item_ids, &item)
                    .unwrap_or_else(|| {
                        self.create_conversation_item(
                            context.thread_id,
                            Some(context.codex_turn_id.to_string()),
                            context.wall_time_unix_ms,
                            item,
                            context.produced_by.clone(),
                        )
                    })
            } else {
                self.create_conversation_item(
                    context.thread_id,
                    Some(context.codex_turn_id.to_string()),
                    context.wall_time_unix_ms,
                    item,
                    context.produced_by.clone(),
                )
            };
            self.update_conversation_item_from_sighting(
                &item_id,
                &tool_link_item,
                &context.produced_by,
            )?;
            self.attach_model_visible_tool_item(
                &item_id,
                tool_link_item.call_id.as_deref(),
                &tool_link_item.kind,
            )?;
            self.attach_model_visible_code_cell_item(
                &item_id,
                tool_link_item.call_id.as_deref(),
                &tool_link_item.kind,
            )?;
            self.resolve_pending_agent_edges_for_item(&item_id)?;
            item_ids.push(item_id);
        }

        self.flush_pending_code_cell_starts()?;
        Ok(item_ids)
    }

    /// Reduces a compaction checkpoint payload into installed replacement history.
    ///
    /// The returned ids let the compaction reducer record both the boundary marker
    /// and the snapshot that future full requests should reconcile against.
    pub(super) fn reduce_compaction_checkpoint(
        &mut self,
        wall_time_unix_ms: i64,
        thread_id: &str,
        codex_turn_id: &str,
        compaction_id: &CompactionId,
        checkpoint_payload: &RawPayloadRef,
    ) -> Result<ReducedCompactionCheckpoint> {
        let payload = self.read_payload_json(checkpoint_payload)?;
        let input_history = required_array(&payload, "input_history", checkpoint_payload)?;
        let replacement_history =
            required_array(&payload, "replacement_history", checkpoint_payload)?;

        let input_items = normalize::normalize_model_items(input_history, checkpoint_payload)?;
        let replacement_items =
            normalize::normalize_model_items(replacement_history, checkpoint_payload)?;
        let input_candidates = self
            .thread_conversation_snapshots
            .get(thread_id)
            .cloned()
            .unwrap_or_default();
        let input_item_ids = self.reconcile_detached_conversation_items(
            input_items,
            DetachedReconcileItems {
                thread_id,
                codex_turn_id,
                wall_time_unix_ms,
                produced_by: Vec::new(),
                candidates: input_candidates,
            },
        )?;
        // A compaction checkpoint has two transcript effects. First, record the structural
        // boundary where old live history ended. Then append the replacement items, including
        // the provider-visible summary item if the compact endpoint returned one.
        let marker_item_id = self.create_conversation_item(
            thread_id,
            Some(codex_turn_id.to_string()),
            wall_time_unix_ms,
            NormalizedConversationItem {
                role: ConversationRole::Assistant,
                channel: None,
                kind: ConversationItemKind::CompactionMarker,
                // The summary is a separate model/provider-visible item. Keep the marker body
                // empty so transcript renderers cannot mistake the boundary for prompt content.
                body: ConversationBody { parts: Vec::new() },
                call_id: None,
            },
            vec![ProducerRef::Compaction {
                compaction_id: compaction_id.clone(),
            }],
        );
        let replacement_item_ids = self.reconcile_detached_conversation_items(
            replacement_items,
            DetachedReconcileItems {
                thread_id,
                codex_turn_id,
                wall_time_unix_ms,
                produced_by: vec![ProducerRef::Compaction {
                    compaction_id: compaction_id.clone(),
                }],
                // Replacement history is a rewrite boundary. Even if the compact endpoint emits
                // text that matches old history, the installed item is a new post-compaction
                // conversation item and should not reuse a pre-compaction ID.
                candidates: Vec::new(),
            },
        )?;
        self.append_thread_conversation_items(thread_id, &input_item_ids)?;
        self.append_thread_conversation_items(thread_id, std::slice::from_ref(&marker_item_id))?;
        self.append_thread_conversation_items(thread_id, &replacement_item_ids)?;
        Ok(ReducedCompactionCheckpoint {
            input_item_ids,
            marker_item_id,
            replacement_item_ids,
        })
    }

    fn reconcile_detached_conversation_items(
        &mut self,
        items: Vec<NormalizedConversationItem>,
        context: DetachedReconcileItems<'_>,
    ) -> Result<Vec<String>> {
        let mut item_ids = Vec::with_capacity(items.len());

        for item in items {
            let tool_link_item = item.clone();
            self.ensure_call_id_consistency(context.thread_id, &item)?;
            let item_id = self
                .find_matching_snapshot_item(&context.candidates, &item_ids, &item)
                .unwrap_or_else(|| {
                    self.create_conversation_item(
                        context.thread_id,
                        Some(context.codex_turn_id.to_string()),
                        context.wall_time_unix_ms,
                        item,
                        context.produced_by.clone(),
                    )
                });
            self.update_conversation_item_from_sighting(
                &item_id,
                &tool_link_item,
                &context.produced_by,
            )?;
            self.attach_model_visible_tool_item(
                &item_id,
                tool_link_item.call_id.as_deref(),
                &tool_link_item.kind,
            )?;
            self.attach_model_visible_code_cell_item(
                &item_id,
                tool_link_item.call_id.as_deref(),
                &tool_link_item.kind,
            )?;
            self.resolve_pending_agent_edges_for_item(&item_id)?;
            item_ids.push(item_id);
        }

        self.flush_pending_code_cell_starts()?;
        Ok(item_ids)
    }

    fn create_conversation_item(
        &mut self,
        thread_id: &str,
        codex_turn_id: Option<String>,
        first_seen_at_unix_ms: i64,
        item: NormalizedConversationItem,
        produced_by: Vec<ProducerRef>,
    ) -> String {
        let item_id = self.next_conversation_item_id();
        self.rollout.conversation_items.insert(
            item_id.clone(),
            ConversationItem {
                item_id: item_id.clone(),
                thread_id: thread_id.to_string(),
                codex_turn_id,
                first_seen_at_unix_ms,
                role: item.role,
                channel: item.channel,
                kind: item.kind,
                body: item.body,
                call_id: item.call_id,
                produced_by,
            },
        );
        item_id
    }

    fn update_conversation_item_from_sighting(
        &mut self,
        item_id: &str,
        normalized: &NormalizedConversationItem,
        produced_by: &[ProducerRef],
    ) -> Result<()> {
        let Some(item) = self.rollout.conversation_items.get_mut(item_id) else {
            bail!("conversation item {item_id} was referenced before it was created");
        };

        if item.kind == ConversationItemKind::Reasoning {
            merge_reasoning_body(&mut item.body, &normalized.body)?;
        }
        for producer in produced_by {
            if !item.produced_by.contains(producer) {
                item.produced_by.push(producer.clone());
            }
        }
        Ok(())
    }

    fn append_thread_conversation_items(
        &mut self,
        thread_id: &str,
        item_ids: &[String],
    ) -> Result<()> {
        let thread = self.thread_mut(thread_id)?;
        for item_id in item_ids {
            if !thread.conversation_item_ids.contains(item_id) {
                thread.conversation_item_ids.push(item_id.clone());
            }
        }
        Ok(())
    }

    fn find_matching_snapshot_item(
        &self,
        previous_snapshot: &[String],
        used_item_ids: &[String],
        normalized: &NormalizedConversationItem,
    ) -> Option<String> {
        previous_snapshot
            .iter()
            .find(|item_id| {
                !used_item_ids.contains(item_id) && self.item_matches(item_id, normalized)
            })
            .cloned()
    }

    fn ensure_call_id_consistency(
        &self,
        thread_id: &str,
        normalized: &NormalizedConversationItem,
    ) -> Result<()> {
        let Some(call_id) = normalized.call_id.as_deref() else {
            return Ok(());
        };
        for item in self.rollout.conversation_items.values() {
            if item.thread_id == thread_id
                && item.call_id.as_deref() == Some(call_id)
                && item.kind == normalized.kind
                && !conversation_item_matches(item, normalized)
            {
                bail!("model-visible call id {call_id} was reused with different content");
            }
        }
        Ok(())
    }

    fn item_matches(&self, item_id: &str, normalized: &NormalizedConversationItem) -> bool {
        let Some(item) = self.rollout.conversation_items.get(item_id) else {
            return false;
        };
        conversation_item_matches(item, normalized)
    }

    fn next_conversation_item_id(&mut self) -> String {
        let ordinal = self.next_conversation_item_ordinal;
        self.next_conversation_item_ordinal += 1;
        format!("conversation_item:{ordinal}")
    }
}

#[derive(Clone, Copy)]
enum ReconcileMode {
    /// Full model requests are authoritative snapshots of the live context. The
    /// prompt builder can reorder already-observed items or replace history
    /// with synthetic summary messages, so item identity is "same content,
    /// reused at most once in this snapshot" rather than "same position only".
    FullSnapshot,
    /// Incremental request deltas and response outputs append to a known prefix.
    /// A mismatch at an occupied position means our reconstructed prefix is
    /// wrong and should fail replay.
    AppendOnly,
}

struct ReconcileItems<'a> {
    thread_id: &'a str,
    codex_turn_id: &'a str,
    wall_time_unix_ms: i64,
    produced_by: Vec<ProducerRef>,
    start_index: usize,
    mode: ReconcileMode,
    snapshot_override: Option<&'a [String]>,
}

struct DetachedReconcileItems<'a> {
    thread_id: &'a str,
    codex_turn_id: &'a str,
    wall_time_unix_ms: i64,
    produced_by: Vec<ProducerRef>,
    candidates: Vec<String>,
}

/// Conversation ids produced when a compaction checkpoint is installed.
///
/// The marker item records the boundary, while replacement items are the live
/// history that subsequent full requests should treat as their baseline.
pub(super) struct ReducedCompactionCheckpoint {
    pub(super) input_item_ids: Vec<String>,
    pub(super) marker_item_id: String,
    pub(super) replacement_item_ids: Vec<String>,
}

fn required_array<'a>(
    payload: &'a Value,
    key: &str,
    raw_payload: &RawPayloadRef,
) -> Result<&'a Vec<Value>> {
    payload.get(key).and_then(Value::as_array).with_context(|| {
        format!(
            "compaction checkpoint payload {} did not contain array {key}",
            raw_payload.raw_payload_id
        )
    })
}

fn conversation_item_matches(
    item: &ConversationItem,
    normalized: &NormalizedConversationItem,
) -> bool {
    let body_matches = if item.kind == ConversationItemKind::Reasoning
        && normalized.kind == ConversationItemKind::Reasoning
    {
        reasoning_body_matches(&item.body, &normalized.body)
    } else {
        conversation_body_matches(&item.body, &normalized.body)
    };

    item.role == normalized.role
        && item.channel == normalized.channel
        && item.kind == normalized.kind
        && body_matches
        && item.call_id == normalized.call_id
}

fn conversation_body_matches(left: &ConversationBody, right: &ConversationBody) -> bool {
    left.parts.len() == right.parts.len()
        && left
            .parts
            .iter()
            .zip(&right.parts)
            .all(|(left, right)| match (left, right) {
                (
                    ConversationPart::Json {
                        summary: left_summary,
                        raw_payload_id: _,
                    },
                    ConversationPart::Json {
                        summary: right_summary,
                        raw_payload_id: _,
                    },
                ) => left_summary == right_summary,
                _ => left == right,
            })
}

fn reasoning_body_matches(left: &ConversationBody, right: &ConversationBody) -> bool {
    if conversation_body_matches(left, right) {
        return true;
    }

    // The Responses API may return readable reasoning on completion, but later
    // request snapshots often replay only the encrypted blob. Treat the blob as
    // stable model-visible identity and merge readable text as best-effort
    // evidence, because request/response serialization can observe different
    // readable forms for the same encrypted reasoning item.
    let Some(left_encoded) = reasoning_encoded_part(left) else {
        return false;
    };
    let Some(right_encoded) = reasoning_encoded_part(right) else {
        return false;
    };

    left_encoded == right_encoded
}

fn merge_reasoning_body(
    existing: &mut ConversationBody,
    incoming: &ConversationBody,
) -> Result<()> {
    if conversation_body_matches(existing, incoming) {
        return Ok(());
    }
    if !reasoning_body_matches(existing, incoming) {
        bail!("reasoning item merge attempted with different encrypted_content identity");
    }

    let existing_text_parts = reasoning_text_parts(existing);
    let existing_summary_parts = reasoning_summary_parts(existing);
    if !existing_text_parts.is_empty() && !existing_summary_parts.is_empty() {
        return Ok(());
    }

    let incoming_text_parts = reasoning_text_parts(incoming);
    let incoming_summary_parts = reasoning_summary_parts(incoming);

    let text_parts = if !existing_text_parts.is_empty() {
        existing_text_parts
    } else {
        incoming_text_parts
    };

    let summary_parts = if !existing_summary_parts.is_empty() {
        existing_summary_parts
    } else {
        incoming_summary_parts
    };

    // We already know that the encoded part exist (and matches).
    let encoded_parts = reasoning_encoded_parts(existing);

    existing.parts = text_parts
        .into_iter()
        .cloned()
        .chain(summary_parts.into_iter().cloned())
        .chain(encoded_parts.into_iter().cloned())
        .collect();

    Ok(())
}

fn reasoning_text_parts(body: &ConversationBody) -> Vec<&ConversationPart> {
    body.parts
        .iter()
        .filter(|part| matches!(part, ConversationPart::Text { .. }))
        .collect()
}

fn reasoning_summary_parts(body: &ConversationBody) -> Vec<&ConversationPart> {
    body.parts
        .iter()
        .filter(|part| matches!(part, ConversationPart::Summary { .. }))
        .collect()
}

fn reasoning_encoded_parts(body: &ConversationBody) -> Vec<&ConversationPart> {
    body.parts
        .iter()
        .filter(|part| matches!(part, ConversationPart::Encoded { .. }))
        .collect()
}

fn reasoning_encoded_part(body: &ConversationBody) -> Option<(&str, &str)> {
    body.parts.iter().find_map(|part| {
        if let ConversationPart::Encoded { label, value } = part {
            Some((label.as_str(), value.as_str()))
        } else {
            None
        }
    })
}

#[cfg(test)]
#[path = "conversation_tests.rs"]
mod tests;
