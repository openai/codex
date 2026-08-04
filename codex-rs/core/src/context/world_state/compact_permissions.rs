use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::APPROVED_COMMAND_PREFIX_SAVED_MESSAGE_PREFIX;
use crate::context::ApprovedCommandPrefixSaved;
use crate::context::ContextualUserFragment;
use codex_execpolicy::Policy;
use codex_protocol::models::format_allow_prefixes;
use std::collections::BTreeSet;

/// Newly approved command prefixes visible without the full permissions instructions.
#[derive(Clone, Debug)]
pub(crate) struct CompactPermissionsState {
    prefixes: BTreeSet<Vec<String>>,
}

impl CompactPermissionsState {
    pub(crate) fn new(exec_policy: &Policy) -> Self {
        Self {
            prefixes: exec_policy.get_allowed_prefixes().into_iter().collect(),
        }
    }
}

impl WorldStateSection for CompactPermissionsState {
    const ID: &'static str = "approved_command_prefixes";
    type Snapshot = BTreeSet<Vec<String>>;

    fn snapshot(&self) -> Self::Snapshot {
        self.prefixes.clone()
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer"
            && text
                .trim_start()
                .starts_with(APPROVED_COMMAND_PREFIX_SAVED_MESSAGE_PREFIX)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        let added_prefixes = match previous {
            PreviousSectionState::Known(previous) => self
                .prefixes
                .difference(previous)
                .cloned()
                .collect::<Vec<_>>(),
            PreviousSectionState::Absent | PreviousSectionState::Unknown => return None,
        };
        format_allow_prefixes(added_prefixes)
            .filter(|prefixes| !prefixes.is_empty())
            .map(|prefixes| Box::new(ApprovedCommandPrefixSaved::new(prefixes)) as _)
    }
}

#[cfg(test)]
#[path = "compact_permissions_tests.rs"]
mod tests;
