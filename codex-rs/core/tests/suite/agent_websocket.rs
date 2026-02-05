use anyhow::Result;
use codex_core::features::Feature;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_done;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_shell_command_call;
use core_test_support::responses::start_websocket_server;
use core_test_support::responses::start_websocket_server_with_headers;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::time::Duration;

const WS_V2_BETA_HEADER_VALUE: &str = "responses_websockets=2026-02-06";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_test_codex_shell_chain() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let call_id = "shell-command-call";
    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-1"),
            ev_shell_command_call(call_id, "echo websocket"),
            ev_done(),
        ],
        vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ],
    ]])
    .await;

    let mut builder = test_codex();

    let test = builder.build_with_websocket_server(&server).await?;
    test.submit_turn("run the echo command").await?;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);

    let first = connection
        .first()
        .expect("missing first request")
        .body_json();
    let second = connection
        .get(1)
        .expect("missing second request")
        .body_json();

    assert_eq!(first["type"].as_str(), Some("response.create"));
    assert_eq!(second["type"].as_str(), Some("response.append"));

    let append_items = second
        .get("input")
        .and_then(Value::as_array)
        .expect("response.append input array");
    assert!(!append_items.is_empty());

    let output_item = append_items
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("function_call_output"))
        .expect("function_call_output in append");
    assert_eq!(
        output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_preconnect_happens_on_session_start() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;

    assert!(
        server.wait_for_handshakes(1, Duration::from_secs(2)).await,
        "expected websocket preconnect handshake during session startup"
    );

    test.submit_turn("hello").await?;

    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 1);

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_first_turn_waits_for_inflight_preconnect() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![vec![ev_response_created("resp-1"), ev_completed("resp-1")]],
        response_headers: Vec::new(),
        // Delay handshake so submit_turn() observes startup preconnect as in-flight.
        accept_delay: Some(Duration::from_millis(150)),
    }])
    .await;

    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;
    test.submit_turn("hello").await?;

    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 1);

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_v2_test_codex_shell_chain() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let call_id = "shell-command-call";
    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-1"),
            ev_shell_command_call(call_id, "echo websocket"),
            ev_completed("resp-1"),
        ],
        vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ],
    ]])
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::ResponsesWebsocketsV2);
    });

    let test = builder.build_with_websocket_server(&server).await?;
    test.submit_turn("run the echo command").await?;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);

    let first = connection
        .first()
        .expect("missing first request")
        .body_json();
    let second = connection
        .get(1)
        .expect("missing second request")
        .body_json();

    assert_eq!(first["type"].as_str(), Some("response.create"));
    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second["previous_response_id"].as_str(), Some("resp-1"));

    let create_items = second
        .get("input")
        .and_then(Value::as_array)
        .expect("response.create input array");
    assert!(!create_items.is_empty());

    let output_item = create_items
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("function_call_output"))
        .expect("function_call_output in create");
    assert_eq!(
        output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );

    let handshake = server.single_handshake();
    assert_eq!(
        handshake.header("openai-beta"),
        Some(WS_V2_BETA_HEADER_VALUE.to_string())
    );

    server.shutdown().await;
    Ok(())
}
