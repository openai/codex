use super::WorldStateSection;
use super::developer_message;
use crate::context::ContextualUserFragment;
use crate::context::PermissionsInstructions;
use crate::session::turn_context::TurnContext;
use codex_execpolicy::Policy;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::TurnContextItem;

#[derive(Debug, PartialEq)]
struct PermissionValues {
    permission_profile: PermissionProfile,
    approval_policy: AskForApproval,
}

#[derive(Debug, Default)]
pub(crate) struct PermissionsState {
    values: Option<PermissionValues>,
    rendered: Option<String>,
}

impl PermissionsState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext, exec_policy: &Policy) -> Self {
        let rendered = turn_context
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
                .render()
            });
        Self {
            values: Some(PermissionValues {
                permission_profile: turn_context.permission_profile(),
                approval_policy: turn_context.approval_policy.value(),
            }),
            rendered,
        }
    }

    pub(crate) fn from_turn_context_item(turn_context_item: &TurnContextItem) -> Self {
        Self {
            values: Some(PermissionValues {
                permission_profile: turn_context_item.permission_profile(),
                approval_policy: turn_context_item.approval_policy,
            }),
            rendered: None,
        }
    }
}

impl WorldStateSection for PermissionsState {
    fn render_diff(&self, previous: &Self) -> Option<ResponseItem> {
        let rendered = self.rendered.as_ref()?;
        (self.values != previous.values).then(|| developer_message(rendered.clone()))
    }
}
