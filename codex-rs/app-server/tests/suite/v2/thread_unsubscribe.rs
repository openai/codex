use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallParams;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::DynamicToolFunctionSpec;
use codex_app_server_protocol::DynamicToolSpec;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
#[tokio::test]
async fn thread_unsubscribe_keeps_thread_loaded_until_idle_timeout() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_sandbox_mode("danger-full-access")
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
    let thread_id = thread.id;

    let unsubscribe: ThreadUnsubscribeResponse = mcp
        .request(|request_id| ClientRequest::ThreadUnsubscribe {
            request_id,
            params: ThreadUnsubscribeParams {
                thread_id: thread_id.clone(),
            },
        })
        .await?;
    assert_eq!(unsubscribe.status, ThreadUnsubscribeStatus::Unsubscribed);

    assert!(
        timeout(
            std::time::Duration::from_millis(250),
            mcp.read_stream_until_notification_message("thread/closed"),
        )
        .await
        .is_err()
    );

    let ThreadLoadedListResponse { data, next_cursor } = mcp
        .request(|request_id| ClientRequest::ThreadLoadedList {
            request_id,
            params: ThreadLoadedListParams::default(),
        })
        .await?;
    assert_eq!(data, vec![thread_id]);
    assert_eq!(next_cursor, None);

    Ok(())
}

#[tokio::test]
async fn thread_unsubscribe_during_turn_keeps_turn_running() -> Result<()> {
    let call_id = "deterministic-wait-call";
    let tool_name = "deterministic_wait";
    let tool_args = json!({});
    let tool_call_arguments = serde_json::to_string(&tool_args)?;

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;

    let (server, mut completions) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call(call_id, tool_name, &tool_call_arguments),
                responses::ev_completed("resp-1"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-2"),
            ]),
        }],
    ])
    .await;
    let first_response_completed = completions.remove(0);
    let final_response_completed = completions.remove(0);
    MockResponsesConfig::new(server.uri())
        .with_sandbox_mode("danger-full-access")
        .write(&codex_home)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(&codex_home)
        .build_initialized()
        .await?;

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            dynamic_tools: Some(vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
                name: tool_name.to_string(),
                description: "Deterministic wait tool".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }),
                defer_loading: false,
            })]),
            ..Default::default()
        })
        .await?;
    let thread_id = thread.id;

    let _: TurnStartResponse = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread_id.clone(),
                client_user_message_id: None,
                input: vec![V2UserInput::Text {
                    text: "run deterministic tool".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        server.wait_for_request_count(/*count*/ 1),
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, first_response_completed).await??;

    let started = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_dynamic_tool_started(&mut mcp, call_id),
    )
    .await??;
    assert_eq!(started.thread_id, thread_id);

    let request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let (request_id, params) = match request {
        ServerRequest::DynamicToolCall { request_id, params } => (request_id, params),
        other => panic!("expected DynamicToolCall request, got {other:?}"),
    };
    assert_eq!(
        params,
        DynamicToolCallParams {
            thread_id: thread_id.clone(),
            turn_id: started.turn_id,
            call_id: call_id.to_string(),
            namespace: None,
            tool: tool_name.to_string(),
            arguments: tool_args,
        }
    );

    let unsubscribe: ThreadUnsubscribeResponse = mcp
        .request(|request_id| ClientRequest::ThreadUnsubscribe {
            request_id,
            params: ThreadUnsubscribeParams {
                thread_id: thread_id.clone(),
            },
        })
        .await?;
    assert_eq!(unsubscribe.status, ThreadUnsubscribeStatus::Unsubscribed);

    let closed_while_tool_call_blocked = timeout(
        std::time::Duration::from_millis(250),
        mcp.read_stream_until_notification_message("thread/closed"),
    );
    let closed_while_tool_call_blocked = closed_while_tool_call_blocked.await;
    assert!(closed_while_tool_call_blocked.is_err());

    let response = DynamicToolCallResponse {
        content_items: vec![DynamicToolCallOutputContentItem::InputText {
            text: "dynamic-ok".to_string(),
        }],
        success: true,
    };
    mcp.send_response(request_id, serde_json::to_value(response)?)
        .await?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        server.wait_for_request_count(/*count*/ 2),
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, final_response_completed).await??;
    server.shutdown().await;

    Ok(())
}

#[tokio::test]
async fn thread_unsubscribe_preserves_cached_status_before_idle_unload() -> Result<()> {
    let server = responses::start_mock_server().await;
    let _response_mock = responses::mount_sse_once(
        &server,
        responses::sse_failed("resp-1", "server_error", "simulated failure"),
    )
    .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_sandbox_mode("danger-full-access")
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
    let thread_id = thread.id;

    let _: TurnStartResponse = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread_id.clone(),
                client_user_message_id: None,
                input: vec![V2UserInput::Text {
                    text: "fail this turn".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("error"),
    )
    .await??;

    let ThreadReadResponse { thread, .. } = mcp
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: thread_id.clone(),
                include_turns: false,
            },
        })
        .await?;
    assert_eq!(thread.status, ThreadStatus::SystemError);

    let unsubscribe: ThreadUnsubscribeResponse = mcp
        .request(|request_id| ClientRequest::ThreadUnsubscribe {
            request_id,
            params: ThreadUnsubscribeParams {
                thread_id: thread_id.clone(),
            },
        })
        .await?;
    assert_eq!(unsubscribe.status, ThreadUnsubscribeStatus::Unsubscribed);
    assert!(
        timeout(
            std::time::Duration::from_millis(250),
            mcp.read_stream_until_notification_message("thread/closed"),
        )
        .await
        .is_err()
    );

    let resume: ThreadResumeResponse = mcp
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id,
                cwd: Some(codex_home.path().to_string_lossy().to_string()),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(resume.thread.status, ThreadStatus::SystemError);

    Ok(())
}

#[tokio::test]
async fn thread_unsubscribe_reports_not_subscribed_before_idle_unload() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_sandbox_mode("danger-full-access")
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
    let thread_id = thread.id;

    let first_unsubscribe: ThreadUnsubscribeResponse = mcp
        .request(|request_id| ClientRequest::ThreadUnsubscribe {
            request_id,
            params: ThreadUnsubscribeParams {
                thread_id: thread_id.clone(),
            },
        })
        .await?;
    assert_eq!(
        first_unsubscribe.status,
        ThreadUnsubscribeStatus::Unsubscribed
    );

    let second_unsubscribe: ThreadUnsubscribeResponse = mcp
        .request(|request_id| ClientRequest::ThreadUnsubscribe {
            request_id,
            params: ThreadUnsubscribeParams { thread_id },
        })
        .await?;
    assert_eq!(
        second_unsubscribe.status,
        ThreadUnsubscribeStatus::NotSubscribed
    );

    Ok(())
}

async fn wait_for_dynamic_tool_started(
    mcp: &mut TestAppServer,
    call_id: &str,
) -> Result<ItemStartedNotification> {
    loop {
        let notification = mcp
            .read_stream_until_notification_message("item/started")
            .await?;
        let Some(params) = notification.params else {
            continue;
        };
        let started: ItemStartedNotification = serde_json::from_value(params)?;
        if matches!(&started.item, ThreadItem::DynamicToolCall { id, .. } if id == call_id) {
            return Ok(started);
        }
    }
}
