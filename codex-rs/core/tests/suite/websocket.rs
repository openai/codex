use anyhow::Result;
use codex_core::WireApi;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_done;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_shell_command_call;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;

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
            ev_done(),
        ],
    ]])
    .await;

    let ws_base_url = format!("{}/v1", server.uri());
    let mock_server = start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider.base_url = Some(ws_base_url);
        config.model_provider.wire_api = WireApi::ResponsesWebsocket;
    });

    let test = builder.build(&mock_server).await?;
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
