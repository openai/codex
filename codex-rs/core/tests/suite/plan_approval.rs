use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::PlanApprovalResponse;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_approval_approved_emits_immediate_background_and_plan_update_events()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    use pretty_assertions::assert_eq;

    const APPROVED_MESSAGE: &str = "Plan approved; continuing...";

    let server = start_mock_server().await;

    let call_id = "approve-plan-call";
    let proposal = json!({
        "title": "Test Plan",
        "summary": "Test plan summary",
        "plan": {
            "explanation": "Original plan explanation",
            "plan": [
                {"step": "Step 1", "status": "pending"},
                {"step": "Step 2", "status": "in_progress"},
            ]
        }
    });
    let args = json!({ "proposal": proposal }).to_string();

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, "approve_plan", &args),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "continuing"),
        ev_completed("resp-2"),
    ]);
    let second_mock = responses::mount_sse_once(&server, second_response).await;

    let test = test_codex().build(&server).await?;
    let session_model = test.session_configured.model.clone();

    let sub_id = test
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please request plan approval".into(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let plan_request = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::PlanApprovalRequest(_) | EventMsg::TaskComplete(_)
        )
    })
    .await;
    match plan_request {
        EventMsg::PlanApprovalRequest(ev) => {
            assert_eq!(ev.call_id, call_id);
            assert_eq!(ev.proposal.title, "Test Plan");
        }
        EventMsg::TaskComplete(_) => {
            panic!("expected PlanApprovalRequest before completion");
        }
        other => {
            panic!("unexpected event: {other:?}");
        }
    }

    let _ = test
        .codex
        .submit(Op::ResolvePlanApproval {
            id: sub_id,
            response: PlanApprovalResponse::Approved,
        })
        .await?;

    let mut saw_background = false;
    let mut saw_plan_update = None;
    for _ in 0..2 {
        let ev = wait_for_event(&test.codex, |event| {
            matches!(
                event,
                EventMsg::BackgroundEvent(_) | EventMsg::PlanUpdate(_) | EventMsg::TaskComplete(_)
            )
        })
        .await;
        match ev {
            EventMsg::BackgroundEvent(bg) => {
                assert_eq!(bg.message, APPROVED_MESSAGE);
                saw_background = true;
            }
            EventMsg::PlanUpdate(update) => {
                saw_plan_update = Some(update);
            }
            EventMsg::TaskComplete(_) => {
                panic!("expected background/plan update before completion");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    assert!(saw_background, "expected a BackgroundEvent after approval");

    let update = saw_plan_update.expect("expected a PlanUpdate after approval");
    assert_eq!(update.explanation, Some(APPROVED_MESSAGE.to_string()));
    let update_json = serde_json::to_value(&update)?;
    assert_eq!(
        update_json,
        json!({
            "explanation": APPROVED_MESSAGE,
            "plan": [
                {"step": "Step 1", "status": "pending"},
                {"step": "Step 2", "status": "in_progress"}
            ]
        })
    );

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TaskComplete(_))
    })
    .await;

    let req = second_mock.single_request();
    let output_text = req
        .function_call_output_text(call_id)
        .expect("approve_plan should include function_call_output");
    let output_json: serde_json::Value = serde_json::from_str(&output_text)?;
    assert_eq!(output_json["response"]["type"], "approved");

    Ok(())
}
