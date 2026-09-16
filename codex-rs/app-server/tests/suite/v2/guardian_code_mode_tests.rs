//! Verifies transparent model-policy wrappers and unchanged legacy Code Mode behavior.

use super::*;
use codex_protocol::openai_models::GuardianModelPolicy;
use codex_protocol::openai_models::GuardianReviewMode;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[derive(Clone, Copy)]
enum PolicyConfig {
    Model,
    RequiredModel,
    RequiredLegacy,
    Legacy,
    LegacyDisabled,
}

#[test_case(PolicyConfig::Model, GuardianReviewMode::Adaptive, GuardianReviewMode::Synchronous, 1, 0; "model policy scores cua and preserves its cache across cells")]
#[test_case(PolicyConfig::Model, GuardianReviewMode::Synchronous, GuardianReviewMode::Adaptive, 0, 2; "adaptive shell does not score browser wrappers")]
#[test_case(PolicyConfig::Model, GuardianReviewMode::Synchronous, GuardianReviewMode::Synchronous, 0, 2; "synchronous policies do not score wrappers")]
#[test_case(PolicyConfig::RequiredModel, GuardianReviewMode::Adaptive, GuardianReviewMode::Synchronous, 1, 0; "required model preserves cua cache across cells")]
#[test_case(PolicyConfig::RequiredModel, GuardianReviewMode::Synchronous, GuardianReviewMode::Adaptive, 0, 2; "required model keeps non cua policies synchronous")]
#[test_case(PolicyConfig::Legacy, GuardianReviewMode::Adaptive, GuardianReviewMode::Synchronous, 1, 0; "legacy cua only still ignores wrappers")]
#[test_case(PolicyConfig::Legacy, GuardianReviewMode::Synchronous, GuardianReviewMode::Synchronous, 0, 0; "legacy models without cua review still do not score")]
#[test_case(PolicyConfig::Legacy, GuardianReviewMode::Adaptive, GuardianReviewMode::Adaptive, 2, 1; "legacy all tools still scores wrappers")]
#[test_case(PolicyConfig::LegacyDisabled, GuardianReviewMode::Adaptive, GuardianReviewMode::Synchronous, 0, 2; "legacy off still uses synchronous cua review")]
#[test_case(PolicyConfig::RequiredLegacy, GuardianReviewMode::Adaptive, GuardianReviewMode::Synchronous, 1, 0; "required legacy cua only still ignores wrappers")]
#[test_case(PolicyConfig::RequiredLegacy, GuardianReviewMode::Adaptive, GuardianReviewMode::Adaptive, 0, 2; "required legacy all tools still uses synchronous review")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_guardian_policy_scores_code_mode_cells(
    policy_config: PolicyConfig,
    computer_use: GuardianReviewMode,
    shell: GuardianReviewMode,
    scores_per_cell: usize,
    expected_reviews: usize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let state = Arc::new(MockResponsesState::default());
    let next_cell = Arc::new(Notify::new());
    let continue_parent = Arc::clone(&next_cell);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let responses_url = format!("http://{}", listener.local_addr()?);
    let router = Router::new()
        .route(
            "/v1/responses",
            get(luna_websocket).post(
                move |State(state): State<Arc<MockResponsesState>>, Json(request): Json<Value>| {
                    let next_cell = Arc::clone(&next_cell);
                    async move {
                        if request.pointer("/client_metadata/x-openai-subagent")
                            == Some(&json!("guardian"))
                        {
                            return parent_response(State(state), Json(request))
                                .await
                                .into_response();
                        }
                        let index = state.parent_requests.fetch_add(/*val*/ 1, Ordering::SeqCst);
                        if index == 1 {
                            next_cell.notified().await;
                        }
                        let events = if index == 1 {
                            let cell_id = request["input"]
                                .as_array()
                                .expect("parent input")
                                .iter()
                                .find(|item| item["type"] == "custom_tool_call_output" && item["call_id"] == "cell-0")
                                .and_then(|item| item["output"].as_array())
                                .into_iter()
                                .flatten()
                                .filter_map(|item| item["text"].as_str())
                                .find_map(|text| text.strip_prefix("Script running with cell ID "))
                                .and_then(|text| text.lines().next())
                                .expect("first cell yielded");
                            vec![
                                responses::ev_response_created("poll"),
                                responses::ev_function_call(
                                    "poll-cell",
                                    "wait",
                                    &json!({"cell_id": cell_id, "terminate": true}).to_string(),
                                ),
                                responses::ev_completed("poll"),
                            ]
                        } else if index < 3 {
                            let cell = usize::from(index == 2);
                            let call_id = format!("cell-{cell}");
                            let pause = if cell == 0 {
                                "yield_control(); await new Promise(() => {});"
                            } else {
                                ""
                            };
                            vec![
                                responses::ev_response_created(&call_id),
                                responses::ev_custom_tool_call(
                                    &call_id,
                                    "exec",
                                    &format!(
                                        "const message = ['browser', '{cell}'].join('-'); text(await tools.mcp__node_repl__js({{message}})); {pause}"
                                    ),
                                ),
                                responses::ev_completed(&call_id),
                            ]
                        } else {
                            vec![
                                responses::ev_assistant_message("done", "done"),
                                responses::ev_completed("done"),
                            ]
                        };
                        (
                            [(header::CONTENT_TYPE, "text/event-stream")],
                            responses::sse(events),
                        )
                            .into_response()
                    }
                },
            ),
        )
        .with_state(Arc::clone(&state));
    let responses_server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let (mcp_url, mcp_server) =
        start_mcp_server_with_tools(&["js"], /*sensitive_action*/ None).await?;
    let codex_home = TempDir::new()?;
    if matches!(
        policy_config,
        PolicyConfig::RequiredModel | PolicyConfig::RequiredLegacy
    ) {
        std::fs::write(
            codex_home.path().join("requirements.toml"),
            format!("[auto_review]\nrequired_on_models = [\"{MODEL}\"]\n"),
        )?;
    }
    let legacy_config = match policy_config {
        PolicyConfig::Model => String::new(),
        PolicyConfig::RequiredModel => "\n[features.guardianv2]\nenabled = true".to_owned(),
        PolicyConfig::Legacy | PolicyConfig::RequiredLegacy | PolicyConfig::LegacyDisabled => {
            let enabled = !matches!(policy_config, PolicyConfig::LegacyDisabled);
            let computer_use_only = shell != GuardianReviewMode::Adaptive;
            format!(
                "\n[features.guardianv2]\nenabled = {enabled}\n\n[features.guardianv2.review_scope]\ncomputer_use_only = {computer_use_only}"
            )
        }
    };
    let analytics_server = responses::start_mock_server().await;
    mount_analytics_capture(&analytics_server, codex_home.path()).await?;
    MockResponsesConfig::new(&responses_url)
        .with_model(MODEL)
        .with_provider_config("supports_websockets = false")
        .with_approval_policy("on-request")
        .with_root_config(&format!(
            "approvals_reviewer = \"auto_review\"\nchatgpt_base_url = \"{}\"",
            analytics_server.uri(),
        ))
        .with_extra_config(&format!(
            "[mcp_servers.node_repl]\nurl = \"{mcp_url}/mcp\"\ndefault_tools_approval_mode = \"auto\"\n\n[analytics]\nenabled = true{legacy_config}"
        ))
        .enable_feature(Feature::GuardianApproval)
        .enable_feature(Feature::CodeModeOnly)
        .write(codex_home.path())?;
    let config = load_default_config_for_test(&codex_home).await;
    let mut model = codex_core::test_support::construct_model_info_offline(MODEL, &config);
    model.guardian = match policy_config {
        PolicyConfig::Legacy | PolicyConfig::RequiredLegacy | PolicyConfig::LegacyDisabled => {
            model.node_repl_auto_review_required = computer_use == GuardianReviewMode::Adaptive;
            None
        }
        PolicyConfig::Model | PolicyConfig::RequiredModel => Some(GuardianModelPolicy {
            computer_use: Some(computer_use),
            shell: Some(shell),
            ..Default::default()
        }),
    };
    write_models_cache_with_models(codex_home.path(), vec![model]).await?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let thread = app_server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread;
    state.allow_guardian_review.notify_one();
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
    // Publish the first cell's scores before polling it and starting the second
    // cell. Polling must not classify or invalidate that score. Keep second-cell
    // classifiers pending so its approval must use the thread's latest score.
    if scores_per_cell > 0 {
        wait_for_luna_request(&state, scores_per_cell - 1).await?;
    }
    if scores_per_cell == 2 {
        // Legacy all-tools mode reviews the first call before a score is available.
        wait_for_guardian_reviews(&state, /*expected*/ 1).await?;
    }
    for completed_scores in 1..=scores_per_cell {
        state.allow_luna.notify_one();
        timeout(TIMEOUT, async {
            loop {
                let events = captured_analytics_events(&analytics_server).await;
                if events
                    .iter()
                    .filter(|event| {
                        event["event_type"] == "codex_guardian_v2_classification"
                            && matches!(
                                event["event_params"]["outcome"].as_str(),
                                Some("success" | "superseded")
                            )
                    })
                    .count()
                    >= completed_scores
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(/*millis*/ 25)).await;
            }
        })
        .await?;
    }
    continue_parent.notify_one();
    let completed: TurnCompletedNotification =
        timeout(TIMEOUT, app_server.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    if scores_per_cell > 0 {
        wait_for_luna_request(&state, 2 * scores_per_cell - 1).await?;
    }
    app_server.shutdown_gracefully().await?;
    let requests = state.luna_requests.lock().expect("score requests");
    assert_eq!(
        (
            requests.len(),
            state.guardian_reviews.load(Ordering::SeqCst)
        ),
        (2 * scores_per_cell, expected_reviews)
    );
    if scores_per_cell > 0 {
        assert!(requests.iter().any(|request| {
            request
                .to_string()
                .contains("const message = ['browser', '0'].join('-')")
        }));
    }
    mcp_server.abort();
    responses_server.abort();
    Ok(())
}
