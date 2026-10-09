//! Bounded, chronological review evidence independent of the parent's compaction.
//!
//! User messages and other entries have separate limits and keep their original order.
//! Hosts append original envelopes, restore bounded checkpoints, and trim rolled-back turns.
//! Compaction output is excluded, and acceptance order survives parent compaction and resume.
//! Clones share immutable payloads; eviction changes the generation so readers cannot
//! reuse an offset into a different retained prefix. Prompt selection remains caller-owned.
//! Non-user overflow drops the oldest half, leaving room for appends without more evictions.
//! User messages retain as much history as their separate limits allow.

use std::collections::VecDeque;
use std::io;
use std::io::Write;
use std::sync::Arc;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::executed_tool_call_metadata_bytes;

use crate::SectionHistory;
use codex_history::GuardianHistoryCheckpoint;
use codex_history::ResponseItemEnvelope;
use codex_history::RetainedInputSource;

const MAX_ITEMS_PER_KIND: usize = 128;
// Encoded payloads (including omitted reasoning) plus the fixed ResponseItem size.
// Each kind gets half the storage budget; tool traffic cannot evict user messages.
const MAX_BYTES_PER_KIND: usize = 4 * 1024 * 1024;

/// Thread-owned review history with shared payloads and bounded storage.
#[derive(Clone, Default)]
pub struct TranscriptHistory {
    items: VecDeque<(Arc<ResponseItemEnvelope>, usize)>,
    generation: u64,
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
    /// Starts retention in a generation newer than the host's previous review snapshot.
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            ..Self::default()
        }
    }

    /// Appends one original item, evicting only older entries of the same kind.
    /// Non-user overflow removes at least half the existing entries; byte limits may need more.
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
            || matches!(item, ResponseItem::Message { internal_chat_message_metadata_passthrough: Some(metadata), .. }
                if metadata.content_item_kinds.as_ref().is_some_and(|kinds|
                    kinds.iter().any(|kind| kind.0 == "compaction.summary")))
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
        if count >= MAX_ITEMS_PER_KIND || bytes > MAX_BYTES_PER_KIND {
            let retained_count = if is_user {
                MAX_ITEMS_PER_KIND - 1
            } else {
                count / 2
            };
            self.items.retain(|(item, size)| {
                if item.is_user_message() == is_user
                    && (count > retained_count || bytes > MAX_BYTES_PER_KIND)
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

    /// Generation of the retained prefix, independent of parent compaction.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl SectionHistory for TranscriptHistory {
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(
            self.items
                .iter()
                .filter(|(item, _)| !crate::is_inherited_manual_approval(item))
                .map(|(item, _)| &item.item),
        )
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
