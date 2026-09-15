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
            .root_user_authorization(session.thread_id)
            .await
            .map(|snapshot| snapshot.authorization_version)
    }

    pub(super) fn parent_compaction(
        self,
        history: &ContextManager,
        reviewer_compaction_hash: Option<&str>,
    ) -> anyhow::Result<Option<ResponseItem>> {
        let strict = self == Self::ThreadOwned;
        if self == Self::Legacy {
            return Ok(None);
        }
        let Some(envelope) = history.annotated_items().iter().rev().find(|envelope| {
            matches!(
                envelope.item,
                ResponseItem::Compaction { .. } | ResponseItem::ContextCompaction { .. }
            )
        }) else {
            return Ok(None);
        };

        let item = &envelope.item;
        let valid = match item {
            ResponseItem::Compaction {
                id: Some(_),
                encrypted_content,
                ..
            } if !encrypted_content.is_empty() => true,
            ResponseItem::ContextCompaction {
                id: Some(_),
                encrypted_content: Some(encrypted_content),
                ..
            } if !encrypted_content.is_empty() => true,
            _ => false,
        };
        if !valid && !strict {
            return Ok(None);
        }
        anyhow::ensure!(
            valid,
            "parent compaction checkpoint is unusable for Guardian review"
        );
        if strict {
            // A resumed parent may now use a different model. Compare the actual
            // checkpoint producer with the selected reviewer, not the live parent model.
            anyhow::ensure!(
                GuardianContextMode::ThreadOwned
                    .for_checkpoint(history.annotated_items(), reviewer_compaction_hash)
                    == GuardianContextMode::ThreadOwned,
                "parent compaction checkpoint is incompatible with the Guardian review model or its compatibility is unknown"
            );
        }
        Ok(Some(item.clone()))
    }
}
