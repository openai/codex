use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_chat_completions_server;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionConfiguredNotification;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_resume_emits_session_configured_notification() -> Result<()> {
    let server = create_mock_chat_completions_server(vec![]).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a thread.
    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5-codex".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread } = to_response::<ThreadStartResponse>(start_resp)?;

    // Resume it via v2 API.
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id.clone(),
            history: None,
            model: None,
            model_provider: None,
            cwd: None,
            approval_policy: None,
            sandbox: None,
            config: None,
            base_instructions: None,
            developer_instructions: None,
        })
        .await?;
    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("sessionConfigured"),
    )
    .await??;
    let session_configured: ServerNotification = notification.try_into()?;
    let ServerNotification::SessionConfigured(SessionConfiguredNotification { session_id, .. }) =
        session_configured
    else {
        unreachable!("expected sessionConfigured notification");
    };
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse { thread: resumed } =
        to_response::<ThreadResumeResponse>(resume_resp)?;
    assert_eq!(resumed, thread);

    Ok(())
}

#[tokio::test]
async fn thread_resume_supports_history_and_overrides() -> Result<()> {
    let server = create_mock_chat_completions_server(vec![]).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a thread.
    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5-codex".to_string()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread } = to_response::<ThreadStartResponse>(start_resp)?;

    let history = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "Hello from history".to_string(),
        }],
    }];

    // Resume with explicit history and override the model.
    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread.id,
            history: Some(history),
            model: Some("mock-model".to_string()),
            model_provider: Some("mock_provider".to_string()),
            cwd: None,
            approval_policy: None,
            sandbox: None,
            config: None,
            base_instructions: None,
            developer_instructions: None,
        })
        .await?;
    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("sessionConfigured"),
    )
    .await??;
    let session_configured: ServerNotification = notification.try_into()?;
    let ServerNotification::SessionConfigured(SessionConfiguredNotification {
        session_id,
        model,
        initial_messages,
        ..
    }) = session_configured
    else {
        unreachable!("expected sessionConfigured notification");
    };
    assert_eq!(model, "mock-model");
    assert!(initial_messages.as_ref().map_or(true, Vec::is_empty));

    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ThreadResumeResponse { thread: resumed } =
        to_response::<ThreadResumeResponse>(resume_resp)?;
    assert_eq!(resumed.id, session_id.to_string());
    assert!(!resumed.id.is_empty());

    Ok(())
}

// Helper to create a config.toml pointing at the mock model server.
fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
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
wire_api = "chat"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
