//! Local system aliases must admit whole patches before ordinary sandboxed execution.

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_exec_command_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case(false; "freeform")]
#[test_case(true; "shell interception")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn system_alias_patch_creates_multiple_files_and_missing_parents_without_review(
    shell: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(Ok(()), "exercises native macOS system aliases");
    let directory = tempfile::tempdir_in("/tmp")?;
    let target = directory
        .path()
        .canonicalize()?
        .join("missing/nested/evidence.json");
    let second = directory.path().join("notes.txt");
    let contents = "x".repeat(/*n*/ 300_000);
    let patch = format!(
        "*** Begin Patch\n*** Add File: {}\n+{contents}\n*** Add File: {}\n+notes\n*** End Patch",
        target.display(),
        second.display(),
    );
    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .permissions
                .approval_policy
                .set(AskForApproval::OnRequest)
                .expect("allow review in the regression");
            config
                .permissions
                .set_permission_profile(PermissionProfile::workspace_write())
                .expect("enable the default temporary-directory grants");
        })
        .build_with_auto_env(&server)
        .await?;
    let call = if shell {
        ev_exec_command_call("patch", &format!("apply_patch <<'PATCH'\n{patch}\nPATCH"))
    } else {
        ev_apply_patch_custom_tool_call("patch", &patch)
    };
    let response = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("patch"),
                call,
                ev_completed("patch"),
            ]),
            sse(vec![
                ev_assistant_message("done", "done"),
                ev_completed("done"),
            ]),
        ],
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Save both evidence files".into(),
            text_elements: vec![],
        }]))
        .await?;
    let mut auto_approved = None;
    loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::PatchApplyBegin(begin) => auto_approved = Some(begin.auto_approved),
            EventMsg::ApplyPatchApprovalRequest(event) => {
                panic!("unexpected approval: {}", event.call_id)
            }
            EventMsg::GuardianAssessment(_) => {
                panic!("permitted temp patch should not reach Guardian")
            }
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    assert_eq!(auto_approved, Some(true));
    assert_eq!(
        (
            std::fs::read_to_string(target)?,
            std::fs::read_to_string(second)?
        ),
        (format!("{contents}\n"), "notes\n".to_string()),
    );
    assert_eq!(response.requests().len(), 2);
    Ok(())
}
