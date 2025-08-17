use std::path::Path;

use mcp_test_support::McpProcess;
use mcp_test_support::create_final_assistant_message_sse_response;
use mcp_test_support::create_mock_chat_completions_server;
use mcp_types::JSONRPCResponse;
use mcp_types::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_conversation_create_and_send_message_ok() {
    // Mock server – we won't strictly rely on it, but provide one to satisfy any model wiring.
    let responses = vec![
        create_final_assistant_message_sse_response("Done").expect("build mock assistant message"),
    ];
    let server = create_mock_chat_completions_server(responses).await;

    // Temporary Codex home with config pointing at the mock server.
    let codex_home = TempDir::new().expect("create temp dir");
    create_config_toml(codex_home.path(), &server.uri()).expect("write config.toml");

    // Start MCP server process and initialize.
    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    // Create a conversation via the new tool.
    let req_id = mcp
        .send_conversation_create_tool_call("", "o3", "/repo")
        .await
        .expect("send conversationCreate");

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await
    .expect("create response timeout")
    .expect("create response error");

    // Structured content must include status=ok, a UUID conversation_id and the model we passed.
    let sc = &resp.result["structuredContent"];
    let conv_id = sc["conversation_id"].as_str().expect("uuid string");
    assert!(!conv_id.is_empty());
    assert_eq!(sc["model"], json!("o3"));

    // Now send a message to the created conversation and expect an OK result.
    let send_id = mcp
        .send_user_message_tool_call("Hello", conv_id)
        .await
        .expect("send message");

    let send_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(send_id)),
    )
    .await
    .expect("send response timeout")
    .expect("send response error");
    assert_eq!(
        send_resp.result["structuredContent"],
        json!({ "status": "ok" })
    );

    // avoid race condition by waiting for the mock server to receive the chat.completions request
    let deadline = std::time::Instant::now() + DEFAULT_READ_TIMEOUT;
    loop {
        let requests = server.received_requests().await.unwrap_or_default();
        if !requests.is_empty() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            panic!("mock server did not receive the chat.completions request in time");
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // Verify the outbound request body matches expectations for Chat Completions.
    let request = &server.received_requests().await.unwrap()[0];
    let body = request
        .body_json::<serde_json::Value>()
        .expect("parse request body as JSON");
    assert_eq!(body["model"], json!("o3"));
    assert!(body["stream"].as_bool().unwrap_or(false));
    let messages = body["messages"]
        .as_array()
        .expect("messages should be array");
    let last = messages.last().expect("at least one message");
    assert_eq!(last["role"], json!("user"));
    assert_eq!(last["content"], json!("Hello"));

    drop(server);
}

// Helper to create a config.toml pointing at the mock model server.
fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
