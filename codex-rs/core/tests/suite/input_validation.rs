use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_with_timeout;
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::any;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn input_validation_should_fail_for_too_large_input() {
    let server = start_mock_server().await;

    let fixture = test_codex().build(&server).await.unwrap();
    let codex = Arc::clone(&fixture.codex);

    // First: normal message with a mocked assistant response
    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(&server, any(), first_response).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello world".into(),
            }],
        })
        .await
        .unwrap();

    // Wait for the normal turn to complete before sending the oversized input
    let turn_timeout = Duration::from_secs(1);
    wait_for_event_with_timeout(
        &codex,
        |ev| matches!(ev, EventMsg::TaskComplete(_)),
        turn_timeout,
    )
    .await;

    // Then: 300k-token message should trigger validation error
    let wait_timeout = Duration::from_millis(100);
    let input_300_tokens = "token ".repeat(300_000);

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: input_300_tokens,
            }],
        })
        .await
        .unwrap();

    let error_event =
        wait_for_event_with_timeout(&codex, |ev| matches!(ev, EventMsg::Error(_)), wait_timeout)
            .await;
    let EventMsg::Error(error_event) = error_event else {
        unreachable!("wait_for_event_with_timeout returned unexpected payload");
    };
    assert_eq!(error_event.message, "invalid input: input too large");
}
