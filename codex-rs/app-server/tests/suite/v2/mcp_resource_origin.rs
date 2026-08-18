use super::mcp_resource::DEFAULT_READ_TIMEOUT;
use super::mcp_resource::ResourceTestEnvironment;
use super::mcp_resource::TEST_WIDGET_RESOURCE_URI;
use super::mcp_resource::start_resource_apps_mcp_server;
use super::mcp_resource::start_resource_test_app_server;
use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::McpResourceContent;
use codex_app_server_protocol::McpResourceReadParams;
use codex_app_server_protocol::McpResourceReadResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadCompactStartParams;
use codex_app_server_protocol::ThreadCompactStartResponse;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::atomic::Ordering;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn widget_reads_survive_history_modes_compaction_restarts_and_app_only_visibility()
-> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_server_url, calls, apps_server_handle) = start_resource_apps_mcp_server().await?;
    calls.tools_enabled.store(true, Ordering::Relaxed);
    let (codex_home, mut app_server) = start_resource_test_app_server(
        &apps_server_url,
        &responses_server.uri(),
        ResourceTestEnvironment::Auto,
    )
    .await?;

    let mut tool_events = vec![responses::ev_response_created("widget-tools")];
    tool_events.extend(
        [
            ("best-buy-call", "best_buy", "lamps", "link_best_buy"),
            ("walmart-call", "walmart", "lamps", "link_walmart"),
            ("failed-call", "walmart", "fail", "link_walmart"),
            ("ambiguous-account-call", "walmart", "lamps", "link_other"),
        ]
        .into_iter()
        .map(|(call_id, app, query, link_id)| {
            responses::ev_function_call_with_namespace(
                call_id,
                &format!("mcp__codex_apps__{app}"),
                "_product_search",
                &json!({ "query": query, "link_id": link_id }).to_string(),
            )
        }),
    );
    tool_events.push(responses::ev_completed("widget-tools"));
    let response = [
        responses::sse(tool_events),
        responses::sse(vec![responses::ev_completed("widget-done")]),
    ];
    let mut model_responses = std::iter::repeat_n(response, 4)
        .flatten()
        .collect::<Vec<_>>();
    model_responses.insert(
        /*index*/ 4,
        responses::sse(vec![
            responses::ev_response_created("widget-compaction"),
            responses::ev_assistant_message("widget-summary", "The apps found matching lamps."),
            responses::ev_completed("widget-compaction"),
        ]),
    );
    model_responses.insert(
        /*index*/ 5,
        responses::sse(vec![
            responses::ev_response_created("after-widget-compaction"),
            responses::ev_assistant_message("widget-follow-up", "The lamps are still available."),
            responses::ev_completed("after-widget-compaction"),
        ]),
    );
    model_responses.push(responses::sse(vec![responses::ev_completed(
        "app-only-visibility",
    )]));
    let response_mock = responses::mount_sse_sequence(&responses_server, model_responses).await;

    let mut persistent_thread_ids = Vec::new();
    for (history_mode, ephemeral) in [
        (ThreadHistoryMode::Legacy, false),
        (ThreadHistoryMode::Paginated, false),
        (ThreadHistoryMode::Legacy, true),
        (ThreadHistoryMode::Paginated, true),
    ] {
        let ThreadStartResponse { thread, .. } = app_server
            .start_thread(ThreadStartParams {
                history_mode: Some(history_mode),
                ephemeral: ephemeral.then_some(true),
                ..Default::default()
            })
            .await?;
        let turn_id = app_server
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "Find a lamp.".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        let _: TurnStartResponse =
            timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(turn_id)).await??;
        timeout(
            DEFAULT_READ_TIMEOUT,
            app_server.read_stream_until_notification_message("turn/completed"),
        )
        .await??;

        for (call_id, connector_id) in [("best-buy-call", "best_buy"), ("walmart-call", "walmart")]
        {
            let response = read_widget(&mut app_server, &thread.id, call_id).await?;
            assert_eq!(
                response,
                McpResourceReadResponse {
                    contents: vec![McpResourceContent::Text {
                        uri: TEST_WIDGET_RESOURCE_URI.to_string(),
                        mime_type: Some("text/html".to_string()),
                        text: format!("<html>{connector_id}</html>"),
                        meta: None,
                    }],
                    origin_call_id: Some(call_id.to_string()),
                }
            );
        }

        for (thread_id, call_id, uri, expected_error) in [
            (
                Some(thread.id.clone()),
                "failed-call",
                TEST_WIDGET_RESOURCE_URI,
                "was not found",
            ),
            (
                Some(thread.id.clone()),
                "walmart-call",
                "ui://widget/wrong.html",
                "does not match",
            ),
            (
                Some(thread.id.clone()),
                "ambiguous-account-call",
                TEST_WIDGET_RESOURCE_URI,
                "ambiguous account",
            ),
            (
                None,
                "walmart-call",
                TEST_WIDGET_RESOURCE_URI,
                "requires threadId",
            ),
        ] {
            let request_id = app_server
                .send_mcp_resource_read_request(McpResourceReadParams {
                    thread_id,
                    origin_call_id: Some(call_id.to_string()),
                    server: "codex_apps".to_string(),
                    uri: uri.to_string(),
                    connector_id: None,
                })
                .await?;
            let error = timeout(
                DEFAULT_READ_TIMEOUT,
                app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
            )
            .await??;
            assert!(
                error.error.message.contains(expected_error),
                "expected {expected_error:?}, got: {error:?}"
            );
        }
        if !ephemeral {
            if history_mode == ThreadHistoryMode::Paginated {
                let compact_id = app_server
                    .send_thread_compact_start_request(ThreadCompactStartParams {
                        thread_id: thread.id.clone(),
                    })
                    .await?;
                let _: ThreadCompactStartResponse =
                    timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(compact_id)).await??;
                timeout(
                    DEFAULT_READ_TIMEOUT,
                    app_server.read_stream_until_notification_message("turn/completed"),
                )
                .await??;
                let turn_id = app_server
                    .send_turn_start_request(TurnStartParams {
                        thread_id: thread.id.clone(),
                        input: vec![UserInput::Text {
                            text: "What did the apps find?".to_string(),
                            text_elements: Vec::new(),
                        }],
                        ..Default::default()
                    })
                    .await?;
                let _: TurnStartResponse =
                    timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(turn_id)).await??;
                timeout(
                    DEFAULT_READ_TIMEOUT,
                    app_server.read_stream_until_notification_message("turn/completed"),
                )
                .await??;
                for call_id in ["walmart-call", "best-buy-call"] {
                    let response = read_widget(&mut app_server, &thread.id, call_id).await?;
                    assert_eq!(response.origin_call_id.as_deref(), Some(call_id));
                }
            }
            persistent_thread_ids.push(thread.id);
        }
    }

    timeout(DEFAULT_READ_TIMEOUT, app_server.shutdown_gracefully()).await??;
    calls.best_buy_app_only.store(true, Ordering::Relaxed);
    let mut restarted = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let model_visibility_thread_id = persistent_thread_ids[0].clone();
    for thread_id in persistent_thread_ids {
        let resume_id = restarted
            .send_thread_resume_request(ThreadResumeParams {
                thread_id,
                ..Default::default()
            })
            .await?;
        let ThreadResumeResponse { thread, .. } =
            timeout(DEFAULT_READ_TIMEOUT, restarted.read_response(resume_id)).await??;
        for call_id in ["walmart-call", "best-buy-call"] {
            let response = read_widget(&mut restarted, &thread.id, call_id).await?;
            assert_eq!(response.origin_call_id.as_deref(), Some(call_id));
        }
    }

    let turn_id = restarted
        .send_turn_start_request(TurnStartParams {
            thread_id: model_visibility_thread_id,
            input: vec![UserInput::Text {
                text: "Find another lamp.".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse =
        timeout(DEFAULT_READ_TIMEOUT, restarted.read_response(turn_id)).await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        restarted.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let requests = response_mock.requests();
    let model_request = requests.last().expect("post-resume model request");
    assert!(
        model_request
            .tool_by_name("mcp__codex_apps__best_buy", "_product_search")
            .is_none()
    );
    assert!(
        model_request
            .tool_by_name("mcp__codex_apps__walmart", "_product_search")
            .is_some()
    );

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

async fn read_widget(
    app_server: &mut TestAppServer,
    thread_id: &str,
    call_id: &str,
) -> Result<McpResourceReadResponse> {
    app_server
        .request(|request_id| ClientRequest::McpResourceRead {
            request_id,
            params: McpResourceReadParams {
                thread_id: Some(thread_id.to_string()),
                origin_call_id: Some(call_id.to_string()),
                server: "codex_apps".to_string(),
                uri: TEST_WIDGET_RESOURCE_URI.to_string(),
                connector_id: None,
            },
        })
        .await
}
