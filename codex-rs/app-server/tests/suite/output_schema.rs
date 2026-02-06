use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::InputItem;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SendUserTurnParams;
use codex_app_server_protocol::SendUserTurnResponse;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ReasoningEffort;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn send_user_turn_accepts_output_schema_v1() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            ..Default::default()
        })
        .await?;
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await??;
    let NewConversationResponse {
        conversation_id, ..
    } = to_response::<NewConversationResponse>(new_conv_resp)?;

    let listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: false,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(listener_id)),
    )
    .await??;

    let output_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "answer": { "type": "string" }
        },
        "required": ["answer"],
        "additionalProperties": false
    });

    let send_turn_id = mcp
        .send_send_user_turn_request(SendUserTurnParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: codex_home.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: "mock-model".to_string(),
            effort: Some(ReasoningEffort::Medium),
            summary: ReasoningSummary::Auto,
            output_schema: Some(output_schema.clone()),
        })
        .await?;
    let _send_turn_resp: SendUserTurnResponse = to_response::<SendUserTurnResponse>(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(send_turn_id)),
        )
        .await??,
    )?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    let request = response_mock.single_request();
    let payload = request.body_json();
    let text = payload.get("text").expect("request missing text field");
    let format = text
        .get("format")
        .expect("request missing text.format field");
    assert_eq!(
        format,
        &serde_json::json!({
            "name": "codex_output_schema",
            "type": "json_schema",
            "strict": true,
            "schema": output_schema,
        })
    );

    Ok(())
}

#[tokio::test]
async fn send_user_turn_output_schema_is_per_turn_v1() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body1 = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock1 = responses::mount_sse_once(&server, body1).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            ..Default::default()
        })
        .await?;
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await??;
    let NewConversationResponse {
        conversation_id, ..
    } = to_response::<NewConversationResponse>(new_conv_resp)?;

    let listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: false,
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(listener_id)),
    )
    .await??;

    let output_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "answer": { "type": "string" }
        },
        "required": ["answer"],
        "additionalProperties": false
    });

    let send_turn_id = mcp
        .send_send_user_turn_request(SendUserTurnParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: codex_home.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: "mock-model".to_string(),
            effort: Some(ReasoningEffort::Medium),
            summary: ReasoningSummary::Auto,
            output_schema: Some(output_schema.clone()),
        })
        .await?;
    let _send_turn_resp: SendUserTurnResponse = to_response::<SendUserTurnResponse>(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(send_turn_id)),
        )
        .await??,
    )?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    let payload1 = response_mock1.single_request().body_json();
    assert_eq!(
        payload1.pointer("/text/format"),
        Some(&serde_json::json!({
            "name": "codex_output_schema",
            "type": "json_schema",
            "strict": true,
            "schema": output_schema,
        }))
    );

    let body2 = responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_assistant_message("msg-2", "Done"),
        responses::ev_completed("resp-2"),
    ]);
    let response_mock2 = responses::mount_sse_once(&server, body2).await;

    let send_turn_id_2 = mcp
        .send_send_user_turn_request(SendUserTurnParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: "Hello again".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: codex_home.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: "mock-model".to_string(),
            effort: Some(ReasoningEffort::Medium),
            summary: ReasoningSummary::Auto,
            output_schema: None,
        })
        .await?;
    let _send_turn_resp_2: SendUserTurnResponse = to_response::<SendUserTurnResponse>(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(send_turn_id_2)),
        )
        .await??,
    )?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    let payload2 = response_mock2.single_request().body_json();
    assert_eq!(payload2.pointer("/text/format"), None);

    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

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
