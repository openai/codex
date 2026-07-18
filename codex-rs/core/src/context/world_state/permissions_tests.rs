use super::*;
use crate::context::world_state::test_support::render_section_cases;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ContentItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ApprovalMessages;
use codex_protocol::openai_models::PermissionMessages;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use std::path::Path;

#[test]
fn snapshots() {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let read_only = permissions_state(PermissionProfile::read_only(), AskForApproval::OnRequest);
    let full_access = permissions_state(PermissionProfile::Disabled, AskForApproval::OnRequest);
    let never_ask = permissions_state(PermissionProfile::read_only(), AskForApproval::Never);

    insta::assert_snapshot!(render_section_cases(&[
        (Absent, Absent),
        (Absent, Known(&read_only)),
        (Known(&read_only), Known(&read_only)),
        (Known(&read_only), Known(&full_access)),
        (Known(&read_only), Known(&never_ask)),
        (Unknown, Known(&read_only)),
    ]));
}

#[test]
fn persisted_permissions_are_detected_inside_bundled_developer_messages() {
    let state = permissions_state(PermissionProfile::read_only(), AskForApproval::OnRequest);
    let retained = ContextualUserFragment::into(state.instructions.clone());
    let mut world_state = super::super::WorldState::default();
    world_state.add_section(state);
    let snapshot = world_state.snapshot();
    let mut bundled_retained = retained.clone();
    let ResponseItem::Message { content, .. } = &mut bundled_retained else {
        panic!("permissions should render as a message");
    };
    content.insert(
        0,
        ContentItem::InputText {
            text: "Other developer instructions.".to_string(),
        },
    );

    assert_eq!(
        world_state
            .render_history_diff(/*previous*/ None, std::slice::from_ref(&retained))
            .len(),
        1,
    );
    assert_eq!(
        world_state.render_history_diff(Some(&snapshot), &[]).len(),
        1,
    );
    assert!(
        world_state
            .render_history_diff(Some(&snapshot), &[bundled_retained])
            .is_empty()
    );
}

fn permissions_state(
    permission_profile: PermissionProfile,
    approval_policy: AskForApproval,
) -> PermissionsState {
    let approval_messages = ApprovalMessages {
        on_request: Some("Ask for approval.".to_string()),
        on_request_auto_review: None,
    };
    let permission_messages = PermissionMessages {
        danger_full_access: Some("Full access.".to_string()),
        workspace_write: Some("Workspace write.".to_string()),
        read_only: Some("Read only.".to_string()),
    };
    PermissionsState::new(
        &permission_profile,
        approval_policy,
        ApprovalPromptContext::new(
            ApprovalsReviewer::User,
            Some(&approval_messages),
            Some(&permission_messages),
        ),
        &Policy::empty(),
        Path::new("/workspace"),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    )
}
