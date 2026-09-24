use super::*;
use crate::safety::PatchSandboxRoute;
use crate::session::tests::make_session_and_context;
use crate::session::tests::update_selected_settings_for_test;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::protocol::AskForApproval;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::sync::Arc;

use tempfile::tempdir;

#[test]
fn convert_apply_patch_maps_add_variant() {
    let tmp = tempdir().expect("tmp");
    let path = tmp.path().join("a.txt");
    let path_uri = PathUri::from_host_native_path(&path).expect("absolute test path");
    let action = ApplyPatchAction::new_add_for_test(&path_uri, "hello".to_string());

    let got = convert_apply_patch_to_protocol(&action);

    assert_eq!(
        got.get(path.as_path()),
        Some(&FileChange::Add {
            content: "hello".to_string()
        })
    );
}

#[tokio::test]
async fn prepare_apply_patch_uses_action_policy_before_turn_policy() {
    let (_, mut turn) = make_session_and_context().await;
    Arc::make_mut(&mut turn.config)
        .permissions
        .approval_policy
        .set(AskForApproval::Never)
        .expect("set admitted turn approval policy");
    let mut step = StepContext::for_test(Arc::new(turn));
    let step_settings = &mut Arc::get_mut(&mut step)
        .expect("unshared test step")
        .settings;
    update_selected_settings_for_test(Arc::make_mut(step_settings), |selected| {
        selected
            .approval_policy
            .set(AskForApproval::OnRequest)
            .expect("set captured approval policy");
    });
    let tmp = tempdir().expect("tmp");
    let path = tmp.path().join("outside.txt");
    let path_uri = PathUri::from_host_native_path(&path).expect("absolute test path");
    let action = ApplyPatchAction::new_add_for_test(&path_uri, "hello".to_string());
    let permission_profile = PermissionProfile::read_only();
    let file_system_policy = permission_profile.file_system_sandbox_policy();
    let mut environment = step
        .environments
        .primary()
        .expect("primary environment")
        .clone();
    environment.config_mut().permission_profile =
        PermissionProfileSnapshot::legacy(permission_profile);
    let sandbox = environment.sandbox_context(/*additional_permissions*/ None);
    let context = sandbox.policy_context();

    let prepared = prepare_apply_patch(
        &step,
        &environment,
        &PatchSandboxRoute::Platform(codex_protocol::config_types::WindowsSandboxLevel::Disabled)
            .prepare_matching(&file_system_policy, &context)
            .expect("prepare patch matching"),
        action,
    )
    .expect("issuing action policy should request approval");

    assert!(!prepared.auto_approved);
    assert!(matches!(
        prepared.exec_approval_requirement,
        ExecApprovalRequirement::NeedsApproval { .. }
    ));
}
