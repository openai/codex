use anyhow::Result;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suggest_next_prompt_samples_history_without_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let first_turn_response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "first turn complete"),
        ev_completed("resp-1"),
    ]);
    let second_turn_response = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-2", "second turn complete"),
        ev_completed("resp-2"),
    ]);
    let suggestion_response = sse(vec![
        ev_response_created("resp-suggestion"),
        ev_rate_limits(),
        ev_assistant_message("msg-suggestion", "run the tests"),
        ev_completed_with_tokens("resp-suggestion", /*total_tokens*/ 33),
    ]);
    let responses = mount_sse_sequence(
        &server,
        vec![
            first_turn_response,
            second_turn_response,
            suggestion_response,
        ],
    )
    .await;

    let test = test_codex().build(&server).await?;
    test.submit_turn("first task").await?;
    test.submit_turn("second task").await?;
    let token_usage_before_suggestion = test.codex.token_usage_info().await;

    let suggestion = test
        .codex
        .suggest_next_prompt(CancellationToken::new())
        .await?;
    assert_eq!(suggestion, Some("run the tests".to_string()));

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    let suggestion_request = &requests[2];
    let suggestion_body = suggestion_request.body_json();
    assert_eq!(suggestion_body["tools"], json!([]));
    assert_eq!(suggestion_body["parallel_tool_calls"], false);

    let user_texts = suggestion_request.message_input_texts("user");
    let suggestion_prompt = user_texts
        .last()
        .expect("suggestion request should append a contextual user prompt");
    assert!(suggestion_prompt.contains("<next_prompt_suggestion>"));
    assert!(suggestion_prompt.contains("Reply with ONLY the suggestion"));
    assert!(suggestion_prompt.contains("</next_prompt_suggestion>"));

    let token_event = wait_for_event(&test.codex, |msg| {
        matches!(msg, EventMsg::TokenCount(ev)
            if ev.info.is_none() && ev.rate_limits.is_some())
    })
    .await;
    let EventMsg::TokenCount(token_count) = token_event else {
        unreachable!("wait_for_event predicate only accepts TokenCount");
    };
    assert_eq!(token_count.info, None);
    assert_eq!(
        token_count
            .rate_limits
            .expect("rate limits should be recorded")
            .primary
            .expect("primary rate limit should be present")
            .used_percent,
        42.0
    );
    assert_eq!(
        test.codex.token_usage_info().await,
        token_usage_before_suggestion
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suggest_next_prompt_skips_early_history_without_request() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let first_turn_response = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "first turn complete"),
        ev_completed("resp-1"),
    ]);
    let responses = mount_sse_sequence(&server, vec![first_turn_response]).await;

    let test = test_codex().build(&server).await?;
    test.submit_turn("first task").await?;

    let suggestion = test
        .codex
        .suggest_next_prompt(CancellationToken::new())
        .await?;
    assert_eq!(suggestion, None);
    assert_eq!(responses.requests().len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suggest_next_prompt_skips_active_realtime_conversation_without_request() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "first turn complete"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "second turn complete"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let realtime_server = start_websocket_server(vec![vec![vec![]]]).await;
    let realtime_base_url = realtime_server.uri().to_string();
    let mut builder = test_codex().with_config(move |config| {
        config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        config.experimental_realtime_ws_startup_context = Some(String::new());
    });
    let test = builder.build(&server).await?;
    test.submit_turn("first task").await?;
    test.submit_turn("second task").await?;
    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            output_modality: RealtimeOutputModality::Audio,
            prompt: Some(Some("backend prompt".to_string())),
            realtime_session_id: None,
            transport: None,
            voice: None,
        }))
        .await?;
    wait_for_event(&test.codex, |msg| {
        matches!(msg, EventMsg::RealtimeConversationStarted(_))
    })
    .await;

    let suggestion = test
        .codex
        .suggest_next_prompt(CancellationToken::new())
        .await?;
    assert_eq!(suggestion, None);
    assert_eq!(responses.requests().len(), 2);

    test.codex.submit(Op::RealtimeConversationClose).await?;
    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suggest_next_prompt_drops_stale_result_when_real_turn_starts() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (release_suggestion_tx, release_suggestion_rx) = oneshot::channel();
    let complete_response = |response_id, message_id, text| {
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_response_created(response_id),
                ev_assistant_message(message_id, text),
                ev_completed(response_id),
            ]),
        }]
    };
    let suggestion_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_response_created("resp-suggestion"),
                ev_assistant_message("msg-suggestion", "stale suggestion"),
            ]),
        },
        StreamingSseChunk {
            gate: Some(release_suggestion_rx),
            body: sse(vec![ev_completed("resp-suggestion")]),
        },
    ];
    let (server, _) = start_streaming_sse_server(vec![
        complete_response("resp-1", "msg-1", "first turn complete"),
        complete_response("resp-2", "msg-2", "second turn complete"),
        suggestion_chunks,
        complete_response("resp-3", "msg-3", "third turn complete"),
    ])
    .await;
    let mut builder = test_codex();
    let test = builder.build_with_streaming_server(&server).await?;
    test.submit_turn("first task").await?;
    test.submit_turn("second task").await?;

    let codex = Arc::clone(&test.codex);
    let suggestion_task =
        tokio::spawn(async move { codex.suggest_next_prompt(CancellationToken::new()).await });
    server.wait_for_request_count(/*count*/ 3).await;
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "third task".to_string(),
                text_elements: Vec::new(),
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    timeout(
        Duration::from_secs(2),
        server.wait_for_request_count(/*count*/ 4),
    )
    .await
    .expect("real turn request should start");

    wait_for_event(&test.codex, |msg| matches!(msg, EventMsg::TurnComplete(_))).await;
    let _ = release_suggestion_tx.send(());
    let suggestion = timeout(Duration::from_secs(2), suggestion_task)
        .await
        .expect("suggestion task should stop after hidden stream completes")
        .expect("suggestion task should not panic")?;
    assert_eq!(suggestion, None);
    assert_eq!(server.requests().await.len(), 4);

    server.shutdown().await;
    Ok(())
}

fn ev_rate_limits() -> serde_json::Value {
    json!({
        "type": "codex.rate_limits",
        "plan_type": "plus",
        "rate_limits": {
            "allowed": true,
            "limit_reached": false,
            "primary": {
                "used_percent": 42,
                "window_minutes": 60,
                "reset_at": 1700000000
            },
            "secondary": null
        },
        "code_review_rate_limits": null,
        "credits": null,
        "promo": null
    })
}
