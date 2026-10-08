use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::write_models_cache;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallParams;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::DynamicToolFunctionSpec;
use codex_app_server_protocol::DynamicToolSpec;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadClosedNotification;
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
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_features::Feature;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
#[tokio::test]
async fn thread_unsubscribe_keeps_thread_loaded_until_idle_timeout() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_sandbox_mode("danger-full-access")
        .with_root_config("thread_unload_delay_secs = 2")
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

    // Persist a rollout so both warm and cold resumes can find the thread.
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![V2UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;

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
    assert_eq!(data, vec![thread_id.clone()]);
    assert_eq!(next_cursor, None);

    let resume: ThreadResumeResponse = mcp
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: thread_id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(resume.thread.id, thread_id);

    // Resubscribing cancels the pending unload, even after the original deadline.
    assert!(
        timeout(
            std::time::Duration::from_millis(2200),
            mcp.read_stream_until_notification_message("thread/closed"),
        )
        .await
        .is_err()
    );
    let _: ThreadUnsubscribeResponse = mcp
        .request(|request_id| ClientRequest::ThreadUnsubscribe {
            request_id,
            params: ThreadUnsubscribeParams {
                thread_id: thread_id.clone(),
            },
        })
        .await?;
    // Losing the last subscriber starts a fresh countdown.
    assert!(
        timeout(
            std::time::Duration::from_millis(250),
            mcp.read_stream_until_notification_message("thread/closed"),
        )
        .await
        .is_err()
    );

    let closed: ThreadClosedNotification =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_notification("thread/closed")).await??;
    assert_eq!(
        closed,
        ThreadClosedNotification {
            thread_id: thread_id.clone()
        }
    );
    let status = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let status: ThreadStatusChangedNotification =
                mcp.read_notification("thread/status/changed").await?;
            if status.status == ThreadStatus::NotLoaded {
                return anyhow::Ok(status);
            }
        }
    })
    .await??;
    assert_eq!(
        status,
        ThreadStatusChangedNotification {
            thread_id: thread_id.clone(),
            status: ThreadStatus::NotLoaded,
        }
    );
    let loaded: ThreadLoadedListResponse = mcp
        .request(|request_id| ClientRequest::ThreadLoadedList {
            request_id,
            params: ThreadLoadedListParams::default(),
        })
        .await?;
    assert_eq!(
        loaded,
        ThreadLoadedListResponse {
            data: Vec::new(),
            next_cursor: None
        }
    );

    let resume: ThreadResumeResponse = mcp
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: thread_id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(resume.thread.id, thread_id);
    assert_eq!(resume.thread.status, ThreadStatus::Idle);

    Ok(())
}

#[test_case(0; "zero_delay")]
#[test_case(1; "one_second_delay")]
#[tokio::test]
async fn thread_unsubscribe_during_turn_keeps_turn_running(delay_secs: u64) -> Result<()> {
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
        .with_root_config(&format!("thread_unload_delay_secs = {delay_secs}"))
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

    // A subscribed, idle thread stays loaded even with no unload delay.
    assert!(
        timeout(
            std::time::Duration::from_millis(250),
            mcp.read_stream_until_notification_message("thread/closed"),
        )
        .await
        .is_err()
    );

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
        std::time::Duration::from_millis(1200),
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
    if delay_secs > 0 {
        // Once the turn finishes, inactivity starts a fresh countdown.
        assert!(
            timeout(
                std::time::Duration::from_millis(250),
                mcp.read_stream_until_notification_message("thread/closed"),
            )
            .await
            .is_err()
        );
    }
    let closed: ThreadClosedNotification =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_notification("thread/closed")).await??;
    assert_eq!(closed, ThreadClosedNotification { thread_id });
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

#[tokio::test]
async fn v2_child_idle_eviction_preserves_mail_and_followup_context() -> Result<()> {
    let server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_model("gpt-5.5")
        .enable_feature(Feature::MultiAgentV2)
        .with_root_config("thread_unload_delay_secs = 0")
        .write(codex_home.path())?;
    write_models_cache(codex_home.path()).await?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let parent = mcp.start_thread(ThreadStartParams::default()).await?.thread;
    let mut child_id = None;

    for (tool, args) in [
        (
            "spawn_agent",
            json!({
                "task_name": "worker", "fork_turns": "none", "message": "Initial child task"
            }),
        ),
        (
            "send_message",
            json!({"target": "worker", "message": "Unread note for later"}),
        ),
        (
            "followup_task",
            json!({"target": "worker", "message": "Continue the child task"}),
        ),
    ] {
        let call_id = format!("idle-eviction-{tool}");
        let _call = responses::mount_sse_once_match(
            &server,
            wiremock::matchers::header("thread-id", parent.id.clone()),
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    &call_id,
                    "collaboration",
                    tool,
                    &args.to_string(),
                ),
                responses::ev_completed("parent-call"),
            ]),
        )
        .await;
        let _parent_done = responses::mount_sse_once_match(
            &server,
            move |request: &wiremock::Request| {
                String::from_utf8_lossy(&request.body).contains(&call_id)
            },
            responses::sse(vec![responses::ev_completed("parent-done")]),
        )
        .await;
        if tool != "send_message" {
            let parent_id = parent.id.clone();
            let _child = responses::mount_sse_once_match(
                &server,
                move |request: &wiremock::Request| {
                    request
                        .headers
                        .get("thread-id")
                        .is_some_and(|id| id != parent_id.as_str())
                },
                responses::sse(vec![
                    responses::ev_assistant_message(
                        "child-result",
                        "Remember the initial child result",
                    ),
                    responses::ev_completed("child-done"),
                ]),
            )
            .await;
        }
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: parent.id.clone(),
                input: vec![V2UserInput::Text {
                    text: format!("Run {tool}"),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            }),
        )
        .await??;

        if tool != "send_message" {
            let completed = timeout(
                DEFAULT_READ_TIMEOUT,
                mcp.read_stream_until_matching_notification(
                    "child turn completed",
                    |notification| {
                        notification.method == "turn/completed"
                            && notification
                                .params
                                .as_ref()
                                .is_some_and(|params| params["threadId"] != parent.id)
                    },
                ),
            )
            .await??;
            let params = completed.params.expect("child completion params");
            let completed_child = params["threadId"].as_str().expect("child ID").to_string();
            if let Some(child_id) = &child_id {
                assert_eq!(&completed_child, child_id);
            } else {
                child_id = Some(completed_child);
            }
        } else {
            // Mail is queued while subscribed. Unsubscribe lets the actual child listener
            // evict it; the parent stays loaded so the follow-up uses the same agent tree.
            let child_id = child_id.clone().expect("spawned child");
            let unsubscribed: ThreadUnsubscribeResponse = mcp
                .request(|request_id| ClientRequest::ThreadUnsubscribe {
                    request_id,
                    params: ThreadUnsubscribeParams {
                        thread_id: child_id.clone(),
                    },
                })
                .await?;
            assert_eq!(unsubscribed.status, ThreadUnsubscribeStatus::Unsubscribed);
            let closed: ThreadClosedNotification =
                timeout(DEFAULT_READ_TIMEOUT, mcp.read_notification("thread/closed")).await??;
            assert_eq!(
                closed,
                ThreadClosedNotification {
                    thread_id: child_id.clone()
                }
            );
            let loaded: ThreadLoadedListResponse = mcp
                .request(|request_id| ClientRequest::ThreadLoadedList {
                    request_id,
                    params: ThreadLoadedListParams::default(),
                })
                .await?;
            assert_eq!(loaded.data, vec![parent.id.clone()]);
            assert_eq!(
                responses::received_responses_requests(&server)
                    .await
                    .iter()
                    .filter(
                        |request| request.header("thread-id").as_deref() == Some(child_id.as_str())
                    )
                    .count(),
                1,
                "queue-only mail must not start another child turn",
            );
        }
    }
    let child_id = child_id.expect("spawned child");
    let requests = responses::received_responses_requests(&server).await;
    let child_requests: Vec<_> = requests
        .iter()
        .filter(|request| request.header("thread-id").as_deref() == Some(child_id.as_str()))
        .collect();
    assert_eq!(child_requests.len(), 2);
    assert_eq!(
        child_requests[1]
            .inputs_of_type("message")
            .into_iter()
            .filter(|item| item["role"] == "assistant")
            .collect::<Vec<_>>(),
        vec![json!({
            "type": "message", "role": "assistant",
            "content": [{"type": "output_text", "text": "Remember the initial child result"}],
        })],
    );
    for message in ["Unread note for later", "Continue the child task"] {
        assert_eq!(
            child_requests[1]
                .inputs_of_type("agent_message")
                .iter()
                .filter(|item| item.to_string().contains(message))
                .count(),
            1,
            "resumed child must receive {message} exactly once",
        );
    }
    mcp.shutdown_gracefully().await?;
    Ok(())
}
