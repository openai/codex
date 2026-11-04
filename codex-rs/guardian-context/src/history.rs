//! Bounded, chronological review evidence independent of the parent's compaction.
//!
//! User messages and other entries have separate limits and keep their original order.
//! Hosts append original envelopes, restore bounded checkpoints, and trim rolled-back turns.
//! Compaction output is excluded, and acceptance order survives parent compaction and resume.
//! Clones share immutable payloads; eviction changes the generation so readers cannot
//! reuse an offset into a different retained prefix. Prompt selection remains caller-owned.
//! Fixed storage bounds entry counts; elastic storage uses byte limits only.
//! Non-user overflow evicts whole entries in chunks to leave room for appends.
//! User messages retain as much history as their separate limits allow.

use std::collections::VecDeque;
use std::io;
use std::io::Write;
use std::sync::Arc;

use codex_context_fragments::COMPACTION_SUMMARY_KIND;
use codex_protocol::models::ContentItem;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::executed_tool_call_metadata_bytes;

use crate::ContextBudgeting;
use crate::SectionHistory;
use codex_history::GuardianHistoryCheckpoint;
use codex_history::ResponseItemEnvelope;
use codex_history::RetainedInputSource;

const MAX_ITEMS_PER_KIND: usize = 128;
const NON_USER_RETENTION_DIVISOR: usize = 2;
// Encoded payloads (including omitted reasoning) plus the fixed ResponseItem size.
// Each kind gets half the storage budget; tool traffic cannot evict user messages.
const MAX_BYTES_PER_KIND: usize = 4 * 1024 * 1024;
const NON_USER_LOW_WATERMARK_BYTES: usize = MAX_BYTES_PER_KIND / NON_USER_RETENTION_DIVISOR;

/// Thread-owned review history with shared payloads and bounded storage.
#[derive(Clone, Default)]
pub struct TranscriptHistory {
    items: VecDeque<(Arc<ResponseItemEnvelope>, usize)>,
    generation: u64,
    budgeting: ContextBudgeting,
}

impl std::fmt::Debug for TranscriptHistory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TranscriptHistory")
            .field("items", &self.items.len())
            .field("generation", &self.generation)
            .finish()
    }
}

impl TranscriptHistory {
    /// Selects storage policy before capture; clones and reset keep the selected policy.
    pub fn new(generation: u64, budgeting: ContextBudgeting) -> Self {
        Self {
            generation,
            budgeting,
            ..Self::default()
        }
    }

    /// Appends one original item, evicting only older entries of the same kind.
    /// Fixed non-user overflow removes at least half the existing entries.
    /// Elastic overflow evicts whole non-user entries toward half the byte allowance.
    /// Oversized user images fall back to bounded text; other oversized items are skipped.
    pub fn record(&mut self, envelope: &ResponseItemEnvelope) {
        let item = &envelope.item;
        if envelope
            .metadata
            .as_ref()
            .is_some_and(|metadata| metadata.compaction_output)
            || matches!(
                item,
                ResponseItem::Compaction { .. }
                    | ResponseItem::ContextCompaction { .. }
                    | ResponseItem::CompactionTrigger { .. }
            )
            || matches!(
                item,
                ResponseItem::Message {
                    internal_chat_message_metadata_passthrough: Some(
                        InternalChatMessageMetadataPassthrough {
                            content_item_kinds: Some(kinds),
                            ..
                        }
                    ),
                    ..
                } if kinds.iter().any(|kind| kind.as_str() == COMPACTION_SUMMARY_KIND)
            )
        {
            return;
        }
        let metadata_bytes = executed_tool_call_metadata_bytes(item);
        let mut size = BoundedSize {
            bytes: std::mem::size_of::<ResponseItemEnvelope>(),
            max_bytes: MAX_BYTES_PER_KIND.saturating_add(metadata_bytes),
        };
        let measured = serde_json::to_writer(&mut size, item).and_then(|()| {
            serde_json::to_writer(&mut size, &envelope.metadata)?;
            // ResponseItem serialization can omit reasoning content that cloning retains.
            // Charge it separately, conservatively counting serialized reasoning twice.
            if let ResponseItem::Reasoning { content, .. } = item {
                serde_json::to_writer(&mut size, content)?;
            }
            Ok(())
        });
        if measured.is_err() {
            if let ResponseItem::Message {
                id,
                role,
                content,
                phase,
                internal_chat_message_metadata_passthrough,
                ..
            } = item
                && role == "user"
                && content
                    .iter()
                    .any(|item| matches!(item, ContentItem::InputImage { .. }))
            {
                // Measure text and metadata before cloning, without copying image data.
                size.bytes = std::mem::size_of::<ResponseItemEnvelope>();
                if serde_json::to_writer(
                    &mut size,
                    &(id, role, phase, internal_chat_message_metadata_passthrough),
                )
                .is_err()
                {
                    return;
                }
                let mut text = Vec::new();
                for item in content.iter().filter(|item| {
                    matches!(
                        item,
                        ContentItem::InputText { .. } | ContentItem::OutputText { .. }
                    )
                }) {
                    if serde_json::to_writer(&mut size, item).is_err() {
                        return;
                    }
                    text.push(item.clone());
                }
                if !text.is_empty() {
                    self.record(&ResponseItemEnvelope {
                        item: ResponseItem::Message {
                            status: None,
                            encrypted_content: None,
                            id: id.clone(),
                            role: role.clone(),
                            content: text,
                            phase: phase.clone(),
                            internal_chat_message_metadata_passthrough:
                                internal_chat_message_metadata_passthrough.clone(),
                        },
                        metadata: envelope.metadata.clone(),
                    });
                }
            }
            return;
        }
        // Analytics must not change which evidence fits Guardian's retention budget.
        size.bytes = size.bytes.saturating_sub(metadata_bytes);
        let is_user = item.is_user_message();
        let (mut count, mut bytes) = self
            .items
            .iter()
            .filter(|(item, _)| item.is_user_message() == is_user)
            .fold((0, size.bytes), |(count, bytes), (_, size)| {
                (count + 1, bytes + size)
            });
        if (self.budgeting == ContextBudgeting::Fixed && count >= MAX_ITEMS_PER_KIND)
            || bytes > MAX_BYTES_PER_KIND
        {
            let retained_count = match (self.budgeting, is_user) {
                (ContextBudgeting::Elastic, _) => count,
                (ContextBudgeting::Fixed, true) => MAX_ITEMS_PER_KIND - 1,
                (ContextBudgeting::Fixed, false) => count / NON_USER_RETENTION_DIVISOR,
            };
            let retained_bytes = if self.budgeting == ContextBudgeting::Elastic && !is_user {
                NON_USER_LOW_WATERMARK_BYTES.max(size.bytes)
            } else {
                MAX_BYTES_PER_KIND
            };
            self.items.retain(|(item, size)| {
                if item.is_user_message() == is_user
                    && (count > retained_count || bytes > retained_bytes)
                {
                    count -= 1;
                    bytes -= size;
                    false
                } else {
                    true
                }
            });
            self.generation = self.generation.saturating_add(1);
        }
        self.items
            .push_back((Arc::new(envelope.clone()), size.bytes));
    }

    /// Replaces evidence after a host history reset; compaction must not call this.
    pub fn reset<'a>(&mut self, items: impl IntoIterator<Item = &'a ResponseItemEnvelope>) {
        self.items.clear();
        self.generation = self.generation.saturating_add(1);
        for item in items {
            self.record(item);
        }
    }

    /// Removes a rolled-back boundary and everything after it. If retention already
    /// evicted the boundary, discard the remaining evidence rather than keep later grants.
    pub fn truncate_before(&mut self, boundary: &ResponseItemEnvelope) {
        let index = self.items.iter().position(|(item, _)| {
            item.id()
                .zip(boundary.id())
                .is_some_and(|(item_id, boundary_id)| item_id == boundary_id)
                || item.item == boundary.item
        });
        self.items.truncate(index.unwrap_or(0));
        if let Some(boundary_order) =
            RetainedInputSource::from(boundary.metadata.as_ref()).acceptance_order()
        {
            self.items.retain(|(entry, _)| {
                match RetainedInputSource::from(entry.metadata.as_ref()) {
                    RetainedInputSource::Local(Some(order)) => order < boundary_order,
                    // Old backups may lack delivery ordering. Do not keep ambiguous assistant
                    // sources ahead of a queued input; retained facts preserve earlier evidence.
                    RetainedInputSource::Local(None) => {
                        !matches!(&entry.item, ResponseItem::Message { role, .. } if role == "assistant")
                            && !matches!(&entry.item, ResponseItem::FunctionCall { .. })
                    }
                    RetainedInputSource::Inherited => true,
                }
            });
        }
        self.generation = self.generation.saturating_add(1);
    }

    /// Saves the same bounded items and provenance used by live rollback.
    pub fn checkpoint(&self) -> GuardianHistoryCheckpoint {
        GuardianHistoryCheckpoint(
            self.items
                .iter()
                .map(|(item, _)| item.as_ref().clone())
                .collect(),
        )
    }

    /// Reviewable evidence with its host provenance still attached.
    pub fn reviewable_items(&self) -> impl Iterator<Item = &ResponseItemEnvelope> + Send + '_ {
        self.items
            .iter()
            .map(|(item, _)| item.as_ref())
            .filter(|item| !crate::is_inherited_manual_approval(item))
    }

    /// Generation of the retained prefix, independent of parent compaction.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl SectionHistory for TranscriptHistory {
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.reviewable_items().map(|item| &item.item))
    }
}

// Count without allocating a serialized copy, and stop before cloning an oversized item.
struct BoundedSize {
    bytes: usize,
    max_bytes: usize,
}

impl Write for BoundedSize {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.max_bytes.saturating_sub(self.bytes) {
            return Err(io::Error::other(
                "review history item exceeds storage budget",
            ));
        }
        self.bytes += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
