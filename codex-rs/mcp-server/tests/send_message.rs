use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use codex_mcp_server::CodexToolCallParam;
use mcp_test_support::McpProcess;
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
    // Spin up a mock completions server that immediately ends the Codex turn.
    // Two Codex turns hit the mock model (session start + send-user-message). Provide two SSE responses.
    let responses = vec![
        create_final_assistant_message_sse_response("Done").expect("build mock assistant message"),
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

    // Kick off a Codex session so we have a valid session_id.
    let codex_request_id = mcp_process
        .send_codex_tool_call(CodexToolCallParam {
            prompt: "Start a session".to_string(),
            ..Default::default()
        })
        .await
        .expect("send codex tool call");

    // Wait for the session_configured event to get the session_id.
    let session_id = mcp_process
        .read_stream_until_configured_response_message()
        .await
        .expect("read session_configured");

    // The original codex call will finish quickly given our mock; consume its response.
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp_process.read_stream_until_response_message(RequestId::Integer(codex_request_id)),
    )
    .await
    .expect("codex response timeout")
    .expect("codex response error");

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
