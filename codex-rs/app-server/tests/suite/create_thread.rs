use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
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
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_conversation_create_and_send_message_ok() -> Result<()> {
    // Mock server – we won't strictly rely on it, but provide one to satisfy any model wiring.
    let response_body = create_final_assistant_message_sse_response("Done")?;
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(&server, vec![response_body]).await;

    // Temporary Codex home with config pointing at the mock server.
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    // Start MCP server process and initialize.
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Create a conversation via the new JSON-RPC API.
    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            model: Some("o3".to_string()),
            ..Default::default()
        })
        .await?;
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await??;
    let NewConversationResponse {
        conversation_id,
        model,
        reasoning_effort: _,
        rollout_path: _,
    } = to_response::<NewConversationResponse>(new_conv_resp)?;
    assert_eq!(model, "o3");

    // Add a listener so we receive notifications for this conversation (not strictly required for this test).
    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: false,
        })
        .await?;
    let _sub: AddConversationSubscriptionResponse =
        to_response::<AddConversationSubscriptionResponse>(
            timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
            )
            .await??,
        )?;

    // Now send a user message via the wire API and expect an OK (empty object) result.
    let send_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
        })
        .await?;
    let send_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(send_id)),
    )
    .await??;
    let _ok: SendUserMessageResponse = to_response::<SendUserMessageResponse>(send_resp)?;

    // Avoid race condition by waiting for the mock server to receive the responses request.
    let deadline = std::time::Instant::now() + DEFAULT_READ_TIMEOUT;
    let requests = loop {
        let requests = response_mock.requests();
        if !requests.is_empty() {
            break requests;
        }
        if std::time::Instant::now() >= deadline {
            panic!("mock server did not receive the responses request in time");
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };

    // Verify the outbound request body matches expectations for Responses.
    let request = requests
        .first()
        .expect("mock server should have received at least one request");
    let body = request.body_json();
    assert_eq!(body["model"], json!("o3"));
    let user_texts = request.message_input_texts("user");
    assert!(
        user_texts.iter().any(|text| text == "Hello"),
        "expected user input to include Hello, got {user_texts:?}"
    );

    drop(server);
    Ok(())
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
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
