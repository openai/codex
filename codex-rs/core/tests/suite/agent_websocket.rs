use anyhow::Result;
use codex_core::features::Feature;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_done;
use core_test_support::responses::ev_done_with_id;
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
    test.submit_turn_with_policy(
        "run the echo command",
        test.config.permissions.sandbox_policy.get().clone(),
    )
    .await?;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);

    let first_turn = connection
        .first()
        .expect("missing first turn request")
        .body_json();
    let second_turn = connection
        .get(1)
        .expect("missing second turn request")
        .body_json();

    assert_eq!(first_turn["type"].as_str(), Some("response.create"));
    assert_eq!(second_turn["type"].as_str(), Some("response.append"));

    let append_items = second_turn
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
async fn websocket_first_turn_uses_preconnect_and_create() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "hello"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;
    test.submit_turn_with_policy(
        "hello",
        test.config.permissions.sandbox_policy.get().clone(),
    )
    .await?;

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 1);
    let turn = connection
        .first()
        .expect("missing turn request")
        .body_json();
    assert!(
        turn["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty()),
        "expected request tools to be populated"
    );
    assert_eq!(turn["type"].as_str(), Some("response.create"));

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_first_turn_handles_handshake_delay_with_preconnect() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "hello"),
            ev_completed("resp-1"),
        ]],
        response_headers: Vec::new(),
        // Delay handshake so turn processing must tolerate websocket startup latency.
        accept_delay: Some(Duration::from_millis(150)),
    }])
    .await;

    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;
    test.submit_turn_with_policy(
        "hello",
        test.config.permissions.sandbox_policy.get().clone(),
    )
    .await?;

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 1);
    let turn = connection
        .first()
        .expect("missing turn request")
        .body_json();
    assert!(
        turn["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty()),
        "expected request tools to be populated"
    );
    assert_eq!(turn["type"].as_str(), Some("response.create"));

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_v2_test_codex_shell_chain() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let call_id = "shell-command-call";
    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_done_with_id("warm-1")],
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
    test.submit_turn_with_policy(
        "run the echo command",
        test.config.permissions.sandbox_policy.get().clone(),
    )
    .await?;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 3);

    let warmup = connection
        .first()
        .expect("missing warmup request")
        .body_json();
    let first_turn = connection
        .get(1)
        .expect("missing first turn request")
        .body_json();
    let second_turn = connection
        .get(2)
        .expect("missing second turn request")
        .body_json();

    assert_eq!(warmup["type"].as_str(), Some("response.create"));
    assert_eq!(warmup["generate"].as_bool(), Some(false));
    assert_eq!(first_turn["type"].as_str(), Some("response.create"));
    assert_eq!(first_turn["previous_response_id"].as_str(), Some("warm-1"));
    assert!(
        first_turn
            .get("input")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    );
    assert_eq!(second_turn["type"].as_str(), Some("response.create"));
    assert_eq!(second_turn["previous_response_id"].as_str(), Some("resp-1"));

    let create_items = second_turn
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
