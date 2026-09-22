//! Collects persisted rollout items needed to reconstruct the most recent context window.
//!
//! Storage readers feed items newest-to-oldest. The scan stops at the newest compaction that has
//! both replacement history and a window number, or at the beginning of the rollout when no such
//! compaction exists. Items are returned in chronological order.

use crate::RolloutItem;

/// Whether a reverse model-context scan needs more rollout items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelContextScanProgress {
    /// The reader should provide the next older rollout item.
    Continue,
    /// The scan has collected a safe bounded suffix.
    Complete,
}

/// Finds a bounded suffix for reconstructing the most recent context window.
///
/// A compaction with replacement history and a window number is a complete conversation-history
/// boundary. Records after it provide any companion state that was persisted. Older compactions
/// missing either field require the complete rollout so reconstruction can rebuild their history
/// or window number.
#[derive(Debug, Default)]
pub struct ModelContextScan {
    items_newest_first: Vec<RolloutItem>,
    requires_full_replay: bool,
}

impl ModelContextScan {
    /// Adds the next newest-to-oldest rollout item and reports whether the reader can stop.
    pub fn push(&mut self, item: RolloutItem) -> ModelContextScanProgress {
        let progress = if self.requires_full_replay {
            ModelContextScanProgress::Continue
        } else if let RolloutItem::Compacted(compacted) = &item {
            if compacted.replacement_history.is_some() && compacted.window_number.is_some() {
                ModelContextScanProgress::Complete
            } else {
                // This compaction cannot be reconstructed from a bounded suffix. Do not stop at
                // an older compaction because this newer one still affects the surviving history.
                self.requires_full_replay = true;
                ModelContextScanProgress::Continue
            }
        } else {
            ModelContextScanProgress::Continue
        };
        self.items_newest_first.push(item);
        progress
    }

    /// Returns the collected items in chronological order.
    ///
    /// Call this after the reader reaches the beginning of its source or after [`Self::push`]
    /// returns [`ModelContextScanProgress::Complete`].
    pub fn finish(mut self) -> Vec<RolloutItem> {
        self.items_newest_first.reverse();
        self.items_newest_first
    }
}
