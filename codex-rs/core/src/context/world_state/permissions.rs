use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use crate::context::PermissionsInstructions;
use crate::session::turn_context::TurnContext;
use codex_execpolicy::Policy;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::TurnContextItem;

#[derive(Debug, PartialEq)]
struct PermissionValues {
    permission_profile: PermissionProfile,
    approval_policy: AskForApproval,
}

#[derive(Debug)]
pub(crate) struct PermissionsState {
    values: PermissionValues,
    instructions: Option<PermissionsInstructions>,
}

impl PermissionsState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext, exec_policy: &Policy) -> Self {
        let instructions = turn_context
            .config
            .include_permissions_instructions
            .then(|| {
                PermissionsInstructions::from_permission_profile(
                    &turn_context.permission_profile,
                    turn_context.approval_policy.value(),
                    turn_context.config.approvals_reviewer,
                    exec_policy,
                    #[allow(deprecated)]
                    &turn_context.cwd,
                    turn_context
                        .config
                        .features
                        .enabled(Feature::ExecPermissionApprovals),
                    turn_context
                        .config
                        .features
                        .enabled(Feature::RequestPermissionsTool),
                )
            });
        Self {
            values: PermissionValues {
                permission_profile: turn_context.permission_profile(),
                approval_policy: turn_context.approval_policy.value(),
            },
            instructions,
        }
    }

    pub(crate) fn from_turn_context_item(turn_context_item: &TurnContextItem) -> Self {
        Self {
            values: PermissionValues {
                permission_profile: turn_context_item.permission_profile(),
                approval_policy: turn_context_item.approval_policy,
            },
            instructions: None,
        }
    }
}

impl WorldStateSection for PermissionsState {
    fn render_diff(&self, previous: Option<&Self>) -> Option<Box<dyn ContextualUserFragment>> {
        let instructions = self.instructions.as_ref()?;
        previous
            .is_none_or(|previous| self.values != previous.values)
            .then(|| Box::new(instructions.clone()) as Box<dyn ContextualUserFragment>)
    }
}
