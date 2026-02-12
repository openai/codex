use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::ArchiveConversationParams;
use codex_app_server_protocol::ArchiveConversationResponse;
use codex_app_server_protocol::InputItem;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_conversation_moves_rollout_into_archived_directory() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let new_request_id = mcp
        .send_new_conversation_request(NewConversationParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let new_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_request_id)),
    )
    .await??;

    let NewConversationResponse {
        conversation_id,
        rollout_path,
        ..
    } = to_response::<NewConversationResponse>(new_response)?;

    assert!(
        !rollout_path.exists(),
        "expected rollout path {} to be deferred until first user message",
        rollout_path.display()
    );

    let add_listener_request_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: false,
        })
        .await?;
    let add_listener_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(add_listener_request_id)),
    )
    .await??;
    let AddConversationSubscriptionResponse { subscription_id: _ } =
        to_response::<AddConversationSubscriptionResponse>(add_listener_response)?;

    let send_request_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: "materialize".to_string(),
                text_elements: Vec::new(),
            }],
        })
        .await?;
    let send_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(send_request_id)),
    )
    .await??;
    let _: SendUserMessageResponse = to_response::<SendUserMessageResponse>(send_response)?;
    let _: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;

    assert!(
        rollout_path.exists(),
        "expected rollout path {} to exist after first user message",
        rollout_path.display()
    );

    let archive_request_id = mcp
        .send_archive_conversation_request(ArchiveConversationParams {
            conversation_id,
            rollout_path: rollout_path.clone(),
        })
        .await?;
    let archive_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(archive_request_id)),
    )
    .await??;

    let _: ArchiveConversationResponse =
        to_response::<ArchiveConversationResponse>(archive_response)?;

    let archived_directory = codex_home.path().join(ARCHIVED_SESSIONS_SUBDIR);
    let archived_rollout_path =
        archived_directory.join(rollout_path.file_name().unwrap_or_else(|| {
            panic!("rollout path {} missing file name", rollout_path.display())
        }));

    assert!(
        !rollout_path.exists(),
        "expected rollout path {} to be moved",
        rollout_path.display()
    );
    assert!(
        archived_rollout_path.exists(),
        "expected archived rollout path {} to exist",
        archived_rollout_path.display()
    );

    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(config_toml, config_contents(server_uri))
}

fn config_contents(server_uri: &str) -> String {
    format!(
        r#"model = "mock-model"
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
    )
}
