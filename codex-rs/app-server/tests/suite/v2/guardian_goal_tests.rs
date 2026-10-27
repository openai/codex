//! Goal API edits reach model history and existing workers' Guardian authorization.

use super::*;
use codex_app_server_protocol::ThreadGoalSetResponse;
use pretty_assertions::assert_eq;
use test_case::test_case;

/// A goal set after spawning a worker must reach its reviewer only with user provenance.
#[test_case("user"; "user goal")]
#[test_case("automatic"; "automatic goal is not authorization")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_guardian_sees_goal_set_after_spawn(origin: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let state = Arc::new(MockResponsesState {
        root_worker: true,
        mcp_server_name: Some(TEST_SERVER_NAME),
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let responses_url = format!("http://{}", listener.local_addr()?);
    let router = Router::new()
        .route("/v1/responses", post(parent_response))
        .with_state(Arc::clone(&state));
    let responses_server = tokio::spawn(async move { axum::serve(listener, router).await });
    let (mcp_url, mcp_server) = start_mcp_server(/*sensitive_action*/ None).await?;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_url)
        .with_provider_config("supports_websockets = false")
        .with_approval_policy("on-request")
        .with_root_config("approvals_reviewer = \"auto_review\"")
        .with_extra_config(&format!(
            "[mcp_servers.{TEST_SERVER_NAME}]\nurl = \"{mcp_url}/mcp\"\ndefault_tools_approval_mode = \"prompt\""
        ))
        .enable_feature(Feature::GuardianApproval)
        .enable_feature(Feature::Collab)
        .enable_feature(Feature::MultiAgentV2)
        .enable_feature(Feature::Goals)
        .disable_feature(Feature::GuardianV2)
        .disable_feature(Feature::EnableRequestCompression)
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let root = app.start_thread(ThreadStartParams::default()).await?.thread;
    state.allow_guardian_review.notify_one();
    let id = app
        .send_turn_start_request(TurnStartParams {
            thread_id: root.id.clone(),
            input: vec![UserInput::Text {
                text: USER_CONTEXT.to_owned(),
                text_elements: vec![],
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = app.read_response(id).await?;
    let review: ItemGuardianApprovalReviewStartedNotification = timeout(
        TIMEOUT,
        app.read_notification("item/autoApprovalReview/started"),
    )
    .await??;
    assert_ne!(review.thread_id, root.id);
    loop {
        let completed: TurnCompletedNotification =
            timeout(TIMEOUT, app.read_notification("turn/completed")).await??;
        if completed.thread_id == review.thread_id {
            assert_eq!(completed.turn.status, TurnStatus::Completed);
            break;
        }
    }
    wait_for_guardian_reviews(&state, /*expected*/ 2).await?;

    // The worker's fork and first review predate the goal. Keep the goal paused so
    // only the explicit follow-up below starts work, without an autonomous goal turn.
    let objective = "Publish the approved report using the configured MCP tool.";
    let id = app
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": root.id, "objective": objective, "status": "paused", "origin": origin,
            })),
        )
        .await?;
    let _: ThreadGoalSetResponse = app.read_response(id).await?;
    let id = app
        .send_turn_start_request(TurnStartParams {
            thread_id: root.id,
            input: vec![UserInput::Text {
                text: "Ask the worker to check again.".to_owned(),
                text_elements: vec![],
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = app.read_response(id).await?;
    wait_for_guardian_reviews(&state, /*expected*/ 3).await?;
    {
        let requests = state.guardian_requests.lock().expect("Guardian requests");
        assert!(!requests[0].to_string().contains(objective));
        let text = requests[2]["input"].to_string();
        assert_eq!(text.contains(objective), origin == "user", "{text}");
        if origin == "user" {
            let (_, root_context) = text
                .split_once(">>> ROOT CONVERSATION START")
                .expect("root authorization context");
            let (root_context, _) = root_context
                .split_once(">>> ROOT CONVERSATION END")
                .expect("root authorization context end");
            assert!(
                root_context.contains("User set the goal:"),
                "{root_context}"
            );
            assert!(root_context.contains(objective), "{root_context}");
        }
    }
    app.shutdown_gracefully().await?;
    mcp_server.abort();
    responses_server.abort();
    Ok(())
}

#[test_case(false; "loaded")]
#[test_case(true; "unloaded with path resume")]
#[tokio::test]
async fn user_goal_updates_survive_resume_and_clear(unloaded: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("done")]),
    )
    .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_provider_config("supports_websockets = false")
        .enable_feature(Feature::Goals)
        .disable_feature(Feature::EnableRequestCompression)
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let thread_id = if unloaded {
        create_fake_rollout(
            codex_home.path(),
            "2025-01-05T12-00-00",
            "2025-01-05T12:00:00Z",
            "",
            Some("mock_provider"),
            /*git_info*/ None,
        )?
    } else {
        app.start_thread(ThreadStartParams::default())
            .await?
            .thread
            .id
    };
    // Automatic lifecycle calls mutate goal state without creating user authorization.
    let id = app
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread_id, "objective": "Automatically generated goal.",
                "status": "paused", "origin": "automatic",
            })),
        )
        .await?;
    let _: ThreadGoalSetResponse = app.read_response(id).await?;
    let mut resume_request = None;
    for objective in [
        "Send the approved report.",
        "Draft the report, but do not send it.",
    ] {
        if unloaded && objective.starts_with("Draft") {
            let path = codex_rollout::find_thread_path_by_id_str(
                codex_home.path(),
                &thread_id,
                /*state_db_ctx*/ None,
            )
            .await?
            .expect("unloaded rollout");
            resume_request = Some(
                app.send_thread_resume_request(ThreadResumeParams {
                    thread_id: String::new(),
                    path: Some(path),
                    ..Default::default()
                })
                .await?,
            );
        }
        let id = app
            .send_raw_request(
                "thread/goal/set",
                Some(json!({
                    "threadId": thread_id, "objective": objective, "status": "paused", "origin": "user",
                })),
            )
            .await?;
        let _: ThreadGoalSetResponse = app.read_response(id).await?;
    }
    if let Some(id) = resume_request {
        let _: ThreadResumeResponse = app.read_response(id).await?;
    }
    let id = app
        .send_raw_request(
            "thread/goal/clear",
            Some(json!({"threadId": thread_id, "origin": "user"})),
        )
        .await?;
    let _: codex_app_server_protocol::ThreadGoalClearResponse = app.read_response(id).await?;
    let id = app
        .send_turn_start_request(TurnStartParams {
            thread_id,
            input: vec![UserInput::Text {
                text: "What is the status?".to_owned(),
                text_elements: vec![],
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = app.read_response(id).await?;
    let completed: TurnCompletedNotification =
        timeout(TIMEOUT, app.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let history = mock.single_request().message_input_texts("user").join("\n");
    assert!(
        !history.contains("Automatically generated goal."),
        "{history}"
    );
    let old = history
        .find("User set the goal: \"Send the approved report.\"")
        .expect("original user goal");
    let new = history
        .find("User set the goal: \"Draft the report, but do not send it.\"")
        .expect("updated user goal");
    assert!(old < new, "{history}");
    assert!(
        history.contains("User set goal status: \"paused\"."),
        "{history}"
    );
    assert!(history.contains("User cleared the goal."), "{history}");
    app.shutdown_gracefully().await?;
    Ok(())
}
