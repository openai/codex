use std::path::Path;

use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_chat_completions_server;
use app_test_support::to_response;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::InputItem;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
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

    // Create a conversation via the new JSON-RPC API.
    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            model: Some("o3".to_string()),
            ..Default::default()
        })
        .await
        .expect("send newConversation");
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await
    .expect("newConversation timeout")
    .expect("newConversation resp");
    let NewConversationResponse {
        conversation_id,
        model,
        reasoning_effort: _,
        rollout_path: _,
    } = to_response::<NewConversationResponse>(new_conv_resp)
        .expect("deserialize newConversation response");
    assert_eq!(model, "o3");

    // Add a listener so we receive notifications for this conversation (not strictly required for this test).
    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams { conversation_id })
        .await
        .expect("send addConversationListener");
    let _sub: AddConversationSubscriptionResponse =
        to_response::<AddConversationSubscriptionResponse>(
            timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
            )
            .await
            .expect("addConversationListener timeout")
            .expect("addConversationListener resp"),
        )
        .expect("deserialize addConversationListener response");

    // Now send a user message via the wire API and expect an OK (empty object) result.
    let send_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: "Hello".to_string(),
            }],
        })
        .await
        .expect("send sendUserMessage");
    let send_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(send_id)),
    )
    .await
    .expect("sendUserMessage timeout")
    .expect("sendUserMessage resp");
    let _ok: SendUserMessageResponse = to_response::<SendUserMessageResponse>(send_resp)
        .expect("deserialize sendUserMessage response");

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
