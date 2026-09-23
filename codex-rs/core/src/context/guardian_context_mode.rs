//! Reviewer policy carried by each history snapshot.
//! Unknown or incompatible checkpoints keep legacy review alongside retained user evidence.

use codex_extension_api::ConversationHistorySnapshot;
use codex_history::ResponseItemEnvelope;

/// Selects checkpoint compatibility review or thread-owned evidence.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GuardianContextMode {
    Legacy,
    #[default]
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

    pub(crate) fn for_checkpoint(
        items: &[ResponseItemEnvelope],
        reviewer_compaction_hash: Option<&str>,
    ) -> Self {
        if codex_history::CompactionCheckpoint::latest(items)
            .is_none_or(|checkpoint| checkpoint.is_compatible_with(reviewer_compaction_hash))
        {
            Self::ThreadOwned
        } else {
            Self::Legacy
        }
    }
}
