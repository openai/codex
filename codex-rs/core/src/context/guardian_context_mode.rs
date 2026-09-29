//! Reviewer policy carried by each history snapshot.
//! Independent review retains its own transcript; other modes select checkpoint compatibility.

use codex_extension_api::ConversationHistorySnapshot;
use codex_history::ResponseItemEnvelope;

/// Selects checkpoint-based, legacy, or independent reviewer history.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GuardianContextMode {
    Legacy,
    /// Thread-owned authorization with a transcript independent of parent compaction.
    Independent,
    #[default]
    ThreadOwned,
}

impl GuardianContextMode {
    /// Stable, bounded label for the context actually selected for a review.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Independent => "independent",
            Self::ThreadOwned => "thread_owned",
        }
    }

    /// Read reviewer policy from the same snapshot as its evidence, including delayed reviews.
    pub fn from_history(history: &dyn ConversationHistorySnapshot) -> Self {
        if history.uses_independent_review_history() {
            Self::Independent
        } else if history.uses_parent_context_for_review() {
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
