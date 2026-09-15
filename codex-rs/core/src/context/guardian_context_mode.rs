//! Session capture policy and the reviewer policy carried by each history snapshot.
//! Older checkpoints keep legacy review while its transcript preserves the retained evidence.

use codex_extension_api::ConversationHistorySnapshot;
use codex_features::Feature;
use codex_features::Features;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;

/// Selects legacy compatibility or thread-owned evidence.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GuardianContextMode {
    #[default]
    Legacy,
    ThreadOwned,
}

impl GuardianContextMode {
    /// Read reviewer policy from the same snapshot as its evidence, including delayed reviews.
    pub fn from_history(history: &dyn ConversationHistorySnapshot) -> Self {
        if history.retained_context().is_some() {
            Self::ThreadOwned
        } else {
            Self::Legacy
        }
    }

    pub(crate) fn from_features(features: &Features) -> Self {
        if features.enabled(Feature::GuardianThreadContext) {
            Self::ThreadOwned
        } else {
            Self::Legacy
        }
    }

    pub(crate) fn for_checkpoint(self, items: &[ResponseItemEnvelope]) -> Self {
        let checkpoint = items.iter().rev().find(|envelope| {
            matches!(
                envelope.item,
                ResponseItem::Compaction { .. } | ResponseItem::ContextCompaction { .. }
            )
        });
        if checkpoint.is_some_and(|checkpoint| {
            checkpoint
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.compaction_model_hash.as_deref())
                .is_none_or(str::is_empty)
        }) {
            Self::Legacy
        } else {
            self
        }
    }
}
