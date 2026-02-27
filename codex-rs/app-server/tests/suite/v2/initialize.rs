use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::fs_wait;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn initialize_uses_client_info_name_as_originator() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "codex_vscode".to_string(),
            title: Some("Codex VS Code Extension".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Response(response) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };
    let InitializeResponse { user_agent } = to_response::<InitializeResponse>(response)?;

    assert!(user_agent.starts_with("codex_vscode/"));
    Ok(())
}

#[tokio::test]
async fn initialize_respects_originator_override_env_var() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[(
            "CODEX_INTERNAL_ORIGINATOR_OVERRIDE",
            Some("codex_originator_via_env_var"),
        )],
    )
    .await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "codex_vscode".to_string(),
            title: Some("Codex VS Code Extension".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Response(response) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };
    let InitializeResponse { user_agent } = to_response::<InitializeResponse>(response)?;

    assert!(user_agent.starts_with("codex_originator_via_env_var/"));
    Ok(())
}

#[tokio::test]
async fn initialize_rejects_invalid_client_name() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[("CODEX_INTERNAL_ORIGINATOR_OVERRIDE", None)],
    )
    .await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "bad\rname".to_string(),
            title: Some("Bad Client".to_string()),
            version: "0.1.0".to_string(),
        }),
    )
    .await??;

    let JSONRPCMessage::Error(error) = message else {
        anyhow::bail!("expected initialize error, got {message:?}");
    };

    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "Invalid clientInfo.name: 'bad\rname'. Must be a valid HTTP header value."
    );
    assert_eq!(error.error.data, None);
    Ok(())
}

#[tokio::test]
async fn initialize_opt_out_notification_methods_filters_notifications() -> Result<()> {
    let responses = Vec::new();
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

    let message = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_capabilities(
            ClientInfo {
                name: "codex_vscode".to_string(),
                title: Some("Codex VS Code Extension".to_string()),
                version: "0.1.0".to_string(),
            },
            Some(InitializeCapabilities {
                experimental_api: true,
                opt_out_notification_methods: Some(vec![
                    "thread/started".to_string(),
                    "codex/event/session_configured".to_string(),
                ]),
            }),
        ),
    )
    .await??;
    let JSONRPCMessage::Response(_) = message else {
        anyhow::bail!("expected initialize response, got {message:?}");
    };

    let request_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let response = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let message = mcp.read_next_message().await?;
            match message {
                JSONRPCMessage::Response(response)
                    if response.id == RequestId::Integer(request_id) =>
                {
                    return Ok(response);
                }
                JSONRPCMessage::Notification(notification)
                    if notification.method == "thread/started" =>
                {
                    anyhow::bail!("thread/started should be filtered by optOutNotificationMethods");
                }
                _ => {}
            }
        }
    })
    .await??;
    let _: ThreadStartResponse = to_response(response)?;

    let thread_started = timeout(
        std::time::Duration::from_millis(500),
        mcp.read_stream_until_notification_message("thread/started"),
    )
    .await;
    assert!(
        thread_started.is_err(),
        "thread/started should be filtered by optOutNotificationMethods"
    );
    Ok(())
}

#[tokio::test]
async fn turn_start_notify_payload_includes_initialize_client_name() -> Result<()> {
    let responses = vec![create_final_assistant_message_sse_response("Done")?];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    let notify_script = codex_home.path().join("notify.py");
    std::fs::write(
        &notify_script,
        r#"from pathlib import Path
import sys

Path(__file__).with_name("notify.json").write_text(sys.argv[-1], encoding="utf-8")
"#,
    )?;
    let notify_file = codex_home.path().join("notify.json");
    let notify_script = notify_script
        .to_str()
        .expect("notify script path should be valid UTF-8");
    create_config_toml_with_extra(
        codex_home.path(),
        &server.uri(),
        "never",
        &format!(
            "notify = [\"python3\", {}]",
            toml_basic_string(notify_script)
        ),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_client_info(ClientInfo {
            name: "xcode".to_string(),
            title: Some("Xcode".to_string()),
            version: "1.0.0".to_string(),
        }),
    )
    .await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![V2UserInput::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _: TurnStartResponse = to_response(turn_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    fs_wait::wait_for_path_exists(&notify_file, Duration::from_secs(5)).await?;
    let payload_raw = tokio::fs::read_to_string(&notify_file).await?;
    let payload: Value = serde_json::from_str(&payload_raw)?;
    assert_eq!(payload["client"], "xcode");

    Ok(())
}

// Helper to create a config.toml pointing at the mock model server.
fn create_config_toml(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
) -> std::io::Result<()> {
    create_config_toml_with_extra(codex_home, server_uri, approval_policy, "")
}

fn create_config_toml_with_extra(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
    extra: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "read-only"

model_provider = "mock_provider"

{extra}

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

fn toml_basic_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
