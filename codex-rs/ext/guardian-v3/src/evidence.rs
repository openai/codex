//! Captures original evidence after an in-memory source cursor, without backfilling.
//! A capture has a fixed end; consumers acknowledge records only after their writes succeed.

use anyhow::ensure;
use codex_history::ResponseItemEnvelope;
use codex_history::RetainedContextEvent;
use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItemProvenance;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HistoryPosition;
use codex_thread_store::OriginalHistoryRequest;
use codex_thread_store::ThreadStore;
use serde::Serialize;
use std::sync::Arc;

/// Position in the containing snapshot's root history, independent of channel routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct EvidenceId {
    pub ordinal: u64,
}

/// The original rollout record retains roles, call IDs and host provenance unchanged.
/// Its contents are deliberately excluded from Debug output.
#[derive(Serialize)]
pub struct OriginalEvidence {
    pub id: EvidenceId,
    pub original: RolloutItem,
    #[serde(skip)]
    end: HistoryPosition,
}

/// An immutable capture. Later history appends cannot enter its reads or routing.
/// Clones share the captured records rather than copying message payloads.
#[derive(Clone)]
pub struct EvidenceSnapshot {
    thread_id: ThreadId,
    start: HistoryPosition,
    end: HistoryPosition,
    records: Arc<[OriginalEvidence]>,
}

impl EvidenceSnapshot {
    pub fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    pub fn records(&self) -> &[OriginalEvidence] {
        &self.records
    }
}

/// Live progress for one board. Never serialized or reconstructed after restart.
/// Create it at board activation, before accepting new input. Hosts flush pending
/// history before activation/capture and coordinate lifetime with thread deletion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceCursor {
    thread_id: ThreadId,
    position: HistoryPosition,
}

impl EvidenceCursor {
    /// Start at the current complete tail. Existing records are not backfilled.
    pub async fn from_now(store: &dyn ThreadStore, thread_id: ThreadId) -> anyhow::Result<Self> {
        let tail = store
            .read_original_history(thread_id, OriginalHistoryRequest::Tail)
            .await?;
        Ok(Self {
            thread_id,
            position: tail.end,
        })
    }

    pub fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    /// Read the unread suffix through a fixed cutoff without advancing progress.
    pub async fn capture(&self, store: &dyn ThreadStore) -> anyhow::Result<EvidenceSnapshot> {
        let history = store
            .read_original_history(self.thread_id, OriginalHistoryRequest::After(self.position))
            .await?;
        let end = history.end;
        let mut records = Vec::new();
        for record in history.items {
            let include = match &record.item {
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
                RolloutItem::SessionMeta(_)
                | RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_)) => {
                    anyhow::bail!("evidence history was replaced");
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
                | RolloutItem::RealtimeItem(_) => false,
            };
            if include {
                records.push(OriginalEvidence {
                    id: EvidenceId {
                        ordinal: record.end.end_ordinal_exclusive.checked_sub(1).ok_or_else(
                            || anyhow::anyhow!("evidence record has no source ordinal"),
                        )?,
                    },
                    original: record.item,
                    end: record.end,
                });
            }
        }
        Ok(EvidenceSnapshot {
            thread_id: self.thread_id,
            start: self.position,
            end,
            records: records.into(),
        })
    }

    /// Records from this capture that have not yet been acknowledged.
    pub fn pending<'a>(
        &self,
        snapshot: &'a EvidenceSnapshot,
    ) -> anyhow::Result<&'a [OriginalEvidence]> {
        ensure!(
            snapshot.thread_id == self.thread_id
                && snapshot.start.thread_id == self.position.thread_id
                && snapshot.end.thread_id == self.position.thread_id
                && snapshot.start.end_byte_offset <= self.position.end_byte_offset
                && self.position.end_byte_offset <= snapshot.end.end_byte_offset
                && snapshot.start.end_ordinal_exclusive <= self.position.end_ordinal_exclusive
                && self.position.end_ordinal_exclusive <= snapshot.end.end_ordinal_exclusive,
            "evidence capture does not contain the current cursor"
        );
        let start = snapshot
            .records
            .partition_point(|record| record.end.end_byte_offset <= self.position.end_byte_offset);
        Ok(&snapshot.records[start..])
    }

    /// Acknowledge a successfully consumed prefix of the pending evidence.
    /// Completing a capture also skips trailing records excluded by evidence selection.
    pub fn acknowledge(&mut self, snapshot: &EvidenceSnapshot, count: usize) -> anyhow::Result<()> {
        let pending = self.pending(snapshot)?;
        ensure!(
            count <= pending.len(),
            "acknowledgement exceeds pending evidence"
        );
        self.position = if count == pending.len() {
            snapshot.end
        } else if count > 0 {
            pending[count - 1].end
        } else {
            self.position
        };
        Ok(())
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
