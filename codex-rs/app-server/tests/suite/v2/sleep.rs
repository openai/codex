use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CurrentTimeReadResponse;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SleepItem;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::timeout;

use super::analytics::captured_analytics_events;
use super::analytics::mount_analytics_capture;
use super::analytics::wait_for_matching_analytics_event;

#[cfg(windows)]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const CURRENT_TIME_AT: i64 = 1_781_717_655;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clock_tools_emit_control_tool_analytics() -> Result<()> {
    let calls = [
        ("curr_time", json!({}), "completed"),
        ("sleep", json!({"duration_ms": 1}), "completed"),
        (
            "sleep",
            json!({"duration_ms": "PRIVATE_INVALID_DURATION"}),
            "failed",
        ),
        ("sleep", json!({"duration_ms": 43_200_000}), "interrupted"),
    ];
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        calls
            .iter()
            .enumerate()
            .map(|(index, (tool, arguments, _))| {
                let response_id = format!("resp-{index}");
                responses::sse(vec![
                    responses::ev_response_created(&response_id),
                    responses::ev_function_call_with_namespace(
                        &format!("call-{index}"),
                        "clock",
                        tool,
                        &arguments.to_string(),
                    ),
                    responses::ev_completed(&response_id),
                ])
            })
            .collect(),
    )
    .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_root_config(&format!("chatgpt_base_url = \"{}\"", server.uri()))
        .with_provider_config("supports_websockets = false")
        .with_extra_config("[features.current_time_reminder]\nenabled = true\nsleep_tool = true")
        .write(codex_home.path())?;
    mount_analytics_capture(&server, codex_home.path()).await?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .without_managed_config()
        .build_initialized()
        .await?;
    let thread = app_server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread;
    let TurnStartResponse { turn } = app_server
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![V2UserInput::Text {
                    text: "Exercise clock tools".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    wait_for_sleep_started(&mut app_server, "call-3").await?;
    app_server
        .interrupt_turn_and_wait_for_aborted(
            thread.id.clone(),
            turn.id.clone(),
            DEFAULT_READ_TIMEOUT,
        )
        .await?;
    let turn_event = wait_for_matching_analytics_event(&server, DEFAULT_READ_TIMEOUT, |event| {
        event["event_type"] == "codex_turn_event" && event["event_params"]["turn_id"] == turn.id
    })
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.shutdown_gracefully()).await??;

    let events = captured_analytics_events(&server).await;
    let mut control_events = events
        .iter()
        .filter(|event| event["event_type"] == "codex_control_tool_call_event")
        .collect::<Vec<_>>();
    control_events.sort_by_key(|event| event["event_params"]["item_id"].as_str());
    assert_eq!(
        control_events
            .iter()
            .map(|event| {
                let params = &event["event_params"];
                json!({
                    "tool": params["tool_name"],
                    "call": params["item_id"],
                    "thread": params["thread_id"],
                    "turn": params["turn_id"],
                    "status": params["terminal_status"],
                    "origin": params["originating_response_id"],
                    "duration": params["execution_duration_ms"].is_u64(),
                })
            })
            .collect::<Vec<_>>(),
        calls
            .iter()
            .enumerate()
            .map(|(index, (tool, _, status))| json!({
                "tool": format!("clock.{tool}"),
                "call": format!("call-{index}"),
                "thread": thread.id,
                "turn": turn.id,
                "status": status,
                "origin": format!("resp-{index}"),
                "duration": true,
            }))
            .collect::<Vec<_>>()
    );
    let control_json = serde_json::to_string(&control_events)?;
    for private_content in ["PRIVATE_INVALID_DURATION", "It is ", "Sleep completed."] {
        assert!(!control_json.contains(private_content));
    }
    assert_eq!(
        json!({
            "total": turn_event["event_params"]["total_tool_call_count"],
            "dynamic": turn_event["event_params"]["dynamic_tool_call_count"],
        }),
        json!({"total": calls.len(), "dynamic": 0})
    );
    assert_eq!(response_mock.requests().len(), calls.len());
    Ok(())
}

#[test_case(0; "initial read fails")]
#[test_case(1; "polling read fails")]
#[test_case(3; "successful reads")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_sleep_polls_current_time_and_emits_items(
    successful_sleep_reads: usize,
) -> Result<()> {
    const CALL_ID: &str = "sleep-1";
    const DURATION_MS: u64 = 2_000;
    let read_fails = successful_sleep_reads < 3;

    let server = responses::start_mock_server().await;
    let model = responses::mount_sse_sequence(
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
        .with_root_config("include_environment_context = false")
        .with_extra_config(&format!(
            r#"[features]
nonfatal_clock_read_errors = {read_fails}
[features.current_time_reminder]
enabled = true
sleep_tool = true
clock_source = "external"
"#,
        ))
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
    for current_time_at in [CURRENT_TIME_AT, CURRENT_TIME_AT + 1, CURRENT_TIME_AT + 2]
        .into_iter()
        .take(successful_sleep_reads)
    {
        respond_to_current_time_read(&mut mcp, &thread.id, current_time_at).await?;
    }
    if read_fails {
        let request = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_request_message(),
        )
        .await??;
        let ServerRequest::CurrentTimeRead { request_id, params } = request else {
            panic!("expected clock read, got {request:?}");
        };
        assert_eq!(params.thread_id, thread.id);
        mcp.send_error(
            request_id,
            JSONRPCErrorError {
                code: -32_000,
                message: "PRIVATE_CLOCK_ERROR".to_string(),
                data: None,
            },
        )
        .await?;
    }

    let completed = wait_for_sleep_completed(&mut mcp, CALL_ID).await?;

    // The next inference boundary reads the same external clock after the tool returns.
    respond_to_current_time_read(&mut mcp, &thread.id, CURRENT_TIME_AT + 2).await?;
    let turn_completed: TurnCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("turn/completed"),
    )
    .await??;
    assert_eq!(turn_completed.turn.status, TurnStatus::Completed);
    if read_fails {
        let requests = model.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[1].function_call_output_text(CALL_ID).as_deref(),
            Some("failed to read current time")
        );
    }

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
