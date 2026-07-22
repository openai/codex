use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CurrentTimeReadResponse;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
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
    MockResponsesConfig::new(&server.uri())
        .with_extra_config(
            r#"[features.current_time_reminder]
enabled = true
sleep_tool = true
clock_source = "external"
"#,
        )
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;

    let TurnStartResponse { turn, .. } = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                input: vec![V2UserInput::Text {
                    text: "Sleep briefly".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

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
        let started: ItemStartedNotification =
            timeout(DEFAULT_READ_TIMEOUT, mcp.read_notification("item/started")).await??;
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
        let completed: ItemCompletedNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_notification("item/completed"),
        )
        .await??;
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
