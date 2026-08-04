use super::*;
use crate::context::world_state::test_support::render_section_cases;
use codex_execpolicy::Decision;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ContentItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ApprovalMessages;
use codex_protocol::openai_models::PermissionMessages;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use serde_json::json;
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
fn approved_prefix_is_rendered_without_reinjecting_permissions() {
    use PreviousSectionState::Known;

    let without_approved_prefix = permissions_state_with_default_messages(&Policy::empty());
    let mut exec_policy = Policy::empty();
    exec_policy
        .add_prefix_rule(
            &["touch".to_string(), "allow-prefix.txt".to_string()],
            Decision::Allow,
        )
        .expect("test prefix should be valid");
    let with_approved_prefix = permissions_state_with_default_messages(&exec_policy);
    let approved_prefix = r#"["touch", "allow-prefix.txt"]"#;
    let without_snapshot = without_approved_prefix.snapshot();
    let with_snapshot = with_approved_prefix.snapshot();
    let rendered_update = with_approved_prefix
        .render_diff(Known(&without_snapshot))
        .expect("approving a prefix should render a world-state update")
        .render();

    assert_ne!(without_snapshot, with_snapshot);
    assert_ne!(
        without_approved_prefix.instructions,
        with_approved_prefix.instructions
    );
    assert!(
        !without_approved_prefix
            .instructions
            .body()
            .contains(approved_prefix)
    );
    assert!(
        with_approved_prefix
            .instructions
            .body()
            .contains(approved_prefix)
    );
    assert_eq!(
        rendered_update,
        "Approved command prefix saved:\n- [\"touch\", \"allow-prefix.txt\"]"
    );
    assert!(!rendered_update.contains("<permissions instructions>"));

    let before_approval_permissions =
        permissions_state(PermissionProfile::read_only(), AskForApproval::OnRequest)
            .instructions
            .render();
    let model_visible_before_and_after = format!(
        "BEFORE APPROVAL — PERMISSIONS ALREADY IN CONTEXT (CONDENSED)\n{before_approval_permissions}\n\nAFTER APPROVAL — WORLD-STATE DIFF APPENDS\n{rendered_update}\n\nFULL PERMISSIONS BLOCK APPENDED AFTER APPROVAL\nNone",
    );
    insta::assert_snapshot!(
        "approved_prefix_is_rendered_without_reinjecting_permissions",
        model_visible_before_and_after
    );
}

#[test]
fn renders_only_newly_approved_prefixes() {
    use PreviousSectionState::Known;

    let mut exec_policy = Policy::empty();
    exec_policy
        .add_prefix_rule(&["git".to_string(), "pull".to_string()], Decision::Allow)
        .expect("test prefix should be valid");
    let with_existing_prefix = permissions_state_with_default_messages(&exec_policy);
    exec_policy
        .add_prefix_rule(&["cargo".to_string(), "test".to_string()], Decision::Allow)
        .expect("test prefix should be valid");
    let with_new_prefix = permissions_state_with_default_messages(&exec_policy);
    let existing_snapshot = with_existing_prefix.snapshot();
    let current_snapshot = with_new_prefix.snapshot();

    assert_eq!(
        with_new_prefix
            .render_diff(Known(&existing_snapshot))
            .map(|fragment| fragment.render()),
        Some("Approved command prefix saved:\n- [\"cargo\", \"test\"]".to_string())
    );
    assert!(
        with_new_prefix
            .render_diff(Known(&current_snapshot))
            .is_none()
    );
}

#[test]
fn legacy_snapshot_deserializes_and_only_suppresses_matching_full_permissions() {
    use super::super::WorldState;
    use super::super::WorldStateSnapshot;

    let without_approved_prefix = permissions_state_with_default_messages(&Policy::empty());
    let mut exec_policy = Policy::empty();
    exec_policy
        .add_prefix_rule(&["touch".to_string()], Decision::Allow)
        .expect("test prefix should be valid");
    let with_approved_prefix = permissions_state_with_default_messages(&exec_policy);
    let matching_legacy: WorldStateSnapshot = serde_json::from_value(json!({
        "permissions": WorldStateHash::from_fragment(&with_approved_prefix.instructions),
    }))
    .expect("legacy world-state snapshot should deserialize");
    let stale_legacy: WorldStateSnapshot = serde_json::from_value(json!({
        "permissions": WorldStateHash::from_fragment(&without_approved_prefix.instructions),
    }))
    .expect("legacy world-state snapshot should deserialize");
    let expected_permissions = with_approved_prefix.instructions.render();
    let mut world_state = WorldState::default();
    world_state.add_section(with_approved_prefix);

    assert!(world_state.render_diff(&matching_legacy).is_empty());
    assert_eq!(
        world_state
            .render_diff(&stale_legacy)
            .into_iter()
            .map(|fragment| fragment.render())
            .collect::<Vec<_>>(),
        vec![expected_permissions]
    );
}

#[test]
fn removing_an_approved_prefix_renders_full_permissions() {
    use PreviousSectionState::Known;

    let mut exec_policy = Policy::empty();
    exec_policy
        .add_prefix_rule(&["touch".to_string()], Decision::Allow)
        .expect("test prefix should be valid");
    let with_approved_prefix = permissions_state_with_default_messages(&exec_policy);
    let without_approved_prefix = permissions_state_with_default_messages(&Policy::empty());

    let rendered = without_approved_prefix
        .render_diff(Known(&with_approved_prefix.snapshot()))
        .expect("removing a prefix should refresh permissions")
        .render();

    assert_eq!(rendered, without_approved_prefix.instructions.render());
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
        never: None,
        unless_trusted: None,
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

fn permissions_state_with_default_messages(exec_policy: &Policy) -> PermissionsState {
    PermissionsState::new(
        &PermissionProfile::read_only(),
        AskForApproval::OnRequest,
        ApprovalPromptContext::new(
            ApprovalsReviewer::User,
            /*messages*/ None,
            /*permission_messages*/ None,
        ),
        exec_policy,
        Path::new("/workspace"),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    )
}
