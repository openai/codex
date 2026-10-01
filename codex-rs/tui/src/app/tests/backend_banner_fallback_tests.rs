use super::*;
use crate::chatwidget::UserMessage;
use crate::chatwidget::tests::helpers::normalize_snapshot_paths;
use crate::chatwidget::tests::helpers::set_fast_mode_test_catalog_for_models;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use pretty_assertions::assert_eq;
use serde_json::json;

pub(super) fn fallback_response() -> GetAccountRateLimitsResponse {
    serde_json::from_value(json!({
        "accountId": "workspace-a", "rateLimits": {},
        "rateLimitUpsell": {
            "banner_type": "model_recovery", "model_slug": "gpt-5.5",
            "blocked_model_slug": "gpt-5.5",
            "fallback_model_slugs": ["unavailable-model", "gpt-5.5", "gpt-5.6-terra"],
            "title": "Selected model usage exhausted",
            "description": "Add credits to use this model again.",
            "presentation": "dismissible",
            "ctas": [{"action": "view_usage", "label": "View usage"}]
        }
    }))
    .unwrap()
}

fn configure_fallback_model(app: &mut App) {
    set_chatgpt_auth(&mut app.chat_widget);
    set_fast_mode_test_catalog_for_models(&mut app.chat_widget, "gpt-5.5", "gpt-5.6-terra");
    app.model_catalog = app.chat_widget.model_catalog();
    app.chat_widget.set_model("gpt-5.5");
}

pub(super) async fn start_fallback_thread(
    app: &mut App,
) -> Result<session_lifecycle_requests::RecordingAppServer> {
    let (mut server, requests, proxy) = session_lifecycle_requests::start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let started = server.start_thread(&app.config).await?;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    configure_fallback_model(app);
    Ok((server, requests, proxy))
}

#[tokio::test]
async fn backend_banner_fallback_updates_task_settings_and_keeps_notice() -> Result<()> {
    for mode_kind in [ModeKind::Default, ModeKind::Plan] {
        let (mut app, mut events, mut ops) = make_test_app_with_channels().await;
        let (mut server, requests, proxy) = start_fallback_thread(&mut app).await?;
        let thread_id = app.active_thread_id.expect("active fallback thread");
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::Medium));
        if mode_kind == ModeKind::Plan {
            app.chat_widget
                .handle_key_event(KeyEvent::from(KeyCode::BackTab));
            app.chat_widget
                .set_plan_mode_reasoning_effort(Some(ReasoningEffortConfig::High));
        }
        app.chat_widget
            .set_feature_enabled(Feature::FastMode, /*enabled*/ true);
        app.chat_widget
            .set_service_tier(Some(ServiceTier::Fast.request_value().to_string()));
        let mut expected_mode = app.chat_widget.effective_collaboration_mode();
        assert_eq!(expected_mode.mode, mode_kind);
        expected_mode.settings.model = "gpt-5.6-terra".into();
        expected_mode.settings.reasoning_effort = Some(ReasoningEffortConfig::Medium);
        let default_model = app.config.model.clone();
        let default_plan_effort = app.config.plan_mode_reasoning_effort.clone();
        let config_path = app.config.codex_home.join("config.toml");
        let saved_config = std::fs::read(&config_path).ok();
        while events.try_recv().is_ok() {}
        while ops.try_recv().is_ok() {}
        requests.lock().unwrap().clear();
        let response = fallback_response();
        let generation = app.rate_limit_hard_stop_generation;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.handle_event(
            &mut tui,
            &mut server,
            AppEvent::RateLimitsLoaded {
                request_id: 1,
                origin: RateLimitRefreshOrigin::StatusCommand { request_id: 0 },
                hard_stop_generation: generation,
                result: Ok(response.clone()),
            },
        )
        .await?;

        let actual_mode = app.chat_widget.effective_collaboration_mode();
        assert_eq!(actual_mode, expected_mode);
        assert_eq!(app.chat_widget.current_service_tier(), None);
        let sent = requests.lock().unwrap().clone();
        let methods: Vec<_> = sent.iter().map(|request| request.method.as_str()).collect();
        assert_eq!(methods, ["thread/settings/update"]);
        assert_eq!(
            serde_json::from_value::<ThreadSettingsUpdateParams>(sent[0].params.clone().unwrap())?,
            ThreadSettingsUpdateParams {
                thread_id: thread_id.to_string(),
                model: Some("gpt-5.6-terra".into()),
                effort: Some(ReasoningEffortConfig::Medium),
                collaboration_mode: Some(expected_mode.clone()),
                service_tier: Some(Some("default".into())),
                ..Default::default()
            }
        );
        let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 72);
        assert!(rendered.contains("Selected model usage exhausted"));
        if mode_kind == ModeKind::Default {
            insta::assert_snapshot!(
                "backend_banner_after_fallback",
                normalize_snapshot_paths(rendered)
            );
        }
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        let queued = std::iter::from_fn(|| events.try_recv().ok()).collect::<Vec<_>>();
        assert!(queued.iter().any(|event| matches!(event, AppEvent::OpenUrlInBrowser { url } if url == "https://chatgpt.com/settings/usage")));
        let notices = queued
            .iter()
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => {
                    Some(lines_to_single_string(&cell.display_lines(/*width*/ 80)))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(notices.len(), 1);
        insta::assert_snapshot!("backend_banner_fallback_notice", notices[0]);
        assert!(!queued.iter().any(|event| matches!(
            event,
            AppEvent::PersistModelSelection { .. } | AppEvent::CodexOp(_)
        )));
        assert!(ops.try_recv().is_err(), "switch must not replay a turn");
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        app.chat_widget.update_backend_banner(&response);
        app.apply_backend_banner_fallback(&mut server).await;
        assert!(
            !render_bottom_popup(&app.chat_widget, /*width*/ 72)
                .contains("Selected model usage exhausted")
        );
        let mut healthy = response;
        healthy.rate_limit_upsell = None;
        app.chat_widget.update_backend_banner(&healthy);
        app.apply_backend_banner_fallback(&mut server).await;
        assert_eq!(app.chat_widget.current_model(), "gpt-5.6-terra");
        assert_eq!(requests.lock().unwrap().len(), 1);
        assert!(
            !std::iter::from_fn(|| events.try_recv().ok())
                .any(|event| matches!(event, AppEvent::InsertHistoryCell(_)))
        );
        assert_eq!(app.config.model, default_model);
        assert_eq!(app.config.plan_mode_reasoning_effort, default_plan_effort);
        assert_eq!(std::fs::read(&config_path).ok(), saved_config);
        if mode_kind == ModeKind::Plan {
            let chat = &mut app.chat_widget;
            chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
            assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Default);
            chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
        }
        app.chat_widget
            .restore_user_message_to_composer(UserMessage::from("continue"));
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_matches!(next_user_turn_op(&mut ops), AppCommand::UserTurn { model, collaboration_mode: Some(mode), .. }
            if model == "gpt-5.6-terra" && mode == expected_mode);
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn backend_banner_fallback_uses_current_task_and_accepted_generation() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let (mut server, requests, proxy) = start_fallback_thread(&mut app).await?;
    let read_generation = app.rate_limit_hard_stop_generation;
    let second = server.start_thread(&app.config).await?;
    let active_id = second.session.thread_id;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    app.store_active_thread_receiver().await;
    app.active_thread_id = None;
    app.enqueue_primary_thread_session(second.session, second.turns)
        .await?;
    configure_fallback_model(&mut app);
    requests.lock().unwrap().clear();
    app.rate_limit_hard_stop_generation = read_generation.wrapping_add(1);
    for generation in [read_generation, app.rate_limit_hard_stop_generation] {
        app.handle_event(
            &mut tui,
            &mut server,
            AppEvent::RateLimitsLoaded {
                request_id: 1,
                origin: RateLimitRefreshOrigin::StatusCommand { request_id: 0 },
                hard_stop_generation: generation,
                result: Ok(fallback_response()),
            },
        )
        .await?;
        if generation == read_generation {
            assert_eq!(app.chat_widget.current_model(), "gpt-5.5");
            assert!(requests.lock().unwrap().is_empty());
        }
    }
    let sent = requests.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].params.as_ref().unwrap()["threadId"],
        active_id.to_string()
    );
    assert_eq!(app.chat_widget.current_model(), "gpt-5.6-terra");
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn backend_banner_fallback_keeps_existing_recovery_without_candidate() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let (mut server, requests, proxy) = start_fallback_thread(&mut app).await?;
    requests.lock().unwrap().clear();
    for (replacement, expect_legacy_banner) in [
        (json!([]), true),
        (json!(["unavailable-model"]), false),
        (json!(["gpt-5.5"]), false),
    ] {
        let mut response = fallback_response();
        response.rate_limit_upsell.as_mut().unwrap()["fallback_model_slugs"] = replacement;
        app.chat_widget.update_backend_banner(&response);
        app.apply_backend_banner_fallback(&mut server).await;
        assert_eq!(app.chat_widget.current_model(), "gpt-5.5");
        let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 72);
        assert_eq!(rendered.contains("View usage"), expect_legacy_banner);
        if !expect_legacy_banner {
            insta::assert_snapshot!(
                "backend_banner_fallback_unavailable",
                normalize_snapshot_paths(rendered)
            );
        }
    }
    app.chat_widget.set_model("gpt-5.6-terra");
    app.chat_widget.update_backend_banner(&fallback_response());
    app.apply_backend_banner_fallback(&mut server).await;
    assert_eq!(app.chat_widget.current_model(), "gpt-5.6-terra");
    assert!(
        render_bottom_popup(&app.chat_widget, /*width*/ 72)
            .contains("Selected model usage exhausted")
    );
    assert!(requests.lock().unwrap().is_empty());
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn backend_banner_fallback_handles_settings_failure_and_queued_manual_selection() -> Result<()>
{
    use futures::SinkExt;
    use futures::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    for (use_reserve, error_code) in [false, true]
        .into_iter()
        .flat_map(|reserve| [Some(-32601), Some(-32603), None].map(|error| (reserve, error)))
    {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let id = ThreadId::new();
        app.active_thread_id = Some(id);
        app.chat_widget
            .handle_thread_session(test_thread_session(id, app.config.cwd.to_path_buf()));
        if use_reserve {
            app.primary_session_configured =
                Some(test_thread_session(id, app.config.cwd.to_path_buf()));
            super::luna_reserve_recovery_tests::configure_reserve_catalog(&mut app);
        } else {
            configure_fallback_model(&mut app);
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let websocket_url = format!("ws://{}", listener.local_addr()?);
        let codex_home = app.config.codex_home.display().to_string();
        let manual_selection_tx = app.app_event_tx.clone();
        let fake = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut socket = tokio_tungstenite::accept_async(stream).await?;
            let mut settings_requests = 0;
            while let Some(frame) = socket.next().await {
                let Message::Text(text) = frame? else {
                    continue;
                };
                let request: serde_json::Value = serde_json::from_str(&text)?;
                if request["id"].is_null() {
                    continue;
                }
                let response = if request["method"] == "initialize" {
                    json!({"id": request["id"], "result": {"userAgent": "test", "codexHome": codex_home}})
                } else {
                    assert_eq!(request["method"], "thread/settings/update");
                    settings_requests += 1;
                    if let Some(error_code) = error_code {
                        json!({"id": request["id"], "error": {"code": error_code, "message": "settings unavailable"}})
                    } else {
                        if settings_requests == 1 {
                            // A manual selection arriving while the RPC is pending must win afterward.
                            manual_selection_tx.send(AppEvent::UpdateModel("gpt-5.5".into()));
                        }
                        json!({"id": request["id"], "result": {}})
                    }
                };
                socket
                    .send(Message::Text(response.to_string().into()))
                    .await?;
            }
            Result::<()>::Ok(())
        });
        let client = crate::connect_remote_app_server(crate::RemoteAppServerEndpoint::WebSocket {
            websocket_url,
            auth_token: None,
        })
        .await?;
        let mut server = AppServerSession::new(
            client,
            crate::app_server_session::ThreadParamsMode::Embedded,
        );
        while events.try_recv().is_ok() {}
        let pending_turn = if use_reserve && error_code.is_some() {
            app.chat_widget
                .restore_user_message_to_composer(UserMessage::from("queued before usage reply"));
            app.chat_widget
                .handle_key_event(KeyEvent::from(KeyCode::Enter));
            Some(
                std::iter::from_fn(|| events.try_recv().ok())
                    .find(|event| matches!(event, AppEvent::CodexOp(AppCommand::UserTurn { .. })))
                    .expect("turn already queued before the failed switch"),
            )
        } else {
            None
        };
        let response = if use_reserve {
            super::luna_reserve_recovery_tests::reserve_response()
        } else {
            fallback_response()
        };
        app.chat_widget.update_backend_banner(&response);
        app.apply_backend_banner_fallback(&mut server).await;
        let queued = std::iter::from_fn(|| events.try_recv().ok()).collect::<Vec<_>>();
        let switch_notices = queued
            .iter()
            .filter(|event| match event {
                AppEvent::InsertHistoryCell(cell) => {
                    lines_to_single_string(&cell.display_lines(/*width*/ 80))
                        .contains("Automatically switched")
                }
                _ => false,
            })
            .count();
        assert_eq!(switch_notices, usize::from(error_code.is_none()));
        if error_code.is_none() {
            assert_eq!(
                app.chat_widget.current_model(),
                if use_reserve {
                    "gpt-reserve"
                } else {
                    "gpt-5.6-terra"
                }
            );
            let manual_selection = queued
                .into_iter()
                .find(|event| matches!(event, AppEvent::UpdateModel(_)))
                .unwrap();
            let mut tui = crate::tui::test_support::make_test_tui()?;
            app.handle_event(&mut tui, &mut server, manual_selection)
                .await?;
        }
        assert_eq!(app.chat_widget.current_model(), "gpt-5.5");
        assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 72).contains("View usage"));
        if use_reserve && error_code.is_some() {
            let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 90);
            assert!(rendered.contains("Add credits"));
            assert!(!rendered.contains("You’re now using Luna"));
            if error_code == Some(-32601) {
                insta::assert_snapshot!(
                    "luna_reserve_settings_unavailable",
                    normalize_snapshot_paths(rendered)
                );
            }
            app.chat_widget
                .handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
            assert!(
                std::iter::from_fn(|| events.try_recv().ok())
                    .any(|event| matches!(event, AppEvent::OpenUrlInBrowser { .. }))
            );
            let mut tui = crate::tui::test_support::make_test_tui()?;
            app.handle_event(&mut tui, &mut server, pending_turn.unwrap())
                .await?;
            assert_eq!(
                app.chat_widget.queued_user_message_texts(),
                vec!["queued before usage reply"]
            );
            app.chat_widget.finish_rate_limit_recovery();
            app.chat_widget
                .restore_user_message_to_composer(UserMessage::from("after recovery"));
            app.chat_widget
                .handle_key_event(KeyEvent::from(KeyCode::Enter));
            assert_eq!(
                app.chat_widget.queued_user_message_texts(),
                vec!["queued before usage reply", "after recovery"]
            );
            assert!(
                !std::iter::from_fn(|| events.try_recv().ok())
                    .any(|event| matches!(event, AppEvent::CodexOp(AppCommand::UserTurn { .. })))
            );
        }
        server.shutdown().await?;
        fake.await??;
    }
    Ok(())
}

#[tokio::test]
async fn backend_banner_fallback_preserves_permissions_for_first_eligible_cyber_model() -> Result<()>
{
    let mut app = make_test_app().await;
    assert!(
        app.apply_permission_profile_selection(PermissionProfileSelection {
            profile_id: codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY.into(),
            approval_policy: Some(AskForApproval::UnlessTrusted),
            approvals_reviewer: Some(ApprovalsReviewer::User),
            display_label: "Read Only".into(),
        })
        .await
    );
    let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let started = server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    configure_fallback_model(&mut app);
    let mut models = app.model_catalog.try_list_models()?;
    let mut cyber_model = models
        .iter()
        .find(|model| model.model == "gpt-5.6-terra")
        .unwrap()
        .clone();
    // Like the manual-selection tests, classify a fixture model through catalog metadata.
    cyber_model.model = "cyber-test-model".into();
    cyber_model.model_specialty = Some(MODEL_SPECIALTY_CYBER.into());
    models.push(cyber_model.clone());
    app.model_catalog = Arc::new(ModelCatalog::new(models));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    init.model_catalog = app.model_catalog.clone();
    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    app.chat_widget
        .handle_thread_session(app.primary_session_configured.clone().unwrap());
    app.chat_widget.set_model("gpt-5.5");
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::Medium));
    let mut response = fallback_response();
    response.rate_limit_upsell.as_mut().unwrap()["fallback_model_slugs"] = json!([
        "unavailable-model",
        "gpt-5.5",
        cyber_model.model,
        "gpt-5.6-terra"
    ]);
    app.chat_widget.update_backend_banner(&response);
    app.apply_backend_banner_fallback(&mut server).await;
    assert_eq!(app.chat_widget.current_model(), cyber_model.model);
    let settings = next_thread_settings_updated(&mut server, thread_id)
        .await
        .thread_settings;
    assert_eq!(
        (
            settings.model,
            settings.effort,
            settings.approval_policy,
            settings.approvals_reviewer.to_core(),
            settings.active_permission_profile.unwrap().id
        ),
        (
            cyber_model.model,
            Some(ReasoningEffortConfig::Medium),
            AskForApproval::UnlessTrusted,
            ApprovalsReviewer::User,
            codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY.to_string()
        ),
    );
    server.shutdown().await?;
    Ok(())
}
