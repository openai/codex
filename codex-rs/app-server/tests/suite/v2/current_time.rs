use std::path::Path;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::CurrentTimeReadResponse;
use codex_app_server_protocol::CurrentTimeSleepNotification;
use codex_app_server_protocol::CurrentTimeWakeParams;
use codex_app_server_protocol::CurrentTimeWakeResponse;
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
const TWELVE_HOURS_SECONDS: i64 = 12 * 60 * 60;
const TWELVE_HOURS_MS: u64 = TWELVE_HOURS_SECONDS as u64 * 1000;

#[tokio::test]
async fn current_time_read_round_trip_adds_reminder_to_model_input() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        create_final_assistant_message_sse_response("Done")?,
    )
    .await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut app_server = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

    let thread_request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
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

    respond_to_current_time_read(&mut app_server, &thread.id, CURRENT_TIME_AT).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    assert!(
        response_mock
            .single_request()
            .message_input_texts("developer")
            .iter()
            .any(|text| text == CURRENT_TIME_REMINDER)
    );
    Ok(())
}

#[tokio::test]
async fn external_sleep_notification_waits_for_wake_and_ignores_stale_wakes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    "sleep-call",
                    "clock",
                    "sleep",
                    &serde_json::json!({ "duration_ms": TWELVE_HOURS_MS }).to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            create_final_assistant_message_sse_response("Done")?,
        ],
    )
    .await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut app_server = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

    let thread_request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
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
                text: "Sleep".to_string(),
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

    respond_to_current_time_read(&mut app_server, &thread.id, CURRENT_TIME_AT).await?;

    let sleep_notification = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_notification_message("currentTime/sleep"),
    )
    .await??;
    let params: CurrentTimeSleepNotification =
        serde_json::from_value(sleep_notification.params.expect("currentTime/sleep params"))?;
    assert_eq!(params.thread_id, thread.id);
    assert_eq!(params.duration_ms, TWELVE_HOURS_MS);
    assert!(!params.sleep_id.is_empty());
    let sleep_id = params.sleep_id;

    assert!(
        timeout(
            Duration::from_millis(100),
            app_server.read_stream_until_notification_message("turn/completed"),
        )
        .await
        .is_err(),
        "turn completed before currentTime/wake"
    );

    let wake_params = CurrentTimeWakeParams {
        thread_id: thread.id.clone(),
        sleep_id,
    };
    let wake_request_id = app_server
        .send_raw_request(
            "currentTime/wake",
            Some(serde_json::to_value(wake_params.clone())?),
        )
        .await?;
    let wake_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(wake_request_id)),
    )
    .await??;
    let _: CurrentTimeWakeResponse = to_response(wake_response)?;

    respond_to_current_time_read(
        &mut app_server,
        &thread.id,
        CURRENT_TIME_AT + TWELVE_HOURS_SECONDS,
    )
    .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let stale_wake_request_id = app_server
        .send_raw_request("currentTime/wake", Some(serde_json::to_value(wake_params)?))
        .await?;
    let stale_wake_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(stale_wake_request_id)),
    )
    .await??;
    let _: CurrentTimeWakeResponse = to_response(stale_wake_response)?;
    Ok(())
}

async fn respond_to_current_time_read(
    app_server: &mut TestAppServer,
    thread_id: &str,
    current_time_at: i64,
) -> Result<()> {
    let server_request = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CurrentTimeRead { request_id, params } = server_request else {
        panic!("expected CurrentTimeRead request, got: {server_request:?}");
    };
    assert_eq!(params.thread_id, thread_id);
    app_server
        .send_response(
            request_id,
            serde_json::to_value(CurrentTimeReadResponse { current_time_at })?,
        )
        .await?;
    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
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
reminder_interval_seconds = 1
clock_source = "external"
sleep_tool = true

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
