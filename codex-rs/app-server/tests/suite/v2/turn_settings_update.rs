use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_request_user_input_sse_response;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_app_server_protocol::ThreadSettingsUpdateResponse;
use codex_app_server_protocol::ThreadSettingsUpdatedNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnSettingsUpdateParams;
use codex_app_server_protocol::TurnSettingsUpdateResponse;
use codex_app_server_protocol::TurnSettingsUpdateStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_exec_server::CreateDirectoryOptions;
use codex_features::Feature;
use codex_models_manager::bundled_models_response;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use core_test_support::responses;
use core_test_support::skip_if_wine_exec;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::timeout;

const MODEL_A: &str = "step-settings-a";
const MODEL_B: &str = "step-settings-b";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy)]
enum UpdateScenario {
    Future,
    Current,
    Rejected,
}

/// Environment updates take effect at their requested scope without enabling model switching.
#[test_case("turn/settings/update", [false, true, false]; "later steps only")]
#[test_case("thread/settings/update", [false, false, true]; "future turns only")]
#[tokio::test]
async fn environment_settings_updates_use_the_requested_scope(
    method: &str,
    expected_new_cwd: [bool; 3],
) -> Result<()> {
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            create_request_user_input_sse_response("pause")?,
            responses::sse_completed("after-update"),
            responses::sse_completed("next-turn"),
        ],
    )
    .await;
    let codex_home = TempDir::new()?;
    mock_config(codex_home.path(), &server.uri())?.write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let original_cwd = app.auto_env()?.selection().cwd.clone();
    let new_cwd = original_cwd.join("settings-environment")?;
    app.auto_env()?
        .environment()
        .get_filesystem()
        .create_directory(
            &new_cwd,
            CreateDirectoryOptions {
                recursive: false,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    let mut environment = app.auto_env_params()?;
    environment.cwd = new_cwd.clone().into();
    let thread_request = app
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let ThreadStartResponse { thread, .. } = app.read_response(thread_request).await?;
    let turn_id = start_turn(&mut app, &thread.id).await?;
    let request = timeout(DEFAULT_TIMEOUT, app.read_stream_until_request_message()).await??;
    let ServerRequest::ToolRequestUserInput { request_id, .. } = request else {
        anyhow::bail!("expected paused turn, received {request:?}");
    };

    let mut update = json!({"threadId": thread.id, "environments": [environment]});
    if method == "turn/settings/update" {
        update["turnId"] = json!(turn_id);
    }
    let mut invalid_update = update.clone();
    invalid_update["environments"][0]["environmentId"] = json!("missing");
    let invalid_id = app.send_request(method, Some(invalid_update)).await?;
    let rejected = app
        .read_stream_until_error_message(RequestId::Integer(invalid_id))
        .await?;
    assert_eq!(
        rejected.error,
        JSONRPCErrorError {
            code: -32600,
            message: "unknown turn environment id `missing`".to_string(),
            data: None,
        }
    );
    let update_id = app.send_request(method, Some(update)).await?;
    let result: Value = app.read_response(update_id).await?;
    if method == "turn/settings/update" {
        assert_eq!(result, json!({"status": "applied"}));
    } else {
        assert_eq!(result, json!({}));
        let updated: ThreadSettingsUpdatedNotification =
            app.read_notification("thread/settings/updated").await?;
        assert_eq!(
            serde_json::to_value(updated.thread_settings.environments)?,
            json!([{
                "environmentId": environment.environment_id,
                "cwd": environment.cwd,
                "runtimeWorkspaceRoots": [new_cwd.inferred_native_path_string()],
            }])
        );
    }
    app.send_response(
        request_id,
        json!({"answers": {"confirm_path": {"answers": ["Yes (Recommended)"]}}}),
    )
    .await?;
    let _: TurnCompletedNotification = app.read_notification("turn/completed").await?;
    start_turn(&mut app, &thread.id).await?;
    let _: TurnCompletedNotification = app.read_notification("turn/completed").await?;

    let original_cwd = format!("<cwd>{}</cwd>", original_cwd.inferred_native_path_string());
    let new_cwd = format!("<cwd>{}</cwd>", new_cwd.inferred_native_path_string());
    let visible_cwds = requests
        .requests()
        .iter()
        .map(|request| {
            request
                .message_input_texts("user")
                .into_iter()
                .rfind(|text| text.contains(&new_cwd) || text.contains(&original_cwd))
                .expect("model request should include a working directory")
                .contains(&new_cwd)
        })
        .collect::<Vec<_>>();
    assert_eq!(visible_cwds, expected_new_cwd);
    Ok(())
}

#[test_case(UpdateScenario::Future; "future-only A A B")]
#[test_case(UpdateScenario::Current; "turn-only A B A")]
#[test_case(UpdateScenario::Rejected; "rejected turn update changes neither owner")]
#[tokio::test]
async fn settings_updates_report_results_and_preserve_the_target_on_saved_threads(
    scenario: UpdateScenario,
) -> Result<()> {
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("response-1"),
                responses::ev_function_call(
                    "pause-turn",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue",
                            "header": "Continue",
                            "question": "Continue after the settings update?",
                            "options": [{
                                "label": "Yes (Recommended)",
                                "description": "Continue the current turn."
                            }, {
                                "label": "No",
                                "description": "Stop the current turn."
                            }]
                        }]
                    })
                    .to_string(),
                ),
                responses::ev_completed("response-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("response-2"),
                responses::ev_assistant_message("message-2", "first turn done"),
                responses::ev_completed("response-2"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("response-3"),
                responses::ev_assistant_message("message-3", "second turn done"),
                responses::ev_completed("response-3"),
            ]),
        ],
    )
    .await;
    let codex_home = TempDir::new()?;
    mock_config(codex_home.path(), &server.uri())?
        .enable_feature(Feature::StepModelSwitching)
        .write(codex_home.path())?;
    if matches!(scenario, UpdateScenario::Rejected) {
        let path = codex_home.path().join("step-models.json");
        let mut catalog: ModelsResponse = serde_json::from_slice(&std::fs::read(&path)?)?;
        for model in &mut catalog.models {
            model.node_repl_disabled = model.slug == MODEL_B;
        }
        std::fs::write(path, serde_json::to_vec(&catalog)?)?;
    }
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let request_id = app
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some(MODEL_A.to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_TIMEOUT, app.read_response(request_id)).await??;
    assert!(!thread.ephemeral);
    let turn_id = start_turn(&mut app, &thread.id).await?;

    let request = timeout(DEFAULT_TIMEOUT, app.read_stream_until_request_message()).await??;
    let ServerRequest::ToolRequestUserInput { request_id, params } = request else {
        anyhow::bail!("expected request_user_input, received {request:?}");
    };
    assert_eq!(params.thread_id, thread.id);

    let patch = TurnSettingsUpdateParams {
        thread_id: thread.id.clone(),
        turn_id: turn_id.clone(),
        model: Some(MODEL_B.to_string()),
        effort: Some(ReasoningEffort::High),
        summary: Some(ReasoningSummary::Detailed),
        service_tier: Some(Some("priority".to_string())),
        ..Default::default()
    };
    match scenario {
        UpdateScenario::Future => {
            let id = app
                .send_thread_settings_update_request(ThreadSettingsUpdateParams {
                    thread_id: thread.id.clone(),
                    model: patch.model,
                    effort: patch.effort,
                    summary: patch.summary,
                    service_tier: patch.service_tier,
                    ..Default::default()
                })
                .await?;
            let _: ThreadSettingsUpdateResponse =
                timeout(DEFAULT_TIMEOUT, app.read_response(id)).await??;
            let updated: ThreadSettingsUpdatedNotification = timeout(
                DEFAULT_TIMEOUT,
                app.read_notification("thread/settings/updated"),
            )
            .await??;
            assert_eq!(updated.thread_settings.model, MODEL_B);
        }
        UpdateScenario::Current => {
            let outcome: TurnSettingsUpdateResponse = app
                .request(|request_id| ClientRequest::TurnSettingsUpdate {
                    request_id,
                    params: patch,
                })
                .await?;
            assert_eq!(
                outcome,
                TurnSettingsUpdateResponse {
                    status: TurnSettingsUpdateStatus::Applied,
                }
            );
            // Null keeps public effort/model/summary selections, but clears tier.
            let id = app
                .send_request(
                    "turn/settings/update",
                    Some(json!({
                        "threadId": thread.id, "turnId": turn_id,
                        "model": null, "effort": null, "summary": null, "serviceTier": null,
                    })),
                )
                .await?;
            let outcome: Value = timeout(DEFAULT_TIMEOUT, app.read_response(id)).await??;
            assert_eq!(outcome, json!({ "status": "applied" }));
        }
        UpdateScenario::Rejected => {
            for field in [
                "personality",
                "permissions",
                "cwd",
                "collaborationMode",
                "scope",
            ] {
                let mut params = serde_json::to_value(&patch)?;
                params[field] = json!("unsupported");
                let id = app
                    .send_request("turn/settings/update", Some(params))
                    .await?;
                let error = timeout(
                    DEFAULT_TIMEOUT,
                    app.read_stream_until_error_message(RequestId::Integer(id)),
                )
                .await??;
                assert!(error.error.message.contains("unknown field"), "{error:?}");
                assert!(error.error.message.contains(field), "{error:?}");
            }
            let id = app
                .send_request("turn/settings/update", Some(serde_json::to_value(patch)?))
                .await?;
            let error = timeout(
                DEFAULT_TIMEOUT,
                app.read_stream_until_error_message(RequestId::Integer(id)),
            )
            .await??;
            assert_eq!(
                error.error,
                JSONRPCErrorError {
                    code: -32600,
                    message:
                        "the destination changes the admitted node REPL availability restriction"
                            .to_string(),
                    data: None,
                }
            );
        }
    }

    app.send_response(
        request_id,
        json!({"answers": {"continue": {"answers": ["Yes (Recommended)"]}}}),
    )
    .await?;
    timeout(
        DEFAULT_TIMEOUT,
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    start_turn(&mut app, &thread.id).await?;
    timeout(
        DEFAULT_TIMEOUT,
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let actual = requests
        .requests()
        .iter()
        .map(|request| {
            let body = request.body_json();
            json!({
                "model": body["model"],
                "reasoning": body["reasoning"],
                "service_tier": body.get("service_tier"),
            })
        })
        .collect::<Vec<_>>();
    let initial = json!({
        "model": MODEL_A,
        "reasoning": { "effort": "low", "summary": "concise" },
        "service_tier": null,
    });
    let changed = json!({
        "model": MODEL_B,
        "reasoning": { "effort": "high", "summary": "detailed" },
        "service_tier": "priority",
    });
    let expected = match scenario {
        UpdateScenario::Future => vec![initial.clone(), initial, changed],
        UpdateScenario::Current => {
            let mut continued = changed;
            continued["service_tier"] = Value::Null;
            vec![initial.clone(), continued, initial]
        }
        UpdateScenario::Rejected => vec![initial.clone(), initial.clone(), initial],
    };
    assert_eq!(actual, expected);
    Ok(())
}

#[tokio::test]
async fn turn_settings_update_reports_unavailable_without_starting_or_changing_a_turn() -> Result<()>
{
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("response-1"),
            responses::ev_assistant_message("message-1", "done"),
            responses::ev_completed("response-1"),
        ]),
    )
    .await;
    let codex_home = TempDir::new()?;
    mock_config(codex_home.path(), &server.uri())?
        .enable_feature(Feature::StepModelSwitching)
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let request_id = app
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some(MODEL_A.to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_TIMEOUT, app.read_response(request_id)).await??;
    assert!(!thread.ephemeral);
    let update = TurnSettingsUpdateParams {
        thread_id: thread.id.clone(),
        turn_id: "never-started".to_string(),
        environments: Some(Vec::new()),
        model: Some(MODEL_B.to_string()),
        ..Default::default()
    };
    let outcome: TurnSettingsUpdateResponse = app
        .request(|request_id| ClientRequest::TurnSettingsUpdate {
            request_id,
            params: update.clone(),
        })
        .await?;
    assert_eq!(
        outcome,
        TurnSettingsUpdateResponse {
            status: TurnSettingsUpdateStatus::TargetUnavailable,
        }
    );
    assert!(requests.requests().is_empty());

    let completed_turn_id = start_turn(&mut app, &thread.id).await?;
    timeout(
        DEFAULT_TIMEOUT,
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let outcome: TurnSettingsUpdateResponse = app
        .request(|request_id| ClientRequest::TurnSettingsUpdate {
            request_id,
            params: TurnSettingsUpdateParams {
                turn_id: completed_turn_id,
                ..update
            },
        })
        .await?;
    assert_eq!(
        outcome,
        TurnSettingsUpdateResponse {
            status: TurnSettingsUpdateStatus::TargetUnavailable,
        }
    );
    assert_eq!(
        requests.single_request().body_json()["model"],
        Value::String(MODEL_A.to_string())
    );
    let read_id = app
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id,
            include_turns: false,
        })
        .await?;
    let read: ThreadReadResponse = app.read_response(read_id).await?;
    assert_eq!(read.thread.environments, thread.environments);
    Ok(())
}

#[tokio::test]
async fn disabled_turn_settings_update_reports_rejection_without_changing_future_settings()
-> Result<()> {
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("response-1"),
            responses::ev_assistant_message("message-1", "done"),
            responses::ev_completed("response-1"),
        ]),
    )
    .await;
    let codex_home = TempDir::new()?;
    mock_config(codex_home.path(), &server.uri())?.write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let request_id = app
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some(MODEL_A.to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_TIMEOUT, app.read_response(request_id)).await??;

    let id = app
        .send_request(
            "turn/settings/update",
            Some(serde_json::to_value(TurnSettingsUpdateParams {
                thread_id: thread.id.clone(),
                turn_id: "idle".to_string(),
                model: Some(MODEL_B.to_string()),
                ..Default::default()
            })?),
        )
        .await?;
    let error = timeout(
        DEFAULT_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(id)),
    )
    .await??;
    assert_eq!(
        error.error,
        JSONRPCErrorError {
            code: -32600,
            message: "turn settings updates require the step_model_switching feature".to_string(),
            data: None,
        }
    );

    start_turn(&mut app, &thread.id).await?;
    let completed: TurnCompletedNotification =
        timeout(DEFAULT_TIMEOUT, app.read_notification("turn/completed")).await??;
    assert_eq!(completed.thread_id, thread.id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert_eq!(completed.turn.error, None);
    assert_eq!(
        requests.single_request().body_json()["model"],
        Value::String(MODEL_A.to_string())
    );
    Ok(())
}

#[test_case(ApprovalsReviewer::User, ApprovalsReviewer::AutoReview; "enable auto review")]
#[test_case(ApprovalsReviewer::AutoReview, ApprovalsReviewer::User; "disable auto review")]
#[tokio::test]
async fn live_reviewer_updates_route_approvals_without_changing_future_turns(
    initial: ApprovalsReviewer,
    updated: ApprovalsReviewer,
) -> Result<()> {
    skip_if_wine_exec!(Ok(()), "command approval requires host-native paths");

    let server = responses::start_mock_server().await;
    let mut replies = vec![responses::sse(vec![
        responses::ev_function_call(
            "pause-turn",
            "request_user_input",
            &json!({"questions": [{
                "id": "continue", "header": "Continue", "question": "Continue?",
                "options": [
                    {"label": "Yes", "description": "Continue"},
                    {"label": "No", "description": "Stop"}
                ]
            }]})
            .to_string(),
        ),
        responses::ev_completed("paused"),
    ])];
    for (index, reviewer) in [&updated, &initial].into_iter().enumerate() {
        replies.push(responses::sse(vec![
            responses::ev_function_call(
                &format!("command-{index}"),
                "exec_command",
                r#"{"cmd":"echo reviewer-test","sandbox_permissions":"require_escalated","justification":"test approval routing"}"#,
            ),
            responses::ev_completed(&format!("command-response-{index}")),
        ]));
        if *reviewer == ApprovalsReviewer::AutoReview {
            replies.push(responses::sse(vec![
                responses::ev_assistant_message("review", r#"{"outcome":"deny"}"#),
                responses::ev_completed("review-response"),
            ]));
        }
        replies.push(responses::sse_completed(&format!("done-{index}")));
    }
    let requests = responses::mount_sse_sequence(&server, replies).await;
    let codex_home = TempDir::new()?;
    // Reviewer-only updates do not require the experimental model-switching feature.
    mock_config(codex_home.path(), &server.uri())?
        .enable_feature(Feature::GuardianApproval)
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let id = app
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            approval_policy: Some(AskForApproval::OnRequest),
            approvals_reviewer: Some(initial),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_TIMEOUT, app.read_response(id)).await??;
    let turn_id = start_turn(&mut app, &thread.id).await?;
    let request = timeout(DEFAULT_TIMEOUT, app.read_stream_until_request_message()).await??;
    let ServerRequest::ToolRequestUserInput { request_id, .. } = request else {
        anyhow::bail!("expected paused turn, received {request:?}");
    };
    let result: TurnSettingsUpdateResponse = app
        .request(|request_id| ClientRequest::TurnSettingsUpdate {
            request_id,
            params: TurnSettingsUpdateParams {
                thread_id: thread.id.clone(),
                turn_id,
                approvals_reviewer: Some(updated),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(
        result,
        TurnSettingsUpdateResponse {
            status: TurnSettingsUpdateStatus::Applied,
        }
    );
    app.send_response(
        request_id,
        json!({"answers": {"continue": {"answers": ["Yes"]}}}),
    )
    .await?;

    for (index, reviewer) in [updated, initial].into_iter().enumerate() {
        if index != 0 {
            start_turn(&mut app, &thread.id).await?;
        }
        if reviewer == ApprovalsReviewer::User {
            let request =
                timeout(DEFAULT_TIMEOUT, app.read_stream_until_request_message()).await??;
            let ServerRequest::CommandExecutionRequestApproval { request_id, .. } = request else {
                anyhow::bail!("expected manual approval, received {request:?}");
            };
            app.send_response(request_id, json!({"decision": "decline"}))
                .await?;
        }
        let completed: TurnCompletedNotification =
            timeout(DEFAULT_TIMEOUT, app.read_notification("turn/completed")).await??;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
    }
    let guardian_requests = requests
        .requests()
        .into_iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .count();
    assert_eq!(guardian_requests, 1);
    Ok(())
}

async fn start_turn(app: &mut TestAppServer, thread_id: &str) -> Result<String> {
    let request_id = app
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.to_string(),
            input: vec![UserInput::Text {
                text: "continue".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let response: TurnStartResponse =
        timeout(DEFAULT_TIMEOUT, app.read_response(request_id)).await??;
    Ok(response.turn.id)
}

fn mock_config(codex_home: &Path, server_uri: &str) -> Result<MockResponsesConfig> {
    let model = bundled_models_response()?
        .models
        .into_iter()
        .find(|model| model.slug == "gpt-5.5")
        .context("bundled catalog should include gpt-5.5")?;
    let models = [MODEL_A, MODEL_B]
        .into_iter()
        .map(|slug| {
            let mut model = model.clone();
            model.slug = slug.to_string();
            model
        })
        .collect();
    let catalog_path = codex_home.join("step-models.json");
    std::fs::write(
        &catalog_path,
        serde_json::to_vec(&ModelsResponse { models })?,
    )?;
    let catalog_path = serde_json::to_string(&catalog_path)?;
    Ok(MockResponsesConfig::new(server_uri)
        .with_model(MODEL_A)
        .with_root_config(&format!(
            "model_catalog_json = {catalog_path}\nmodel_reasoning_effort = \"low\"\nmodel_reasoning_summary = \"concise\""
        ))
        .with_provider_config("supports_websockets = false")
        .enable_feature(Feature::DefaultModeRequestUserInput)
        .enable_feature(Feature::FastMode))
}
