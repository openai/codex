//! Session capture policy and the reviewer policy carried by each history snapshot.
//! Unknown or incompatible checkpoints keep legacy review alongside retained user evidence.

use codex_extension_api::ConversationHistorySnapshot;
use codex_features::Feature;
use codex_features::Features;
use codex_history::ResponseItemEnvelope;

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
        if history.uses_parent_context_for_review() {
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

    pub(crate) fn for_checkpoint(
        self,
        items: &[ResponseItemEnvelope],
        reviewer_compaction_hash: Option<&str>,
    ) -> Self {
        if codex_history::CompactionCheckpoint::latest(items)
            .is_none_or(|checkpoint| checkpoint.is_compatible_with(reviewer_compaction_hash))
        {
            self
        } else {
            Self::Legacy
        }
    }
}
