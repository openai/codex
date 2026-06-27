use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ModelSafetyBufferingUpdatedNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(20);
const FASTER_MODEL: &str = "gpt-fast-wire";

#[tokio::test]
async fn direct_websocket_safety_buffering_reaches_app_server_notification() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut buffering_event = responses::ev_response_created("resp-1");
    buffering_event["safety_buffering"] = json!({
        "use_cases": ["cyber"],
        "reasons": ["user_risk"],
        "faster_model": FASTER_MODEL,
    });
    let websocket_server = responses::start_websocket_server(vec![vec![
        vec![
            json!({
                "type": "codex.response.metadata",
                "headers": {"x-codex-safety-buffering-enabled": "false"},
            }),
            responses::ev_response_created("warm-1"),
            responses::ev_completed("warm-1"),
        ],
        vec![
            buffering_event,
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ],
    ]])
    .await;

    let codex_home = TempDir::new()?;
    create_websocket_config(
        codex_home.path(),
        &websocket_server.uri().replacen("ws://", "http://", 1),
    )?;
    let mut mcp = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_request = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_request)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_response)?;

    let turn_request = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![UserInput::Text {
                text: "Check this request".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_request)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response(turn_response)?;

    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("model/safetyBuffering/updated"),
    )
    .await??;
    let notification: ModelSafetyBufferingUpdatedNotification =
        serde_json::from_value(notification.params.expect("notification params"))?;

    assert_eq!(notification.thread_id, thread.id);
    assert_eq!(notification.turn_id, turn.id);
    assert_eq!(notification.model, "mock-model");
    assert_eq!(notification.use_cases, ["cyber"]);
    assert_eq!(notification.reasons, ["user_risk"]);
    assert!(notification.show_buffering_ui);
    assert_eq!(notification.faster_model.as_deref(), Some(FASTER_MODEL));

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    websocket_server.shutdown().await;
    Ok(())
}

fn create_websocket_config(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
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
supports_websockets = true
"#
        ),
    )
}
