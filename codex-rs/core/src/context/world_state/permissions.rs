use super::PreviousSectionState;
use super::WorldStateHash;
use super::WorldStateSection;
use crate::context::ApprovalPromptContext;
use crate::context::ApprovedCommandPrefixSaved;
use crate::context::ContextualUserFragment;
use crate::context::PermissionsInstructions;
use codex_execpolicy::Policy;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::format_allow_prefixes;
use codex_protocol::protocol::AskForApproval;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

/// Permission instructions currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct PermissionsState {
    snapshot: PermissionsSnapshot,
    instructions: PermissionsInstructions,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum PermissionsSnapshot {
    Current {
        instructions: WorldStateHash,
        approved_command_prefixes: BTreeSet<Vec<String>>,
    },
    Legacy(WorldStateHash),
}

impl PermissionsState {
    pub(crate) fn new(
        permission_profile: &PermissionProfile,
        approval_policy: AskForApproval,
        approval_context: ApprovalPromptContext<'_>,
        exec_policy: &Policy,
        cwd: &Path,
        exec_permission_approvals_enabled: bool,
        request_permissions_tool_enabled: bool,
    ) -> Self {
        let build_instructions = |exec_policy| {
            PermissionsInstructions::from_permission_profile(
                permission_profile,
                approval_policy,
                approval_context,
                exec_policy,
                cwd,
                exec_permission_approvals_enabled,
                request_permissions_tool_enabled,
            )
        };
        let instructions = build_instructions(exec_policy);
        let instructions_without_approved_prefixes = build_instructions(&Policy::empty());
        let snapshot = PermissionsSnapshot::Current {
            instructions: WorldStateHash::from_fragment(&instructions_without_approved_prefixes),
            approved_command_prefixes: exec_policy.get_allowed_prefixes().into_iter().collect(),
        };
        Self {
            snapshot,
            instructions,
        }
    }
}

impl WorldStateSection for PermissionsState {
    const ID: &'static str = "permissions";
    type Snapshot = PermissionsSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        self.snapshot.clone()
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && PermissionsInstructions::matches_text(text)
    }

    fn has_retained_fragment_matcher() -> bool {
        true
    }

    fn matches_retained_fragment(role: &str, text: &str) -> bool {
        Self::matches_legacy_fragment(role, text)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        match (previous, &self.snapshot) {
            (
                PreviousSectionState::Known(PermissionsSnapshot::Current {
                    instructions: previous_instructions,
                    approved_command_prefixes: previous_prefixes,
                }),
                PermissionsSnapshot::Current {
                    instructions,
                    approved_command_prefixes,
                },
            ) if previous_instructions == instructions => {
                if previous_prefixes == approved_command_prefixes {
                    return None;
                }
                if previous_prefixes.is_subset(approved_command_prefixes) {
                    let added_prefixes = approved_command_prefixes
                        .difference(previous_prefixes)
                        .cloned()
                        .collect();
                    if let Some(prefixes) = format_allow_prefixes(added_prefixes) {
                        return Some(Box::new(ApprovedCommandPrefixSaved::new(prefixes)));
                    }
                }
            }
            (
                PreviousSectionState::Known(PermissionsSnapshot::Legacy(previous)),
                PermissionsSnapshot::Current { .. },
            ) if previous == &WorldStateHash::from_fragment(&self.instructions) => return None,
            _ => {}
        }

        Some(Box::new(self.instructions.clone()))
    }
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
