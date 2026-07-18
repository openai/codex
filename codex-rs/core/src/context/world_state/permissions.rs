use super::PreviousSectionState;
use super::WorldStateHash;
use super::WorldStateSection;
use crate::context::ApprovalPromptContext;
use crate::context::ContextualUserFragment;
use crate::context::PermissionsInstructions;
use codex_execpolicy::Policy;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use std::path::Path;

/// Permission instructions currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct PermissionsState {
    snapshot: WorldStateHash,
    instructions: PermissionsInstructions,
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
        let instructions = PermissionsInstructions::from_permission_profile(
            permission_profile,
            approval_policy,
            approval_context,
            exec_policy,
            cwd,
            exec_permission_approvals_enabled,
            request_permissions_tool_enabled,
        );
        let snapshot = WorldStateHash::from_fragment(&instructions);
        Self {
            snapshot,
            instructions,
        }
    }
}

impl WorldStateSection for PermissionsState {
    const ID: &'static str = "permissions";
    type Snapshot = WorldStateHash;

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
        if matches!(previous, PreviousSectionState::Known(previous) if previous == &self.snapshot) {
            return None;
        }

        Some(Box::new(self.instructions.clone()))
    }
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
