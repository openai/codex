//! Captures original conversation, tool activity and host-verified evidence from history.
//! The prototype supports append-only root histories; it has no durable intake state.

use anyhow::ensure;
use codex_history::ResponseItemEnvelope;
use codex_history::RetainedContextEvent;
use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItemProvenance;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_thread_store::LoadThreadHistoryParams;
use codex_thread_store::ThreadStore;
use serde::Serialize;
use std::sync::Arc;

/// Position in the containing snapshot's root history, independent of channel routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct EvidenceId {
    pub ordinal: usize,
}

/// The original rollout record retains roles, call IDs and host provenance unchanged.
/// Its contents are deliberately excluded from Debug output.
#[derive(Serialize)]
pub struct OriginalEvidence {
    pub id: EvidenceId,
    pub original: RolloutItem,
}

/// An immutable capture. Later history appends cannot enter its reads or routing.
/// Clones share the captured records rather than copying message payloads.
#[derive(Clone)]
pub struct EvidenceSnapshot {
    thread_id: ThreadId,
    records: Arc<[OriginalEvidence]>,
}

impl EvidenceSnapshot {
    /// Load through the existing history API after the host flushes pending originals.
    /// This deliberately reloads history; incremental IO and restart recovery are deferred.
    /// Forked, reverted and ambiguously attributed user histories are unsupported.
    pub async fn capture(store: &dyn ThreadStore, thread_id: ThreadId) -> anyhow::Result<Self> {
        let history = store
            .load_history(LoadThreadHistoryParams {
                thread_id,
                include_archived: false,
            })
            .await?;
        ensure!(
            history.thread_id == thread_id,
            "evidence history owner mismatch"
        );
        let mut records = Vec::new();
        for (ordinal, item) in history.items.into_iter().enumerate() {
            let include = match &item {
                RolloutItem::ResponseItem(envelope) => selected(envelope)?,
                RolloutItem::RetainedContext(RetainedContextEvent::VerifiedAnswer {
                    answer,
                    ..
                }) => {
                    ensure!(!answer.questions.is_empty(), "incomplete verified answer");
                    true
                }
                RolloutItem::RetainedContext(RetainedContextEvent::DeliveredAssistantMessage {
                    message,
                    ..
                }) => {
                    ensure!(message.complete, "incomplete delivered assistant message");
                    true
                }
                RolloutItem::SessionMeta(meta) => {
                    ensure!(
                        meta.meta.forked_from_id.is_none(),
                        "forked evidence is unsupported"
                    );
                    continue;
                }
                RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_)) => {
                    anyhow::bail!("reverted evidence is unsupported");
                }
                RolloutItem::RetainedContext(RetainedContextEvent::AcceptedFileAudio {
                    ..
                })
                | RolloutItem::DynamicToolsUpdated { .. }
                | RolloutItem::InterAgentCommunication(_)
                | RolloutItem::InterAgentCommunicationMetadata { .. }
                | RolloutItem::Compacted(_)
                | RolloutItem::TurnContext(_)
                | RolloutItem::TokenUsageRecord(_)
                | RolloutItem::WorldState(_)
                | RolloutItem::SecurityRiskScore(_)
                | RolloutItem::EventMsg(_)
                | RolloutItem::RealtimeItem(_) => continue,
            };
            if include {
                records.push(OriginalEvidence {
                    id: EvidenceId { ordinal },
                    original: item,
                });
            }
        }
        Ok(Self {
            thread_id,
            records: records.into(),
        })
    }

    pub fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    pub fn records(&self) -> &[OriginalEvidence] {
        &self.records
    }
}

fn selected(envelope: &ResponseItemEnvelope) -> anyhow::Result<bool> {
    if let Some(metadata) = &envelope.metadata {
        ensure!(
            !metadata.inherited_user_message,
            "inherited evidence is unsupported"
        );
        if metadata.compaction_output {
            return Ok(false);
        }
    }
    Ok(match &envelope.item {
        ResponseItem::Message {
            role,
            content,
            internal_chat_message_metadata_passthrough,
            ..
        } if role == "user" => {
            if envelope
                .metadata
                .as_ref()
                .is_some_and(|metadata| metadata.user_input_order.is_some())
            {
                true
            } else {
                // Generated context can use the user wire role. Only explicit producer
                // attribution permits skipping it; unknown user evidence stops capture.
                let metadata = internal_chat_message_metadata_passthrough.as_ref();
                let kinds = metadata.and_then(|metadata| metadata.content_item_kinds.as_ref());
                let sources = metadata.and_then(|metadata| metadata.content_item_metadata.as_ref());
                ensure!(
                    kinds.zip(sources).is_some_and(|(kinds, sources)| {
                        !content.is_empty()
                            && kinds.len() == content.len()
                            && sources.len() == content.len()
                            && kinds.iter().zip(sources).all(|(kind, source)| {
                                !kind.as_str().is_empty()
                                    && kind.as_str() != "unknown"
                                    && !kind.as_str().starts_with("user.")
                                    && matches!(
                                        &source.provenance,
                                        ContentItemProvenance::Harness { .. }
                                            | ContentItemProvenance::AgentsMd
                                            | ContentItemProvenance::Skills
                                            | ContentItemProvenance::DeveloperInstructions { .. }
                                            | ContentItemProvenance::Tool { .. }
                                    )
                            })
                    }),
                    "user evidence has no host acceptance or context provenance"
                );
                false
            }
        }
        ResponseItem::Message { role, .. } if role == "assistant" => true,
        ResponseItem::Message {
            role,
            content,
            internal_chat_message_metadata_passthrough: Some(metadata),
            ..
        } if role == "developer" => {
            // Only the host's approval fragment is evidence, not arbitrary developer text.
            metadata
                .content_item_kinds
                .as_ref()
                .zip(metadata.content_item_metadata.as_ref())
                .is_some_and(|(kinds, sources)| {
                    !content.is_empty()
                        && kinds.len() == content.len()
                        && sources.len() == content.len()
                        && kinds.iter().zip(sources).all(|(kind, source)| {
                            kind.as_str() == "guardian.approved_action"
                                && matches!(
                                    &source.provenance,
                                    ContentItemProvenance::Harness { .. }
                                )
                        })
                })
        }
        ResponseItem::FunctionCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. } => true,
        ResponseItem::Message { .. }
        | ResponseItem::AgentMessage { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::AdditionalTools { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::ConfigurationUpdate { .. }
        | ResponseItem::CompactionTrigger { .. }
        | ResponseItem::Other => false,
    })
}
