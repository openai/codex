#![allow(clippy::unwrap_used)]

use codex_core::features::Feature;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::skill_approval::SkillApprovalResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_approval_event_round_trip_unblocks_turn() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "approved"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let builder = test_codex();
    let TestCodex {
        codex,
        cwd,
        session_configured,
        ..
    } = builder
        .with_config(|config| {
            config.features.enable(Feature::SkillApproval);
        })
        .build(&server)
        .await?;

    let turn_id = codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "trigger skill approval test".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_configured.model.clone(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let request = wait_for_event_match(&codex, |event| match event {
        EventMsg::SkillRequestApproval(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(request.item_id, turn_id);
    assert_eq!(request.skill_name, "test-skill");

    codex
        .submit(Op::SkillApproval {
            id: request.item_id,
            response: SkillApprovalResponse { approved: true },
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    Ok(())
}
