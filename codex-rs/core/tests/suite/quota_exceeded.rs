use anyhow::Result;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quota_exceeded_emits_single_error_event() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            json!({
                "type": "response.failed",
                "response": {
                    "id": "resp-1",
                    "error": {
                        "code": "insufficient_quota",
                        "message": "You exceeded your current quota, please check your plan and billing details."
                    }
                }
            }),
        ]),
    )
    .await;

    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "quota?".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await
        .unwrap();

    let mut error_events = 0;

    loop {
        let event = wait_for_event(&test.codex, |_| true).await;

        match event {
            EventMsg::Error(err) => {
                error_events += 1;
                assert_eq!(
                    err.message,
                    "Quota exceeded. Check your plan and billing details."
                );
            }
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    assert_eq!(error_events, 1, "expected exactly one Codex:Error event");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quota_exceeded_does_not_auto_send_pending_steer() -> Result<()> {
    assert_usage_limit_like_failure_does_not_auto_send_pending_steer(
        "insufficient_quota",
        "You exceeded your current quota.",
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn usage_not_included_does_not_auto_send_pending_steer() -> Result<()> {
    assert_usage_limit_like_failure_does_not_auto_send_pending_steer(
        "usage_not_included",
        "Usage is not included with this plan.",
    )
    .await
}

async fn assert_usage_limit_like_failure_does_not_auto_send_pending_steer(
    code: &str,
    message: &str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    mount_response_once(
        &server,
        sse_response(sse(vec![
            ev_response_created("resp-1"),
            json!({
                "type": "response.failed",
                "response": {
                    "id": "resp-1",
                    "error": {
                        "code": code,
                        "message": message,
                    }
                }
            }),
        ]))
        .set_delay(std::time::Duration::from_millis(100)),
    )
    .await;
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await?;
    wait_for_event(&test.codex, |msg| matches!(msg, EventMsg::TurnStarted(_))).await;

    test.codex
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "steer while blocked".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await?;

    wait_for_event(&test.codex, |msg| matches!(msg, EventMsg::Error(_))).await;
    wait_for_event(&test.codex, |msg| matches!(msg, EventMsg::TurnComplete(_))).await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    assert_eq!(
        server.received_requests().await.unwrap_or_default().len(),
        1
    );

    Ok(())
}
