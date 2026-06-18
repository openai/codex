use std::path::Path;

use anyhow::Result;
use anyhow::bail;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::CurrentTimeReadResponse;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

#[cfg(windows)]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const CURRENT_TIME_AT: i64 = 1_781_717_655;
const CURRENT_TIME_REMINDER: &str = "It is 2026-06-17 17:34:15 UTC.";

#[tokio::test]
async fn current_time_read_round_trip_adds_reminder_to_model_input() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let codex_home = TempDir::new()?;
    write_config(codex_home.path(), &server.uri())?;

    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.initialize_with_capabilities(
            ClientInfo {
                name: "codex-app-server-tests".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            Some(InitializeCapabilities {
                experimental_api: true,
                request_current_time: true,
                ..Default::default()
            }),
        ),
    )
    .await??;

    let thread_request_id = app_server
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(thread_request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_response)?;

    let turn_request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "What time is it?".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(turn_request_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response(turn_response)?;

    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            match app_server.read_next_message().await? {
                JSONRPCMessage::Request(request) => {
                    let request = ServerRequest::try_from(request)?;
                    let ServerRequest::CurrentTimeRead { request_id, params } = request else {
                        bail!("expected currentTime/read request, got {request:?}");
                    };
                    assert_eq!(params.thread_id, thread.id);
                    app_server
                        .send_response(
                            request_id,
                            serde_json::to_value(CurrentTimeReadResponse {
                                current_time_at: CURRENT_TIME_AT,
                            })?,
                        )
                        .await?;
                }
                JSONRPCMessage::Notification(notification)
                    if notification.method == "turn/completed" =>
                {
                    break Ok(());
                }
                _ => {}
            }
        }
    })
    .await??;

    assert!(
        response_mock
            .single_request()
            .message_input_texts("developer")
            .contains(&CURRENT_TIME_REMINDER.to_string())
    );
    Ok(())
}

fn write_config(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"

[features.current_time_reminder]
enabled = true
reminder_interval_model_requests = 1
clock_source = "app_server_client"

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
