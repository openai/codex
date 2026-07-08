use super::*;
use crate::context::ApprovalPromptContext;
use codex_execpolicy::Decision;
use codex_execpolicy::Policy;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ApprovalMessages;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use std::path::Path;

fn permissions_state(
    model_slug: &str,
    approval_policy: AskForApproval,
    reviewer: ApprovalsReviewer,
    on_request: Option<&str>,
    on_request_auto_review: Option<&str>,
) -> PermissionsState {
    let messages = ApprovalMessages {
        on_request: on_request.map(str::to_string),
        on_request_auto_review: on_request_auto_review.map(str::to_string),
    };
    let permission_profile = PermissionProfile::Disabled;
    let instructions = PermissionsInstructions::from_permission_profile(
        &permission_profile,
        approval_policy,
        ApprovalPromptContext::new(reviewer, Some(&messages)),
        &Policy::empty(),
        Path::new("."),
        /*exec_permission_approvals_enabled*/ false,
        /*request_permissions_tool_enabled*/ false,
    );
    PermissionsState::enabled(instructions, model_slug)
}

fn render(
    state: &PermissionsState,
    previous: PreviousSectionState<'_, PermissionsSnapshot>,
) -> Vec<String> {
    state
        .render_diff(previous)
        .into_iter()
        .map(|fragment| fragment.render())
        .collect()
}

#[test]
fn renders_when_reviewer_or_catalog_message_changes() {
    let user = permissions_state(
        "model-a",
        AskForApproval::OnRequest,
        ApprovalsReviewer::User,
        Some("user approvals v1"),
        Some("auto approvals"),
    );
    let same_snapshot = user.snapshot();
    let auto = permissions_state(
        "model-a",
        AskForApproval::OnRequest,
        ApprovalsReviewer::AutoReview,
        Some("user approvals v1"),
        Some("auto approvals"),
    );
    let updated_catalog = permissions_state(
        "model-a",
        AskForApproval::OnRequest,
        ApprovalsReviewer::User,
        Some("user approvals v2"),
        Some("auto approvals"),
    );

    assert_eq!(render(&user, PreviousSectionState::Absent).len(), 1);
    assert_eq!(
        render(&user, PreviousSectionState::Known(&same_snapshot)),
        Vec::<String>::new()
    );
    assert!(
        render(&auto, PreviousSectionState::Known(&same_snapshot))[0].contains("auto approvals")
    );
    assert!(
        render(
            &updated_catalog,
            PreviousSectionState::Known(&same_snapshot)
        )[0]
        .contains("user approvals v2")
    );
    assert_eq!(
        render(&PermissionsState::disabled(), PreviousSectionState::Absent),
        Vec::<String>::new()
    );
}

#[test]
fn missing_and_empty_catalog_messages_have_distinct_snapshots() {
    let missing = permissions_state(
        "model-a",
        AskForApproval::OnRequest,
        ApprovalsReviewer::User,
        None,
        None,
    );
    let empty = permissions_state(
        "model-a",
        AskForApproval::OnRequest,
        ApprovalsReviewer::User,
        Some(""),
        None,
    );

    assert_ne!(missing.snapshot(), empty.snapshot());
}

#[test]
fn model_changes_render_even_when_catalog_messages_match() {
    let model_a = permissions_state(
        "model-a",
        AskForApproval::OnRequest,
        ApprovalsReviewer::User,
        Some("shared approvals"),
        None,
    );
    let model_b = permissions_state(
        "model-b",
        AskForApproval::OnRequest,
        ApprovalsReviewer::User,
        Some("shared approvals"),
        None,
    );
    let expected = render(&model_b, PreviousSectionState::Absent);

    assert_eq!(
        render(&model_b, PreviousSectionState::Known(&model_a.snapshot())),
        expected
    );
}

#[test]
fn reviewer_changes_are_ignored_when_approval_policy_is_never() {
    let user = permissions_state(
        "model-a",
        AskForApproval::Never,
        ApprovalsReviewer::User,
        None,
        None,
    );
    let auto = permissions_state(
        "model-a",
        AskForApproval::Never,
        ApprovalsReviewer::AutoReview,
        None,
        None,
    );

    assert_eq!(user.snapshot(), auto.snapshot());
    assert!(render(&auto, PreviousSectionState::Known(&user.snapshot())).is_empty());
}

#[test]
fn approved_prefix_changes_rendered_permissions_snapshot() {
    let permission_profile = PermissionProfile::Disabled;
    let make_state = |exec_policy: &Policy| {
        let instructions = PermissionsInstructions::from_permission_profile(
            &permission_profile,
            AskForApproval::OnRequest,
            ApprovalPromptContext::new(ApprovalsReviewer::User, /*messages*/ None),
            exec_policy,
            Path::new("."),
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        );
        PermissionsState::enabled(instructions, "model-a")
    };
    let before = make_state(&Policy::empty());
    let mut policy_with_prefix = Policy::empty();
    policy_with_prefix
        .add_prefix_rule(&["git".to_string(), "pull".to_string()], Decision::Allow)
        .expect("add prefix rule");
    let after = make_state(&policy_with_prefix);

    assert_ne!(before.snapshot(), after.snapshot());
    assert!(
        render(&after, PreviousSectionState::Known(&before.snapshot()))[0]
            .contains(r#"["git", "pull"]"#)
    );
}

#[test]
fn legacy_permissions_are_reinjected_to_establish_a_snapshot() {
    let state = permissions_state(
        "model-a",
        AskForApproval::OnRequest,
        ApprovalsReviewer::User,
        Some("user approvals"),
        Some("auto approvals"),
    );
    let legacy: ResponseItem = ContextualUserFragment::into(
        state
            .instructions
            .clone()
            .expect("enabled state should have instructions"),
    );
    let mut world_state = super::super::WorldState::default();
    world_state.add_section(state);

    assert_eq!(
        world_state
            .render_history_diff(/*previous*/ None, &[legacy])
            .len(),
        1
    );
}

#[test]
fn persisted_permissions_are_restored_only_when_missing_from_history() {
    let state = permissions_state(
        "model-a",
        AskForApproval::OnRequest,
        ApprovalsReviewer::User,
        Some("user approvals"),
        Some("auto approvals"),
    );
    let retained: ResponseItem = ContextualUserFragment::into(
        state
            .instructions
            .clone()
            .expect("enabled state should have instructions"),
    );
    let mut world_state = super::super::WorldState::default();
    world_state.add_section(state);
    let snapshot = world_state.snapshot();

    assert_eq!(
        world_state.render_history_diff(Some(&snapshot), &[]).len(),
        1
    );
    assert!(
        world_state
            .render_history_diff(Some(&snapshot), &[retained])
            .is_empty()
    );
}
