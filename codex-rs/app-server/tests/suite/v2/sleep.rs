use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::CurrentTimeReadResponse;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SleepItem;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg(windows)]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const CURRENT_TIME_AT: i64 = 1_781_717_655;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_sleep_polls_current_time_and_emits_items() -> Result<()> {
    const CALL_ID: &str = "sleep-1";
    const DURATION_MS: u64 = 2_000;

    let server = responses::start_mock_server().await;
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    CALL_ID,
                    "clock",
                    "sleep",
                    &serde_json::json!({ "duration_ms": DURATION_MS }).to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_start_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_response)?;

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Sleep briefly".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_start_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let TurnStartResponse { turn, .. } = to_response(turn_start_response)?;

    // Read once for the initial reminder, then once to establish the sleep deadline.
    respond_to_current_time_read(&mut mcp, &thread.id, CURRENT_TIME_AT).await?;
    let started = wait_for_sleep_started(&mut mcp, CALL_ID).await?;
    respond_to_current_time_read(&mut mcp, &thread.id, CURRENT_TIME_AT).await?;

    // The first poll remains below the deadline, so the provider must request time again.
    respond_to_current_time_read(&mut mcp, &thread.id, CURRENT_TIME_AT + 1).await?;
    respond_to_current_time_read(&mut mcp, &thread.id, CURRENT_TIME_AT + 2).await?;

    let completed = wait_for_sleep_completed(&mut mcp, CALL_ID).await?;

    // The next inference boundary reads the same external clock after the sleep completes.
    respond_to_current_time_read(&mut mcp, &thread.id, CURRENT_TIME_AT + 2).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let expected_item = ThreadItem::Sleep(SleepItem {
        id: CALL_ID.to_string(),
        duration_ms: DURATION_MS,
    });
    assert!(completed.completed_at_ms >= started.started_at_ms);
    assert_eq!(
        started,
        ItemStartedNotification {
            item: expected_item.clone(),
            thread_id: thread.id.clone(),
            turn_id: turn.id.clone(),
            started_at_ms: started.started_at_ms,
        }
    );
    assert_eq!(
        completed,
        ItemCompletedNotification {
            item: expected_item,
            thread_id: thread.id,
            turn_id: turn.id,
            completed_at_ms: completed.completed_at_ms,
        }
    );

    Ok(())
}

async fn wait_for_sleep_started(
    mcp: &mut TestAppServer,
    call_id: &str,
) -> Result<ItemStartedNotification> {
    loop {
        let notification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("item/started"),
        )
        .await??;
        let started: ItemStartedNotification =
            serde_json::from_value(notification.params.expect("item/started params"))?;
        if matches!(&started.item, ThreadItem::Sleep(item) if item.id == call_id) {
            return Ok(started);
        }
    }
}

async fn wait_for_sleep_completed(
    mcp: &mut TestAppServer,
    call_id: &str,
) -> Result<ItemCompletedNotification> {
    loop {
        let notification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("item/completed"),
        )
        .await??;
        let completed: ItemCompletedNotification =
            serde_json::from_value(notification.params.expect("item/completed params"))?;
        if matches!(&completed.item, ThreadItem::Sleep(item) if item.id == call_id) {
            return Ok(completed);
        }
    }
}

async fn respond_to_current_time_read(
    mcp: &mut TestAppServer,
    thread_id: &str,
    current_time_at: i64,
) -> Result<()> {
    let request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CurrentTimeRead { request_id, params } = request else {
        panic!("expected CurrentTimeRead request, got: {request:?}");
    };
    assert_eq!(params.thread_id, thread_id);
    mcp.send_response(
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

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[features.current_time_reminder]
enabled = true
sleep_tool = true
clock_source = "external"
"#
        ),
    )
}
