//! A later LOW releases its own review without releasing an earlier pending review.

use super::*;
use codex_app_server_protocol::GuardianApprovalReviewStatus;
use codex_app_server_protocol::ItemGuardianApprovalReviewCompletedNotification;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case("snapshot"; "snapshot")]
#[test_case("conversation"; "conversation")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn later_low_does_not_release_earlier_review(mode: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let state = Arc::new(MockResponsesState {
        luna_gates: (0..2).map(|_| Notify::new()).collect(),
        gate_each_guardian_review: true,
        review_outcome: ReviewOutcome::Deny,
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let responses_url = format!("http://{}", listener.local_addr()?);
    let router = Router::new()
        .route(
            "/v1/responses",
            get(luna_websocket).post(
                |State(state): State<Arc<MockResponsesState>>, Json(request): Json<Value>| async move {
                    if request.pointer("/client_metadata/x-openai-subagent") == Some(&json!("guardian")) {
                        return parent_response(State(state), Json(request)).await.into_response();
                    }
                    if state.parent_requests.fetch_add(/*val*/ 1, Ordering::SeqCst) > 0 {
                        return (
                            [(header::CONTENT_TYPE, "text/event-stream")],
                            responses::sse(vec![
                                responses::ev_assistant_message("done", "done"),
                                responses::ev_completed("done"),
                            ]),
                        ).into_response();
                    }
                    let stream = futures::stream::iter(0..=2).then(move |index| {
                        let state = Arc::clone(&state);
                        async move {
                            // Start each action only once the previous action is awaiting
                            // synchronous review and its classifier request has arrived.
                            if index > 0 {
                                wait_for_luna_request(&state, index - 1)
                                    .await
                                    .expect("previous action's classifier request should arrive");
                                wait_for_guardian_reviews(&state, index)
                                    .await
                                    .expect("previous action should be awaiting synchronous review");
                            }
                            let mut events = Vec::new();
                            if index == 0 {
                                events.push(responses::ev_response_created("actions"));
                            }
                            if index < 2 {
                                events.push(responses::ev_function_call_with_namespace(
                                    &format!("action-{index}"),
                                    &format!("mcp__{TEST_SERVER_NAME}"),
                                    TEST_TOOL_NAME,
                                    &json!({"message": format!("action-{index}")}).to_string(),
                                ));
                            } else {
                                events.push(responses::ev_completed("actions"));
                            }
                            Ok::<_, std::convert::Infallible>(responses::sse(events))
                        }
                    });
                    (
                        [(header::CONTENT_TYPE, "text/event-stream")],
                        axum::body::Body::from_stream(stream),
                    ).into_response()
                },
            ),
        )
        .with_state(Arc::clone(&state));
    let responses_server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let (mcp_url, mcp_server) = start_mcp_server(/*sensitive_action*/ None).await?;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_url)
        .with_model(MODEL)
        .with_provider_config("supports_websockets = false")
        .with_approval_policy("on-request")
        .with_root_config("approvals_reviewer = \"auto_review\"")
        .with_extra_config(&format!(
            "[mcp_servers.{TEST_SERVER_NAME}]\nurl = \"{mcp_url}/mcp\"\ndefault_tools_approval_mode = \"prompt\"\nsupports_parallel_tool_calls = true\n\n[features.guardianv2]\nenabled = true\nmax_tool_call_lag = 2\nasync_classifier_mode = '{mode}'\n\n[features.guardianv2.review_scope]\ncomputer_use_only = false"
        ))
        .enable_feature(Feature::GuardianApproval)
        .write(codex_home.path())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let thread = app_server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread;
    let id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: USER_CONTEXT.to_owned(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(TIMEOUT, app_server.read_response(id)).await??;
    wait_for_luna_request(&state, /*index*/ 1).await?;
    wait_for_guardian_reviews(&state, /*expected*/ 2).await?;

    // Both classifiers are within the lag window and must run concurrently.
    // Only the later action gets a LOW, at thread lag zero.
    state.luna_gates[1].notify_one();
    let approved: ItemGuardianApprovalReviewCompletedNotification = timeout(
        TIMEOUT,
        app_server.read_notification("item/autoApprovalReview/completed"),
    )
    .await??;
    assert_eq!(
        (
            approved.target_item_id,
            approved.review.status,
            approved.review.risk_level
        ),
        (
            Some("action-1".to_owned()),
            GuardianApprovalReviewStatus::Approved,
            None
        )
    );
    assert!(
        timeout(
            Duration::from_millis(/*millis*/ 250),
            app_server.read_notification::<ItemGuardianApprovalReviewCompletedNotification>(
                "item/autoApprovalReview/completed"
            ),
        )
        .await
        .is_err()
    );

    // The earlier action must still wait for, and honor, its synchronous denial.
    state.allow_guardian_review.notify_waiters();
    let denied: ItemGuardianApprovalReviewCompletedNotification = timeout(
        TIMEOUT,
        app_server.read_notification("item/autoApprovalReview/completed"),
    )
    .await??;
    assert_eq!(
        (denied.target_item_id, denied.review.status),
        (
            Some("action-0".to_owned()),
            GuardianApprovalReviewStatus::Denied
        )
    );
    let completed: TurnCompletedNotification =
        timeout(TIMEOUT, app_server.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    app_server.shutdown_gracefully().await?;
    mcp_server.abort();
    responses_server.abort();
    Ok(())
}
