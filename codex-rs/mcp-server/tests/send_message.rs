use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use codex_mcp_server::wire_format::ConversationId;
use codex_mcp_server::wire_format::InputItem;
use codex_mcp_server::wire_format::NewConversationParams;
use codex_mcp_server::wire_format::NewConversationResponse;
use codex_mcp_server::wire_format::SendUserMessageParams;
use codex_mcp_server::wire_format::SendUserMessageResponse;
use mcp_test_support::McpProcess;
use mcp_test_support::create_final_assistant_message_sse_response;
use mcp_test_support::create_mock_chat_completions_server;
use mcp_types::JSONRPCResponse;
use mcp_types::RequestId;
use pretty_assertions::assert_eq;
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

    // Start a conversation using the new wire API.
    let new_conv_id = mcp_process
        .send_new_conversation_request(NewConversationParams::default())
        .await
        .expect("send newConversation");
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp_process.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await
    .expect("newConversation timeout")
    .expect("newConversation resp");
    let NewConversationResponse {
        conversation_id, ..
    } = to_response::<NewConversationResponse>(new_conv_resp)
        .expect("deserialize newConversation response");

    // Now exercise sendUserMessage.
    let send_id = mcp_process
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: "Hello again".to_string(),
            }],
        })
        .await
        .expect("send sendUserMessage");

    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp_process.read_stream_until_response_message(RequestId::Integer(send_id)),
    )
    .await
    .expect("sendUserMessage response timeout")
    .expect("sendUserMessage response error");

    let _ok: SendUserMessageResponse = to_response::<SendUserMessageResponse>(response)
        .expect("deserialize sendUserMessage response");
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

    let unknown = ConversationId(uuid::Uuid::new_v4());
    let req_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id: unknown,
            items: vec![InputItem::Text {
                text: "ping".to_string(),
            }],
        })
        .await
        .expect("send sendUserMessage");

    // Expect an error response for unknown conversation.
    let err = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(req_id)),
    )
    .await
    .expect("timeout")
    .expect("error");
    assert_eq!(err.id, RequestId::Integer(req_id));
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

fn to_response<T: serde::de::DeserializeOwned>(response: JSONRPCResponse) -> anyhow::Result<T> {
    let value = serde_json::to_value(response.result)?;
    let codex_response = serde_json::from_value(value)?;
    Ok(codex_response)
}
