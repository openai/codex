/// E2E tests for API validation error rollback
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::json;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::*;

fn invalid_request_error(message: &str) -> ResponseTemplate {
    ResponseTemplate::new(400)
        .insert_header("content-type", "application/json")
        .set_body_json(json!({
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": message
            }
        }))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_invalid_request_triggers_rollback_and_retry() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_codex().build(&server).await?;

    let turn1 = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "First response"),
        ev_completed("resp-1"),
    ]);
    let _m1 = mount_sse_once_match(&server, any(), turn1).await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Start".into(),
            }],
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Second turn: API returns 400, then succeeds after rollback
    let turn2_success = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-2", "Recovered"),
        ev_completed("resp-2"),
    ]);
    let _m2 =
        mount_sse_once_match(&server, body_string_contains("SYSTEM ERROR"), turn2_success).await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(invalid_request_error("Invalid content"))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Continue".into(),
            }],
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_consecutive_failures_exhausts_retries() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_codex().build(&server).await?;

    let turn1 = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "First response"),
        ev_completed("resp-1"),
    ]);
    let _m1 = mount_sse_once_match(&server, any(), turn1).await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Start".into(),
            }],
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // All subsequent requests fail with 400
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(invalid_request_error("Persistent validation error"))
        .mount(&server)
        .await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Continue".into(),
            }],
        })
        .await?;

    // Should get error event after exhausting retries
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::Error(_))).await;

    Ok(())
}
