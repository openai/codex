#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_done;
use core_test_support::responses::ev_reasoning_item;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_shell_command_call;
use core_test_support::responses::mount_response_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::start_websocket_server_with_headers;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

const TURN_STATE_HEADER: &str = "x-codex-turn-state";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_turn_state_persists_within_turn_and_resets_after() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "shell-turn-state";

    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_reasoning_item("rsn-1", &["thinking"], &[]),
        ev_shell_command_call(call_id, "echo turn-state"),
        ev_completed("resp-1"),
    ]);
    let second_response = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let third_response = sse(vec![
        ev_response_created("resp-3"),
        ev_assistant_message("msg-2", "done"),
        ev_completed("resp-3"),
    ]);

    // First response sets turn_state; follow-up request in the same turn should echo it.
    let responses = vec![
        sse_response(first_response).insert_header(TURN_STATE_HEADER, "ts-1"),
        sse_response(second_response),
        sse_response(third_response),
    ];
    let request_log = mount_response_sequence(&server, responses).await;

    let test = test_codex().build(&server).await?;
    test.submit_turn("run a shell command").await?;
    test.submit_turn("second turn").await?;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 3);
    // Initial turn request has no header; follow-up has it; next turn clears it.
    assert_eq!(requests[0].header(TURN_STATE_HEADER), None);
    assert_eq!(
        requests[1].header(TURN_STATE_HEADER),
        Some("ts-1".to_string())
    );
    assert_eq!(requests[2].header(TURN_STATE_HEADER), None);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_turn_state_persists_within_turn_and_resets_after() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let call_id = "ws-shell-turn-state";
    // First connection delivers turn_state; second (same turn) must send it; third (new turn) must not.
    let server = start_websocket_server_with_headers(vec![
        WebSocketConnectionConfig {
            requests: vec![vec![
                ev_response_created("resp-1"),
                ev_reasoning_item("rsn-1", &["thinking"], &[]),
                ev_shell_command_call(call_id, "echo websocket"),
                ev_done(),
            ]],
            response_headers: vec![(TURN_STATE_HEADER.to_string(), "ts-1".to_string())],
        },
        WebSocketConnectionConfig {
            requests: vec![vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]],
            response_headers: Vec::new(),
        },
        WebSocketConnectionConfig {
            requests: vec![vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-3"),
            ]],
            response_headers: Vec::new(),
        },
    ])
    .await;

    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;
    test.submit_turn("run the echo command").await?;
    test.submit_turn("second turn").await?;

    let handshakes = server.handshakes();
    assert_eq!(handshakes.len(), 3);
    assert_eq!(handshakes[0].header(TURN_STATE_HEADER), None);
    assert_eq!(
        handshakes[1].header(TURN_STATE_HEADER),
        Some("ts-1".to_string())
    );
    assert_eq!(handshakes[2].header(TURN_STATE_HEADER), None);

    server.shutdown().await;
    Ok(())
}
