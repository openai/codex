//! Owns sync reviewer checkpoint and invalidation policy for both context modes.
//! Legacy may keep its existing transcript; thread-owned mode requires current parent context.

use codex_features::Feature;
use codex_protocol::models::ResponseItem;

use crate::codex_thread::GuardianAuthorizationVersion;
use crate::config::ManagedFeatures;
use crate::context::GuardianContextMode;
use crate::context_manager::ContextManager;
use crate::session::session::Session;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ReviewContextPolicy {
    Legacy,
    LegacyWithCheckpointReuse,
    ThreadOwned,
}

impl ReviewContextPolicy {
    pub(super) fn for_context(mode: GuardianContextMode, features: &ManagedFeatures) -> Self {
        match mode {
            GuardianContextMode::ThreadOwned => Self::ThreadOwned,
            GuardianContextMode::Legacy
                if features.enabled(Feature::GuardianReuseParentCompaction) =>
            {
                Self::LegacyWithCheckpointReuse
            }
            GuardianContextMode::Legacy => Self::Legacy,
        }
    }

    pub(super) async fn root_authorization_version(
        self,
        session: &Session,
    ) -> Option<GuardianAuthorizationVersion> {
        if self != Self::ThreadOwned {
            return None;
        }
        session
            .services
            .agent_control
            .get_guardian_package(session.thread_id)
            .await
            .map(|snapshot| snapshot.authorization_version)
    }

    pub(super) fn parent_compaction(
        self,
        history: &ContextManager,
    ) -> anyhow::Result<Option<ResponseItem>> {
        if self == Self::Legacy {
            return Ok(None);
        }
        let Some(checkpoint) =
            codex_history::CompactionCheckpoint::latest(history.annotated_items())
        else {
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
