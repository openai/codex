#![allow(clippy::expect_used)]

use std::thread::sleep;
use std::time::Duration;

use mcp_test_support::McpProcess;
use mcp_test_support::create_config_toml;
use mcp_test_support::create_final_assistant_message_sse_response;
use mcp_test_support::create_mock_chat_completions_server;
use mcp_types::JSONRPC_VERSION;
use mcp_types::JSONRPCResponse;
use mcp_types::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn test_send_message_success() {
    // Spin up a mock completions server that ends the Codex turn for the send-user-message call.
    let responses = vec![
        create_final_assistant_message_sse_response("Done").expect("build mock assistant message"),
    ];
    let server = create_mock_chat_completions_server(responses).await;

    // Create a temporary Codex home with config pointing at the mock server.
    let codex_home = TempDir::new().expect("create temp dir");
    create_config_toml(codex_home.path(), &server.uri()).expect("write config.toml");

    // Start MCP server process and initialize.
    let mut mcp_process = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp_process.initialize())
        .await
        .expect("init timed out")
        .expect("init failed");

    // Create a conversation using the tool and get its conversation_id
    let session_id = mcp_process
        .create_conversation_and_get_id("", "mock-model", "/repo")
        .await
        .expect("create conversation");

    // Now exercise the send-user-message tool.
    let send_msg_request_id = mcp_process
        .send_user_message_tool_call("Hello again", &session_id)
        .await
        .expect("send send-message tool call");

    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp_process.read_stream_until_response_message(RequestId::Integer(send_msg_request_id)),
    )
    .await
    .expect("send-user-message response timeout")
    .expect("send-user-message response error");

    assert_eq!(
        JSONRPCResponse {
            jsonrpc: JSONRPC_VERSION.into(),
            id: RequestId::Integer(send_msg_request_id),
            result: json!({
                "content": [
                    {
                        "text": "{\"status\":\"ok\"}",
                        "type": "text",
                    }
                ],
                "isError": false,
                "structuredContent": {
                    "status": "ok"
                }
            }),
        },
        response
    );
    // wait for the server to hear the user message
    sleep(Duration::from_secs(5));

    // Ensure the server and tempdir live until end of test
    drop(server);
}

#[tokio::test]
async fn test_send_message_session_not_found() {
    // Start MCP without creating a Codex session
    let codex_home = TempDir::new().expect("tempdir");
    let mut mcp = McpProcess::new(codex_home.path()).await.expect("spawn");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("timeout")
        .expect("init");

    let unknown = uuid::Uuid::new_v4().to_string();
    let req_id = mcp
        .send_user_message_tool_call("ping", &unknown)
        .await
        .expect("send tool");

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await
    .expect("timeout")
    .expect("resp");

    let result = resp.result.clone();
    let content = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(content.contains("Session does not exist"));
    assert_eq!(result["isError"], json!(true));
}

// Helpers are provided by tests/common
