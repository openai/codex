//! Owns sync reviewer checkpoint policy for each context mode.
//! Independent review retains thread-owned authorization without parent checkpoints.

use codex_features::Feature;
use codex_features::Features;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;

use crate::GuardianContextMode;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReviewContextPolicy {
    Legacy,
    LegacyWithCheckpointReuse,
    ThreadOwned,
    Independent,
}

impl ReviewContextPolicy {
    pub fn for_context(mode: GuardianContextMode, features: &Features) -> Self {
        match mode {
            GuardianContextMode::ThreadOwned => Self::ThreadOwned,
            GuardianContextMode::Independent => Self::Independent,
            GuardianContextMode::Legacy
                if features.enabled(Feature::GuardianReuseParentCompaction) =>
            {
                Self::LegacyWithCheckpointReuse
            }
            GuardianContextMode::Legacy => Self::Legacy,
        }
    }

    pub fn parent_compaction(
        self,
        history: &[ResponseItemEnvelope],
    ) -> anyhow::Result<Option<ResponseItem>> {
        if matches!(self, Self::Legacy | Self::Independent) {
            return Ok(None);
        }
        let Some(checkpoint) = codex_history::CompactionCheckpoint::latest(history) else {
            return Ok(None);
        };
        let valid = checkpoint.is_usable();
        if !valid && self == Self::LegacyWithCheckpointReuse {
            // Legacy review can use its retained transcript without this checkpoint.
            return Ok(None);
        }
        anyhow::ensure!(
            valid,
            "parent compaction checkpoint is unusable for Guardian review"
        );
        // The synchronous reviewer can consume checkpoints across advertised comp_hash
        // values. Let the backend validate the payload; review errors still fail closed.
        Ok(Some(checkpoint.item.clone()))
    }
}
