use super::*;
use crate::collaboration_modes;
use codex_app_server_client::AppServerClient;
use codex_app_server_client::CloudOperation;
use codex_app_server_client::CloudUserMessageReceipt;
use codex_config::LoaderOverrides;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use crossterm::event::KeyCode;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn session_model_selection_preserves_defaults_and_updates_active_thread() -> Result<()> {
    for mode in [ModeKind::Default, ModeKind::Plan] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        app.config.model = Some("gpt-5.5".into());
        app.config.model_reasoning_effort = Some(ReasoningEffortConfig::Medium);
        app.config.plan_mode_reasoning_effort = Some(ReasoningEffortConfig::Low);
        let config_path = app.config.codex_home.join("config.toml");
        let original = "# My defaults\nmodel = 'gpt-5.5'\nmodel_reasoning_effort = 'medium'\nplan_mode_reasoning_effort = 'low'\n";
        std::fs::write(&config_path, original)?;
        let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let started = server.start_thread(&app.config).await?;
        let thread_id = started.session.thread_id;
        app.enqueue_primary_thread_session(started.session, started.turns)
            .await?;
        app.chat_widget
            .set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
        if mode == ModeKind::Plan {
            let mask = collaboration_modes::plan_mask(app.model_catalog.as_ref()).unwrap();
            app.chat_widget.set_collaboration_mask(mask);
        }
        let mut tui = crate::tui::test_support::make_test_tui()?;
        for effort in [
            ReasoningEffortConfig::High,
            ReasoningEffortConfig::Ultra,
            ReasoningEffortConfig::Max,
        ] {
            while events.try_recv().is_ok() {}
            let command = match effort {
                ReasoningEffortConfig::High => {
                    "/model gpt-5.6-terra --reasoning high --speed=standard".into()
                }
                ReasoningEffortConfig::Ultra => "/model --reasoning=ultra".into(),
                ReasoningEffortConfig::Max => "/model --reasoning max".into(),
                _ => unreachable!(),
            };
            app.chat_widget.apply_external_edit(command);
            app.chat_widget.handle_key_event(KeyCode::Esc.into());
            app.chat_widget.handle_key_event(KeyCode::Enter.into());
            let mut messages = Vec::new();
            while let Ok(event) = events.try_recv() {
                if let AppEvent::InsertHistoryCell(cell) = &event {
                    messages.push(lines_to_single_string(&cell.transcript_lines(/*width*/ 80)));
                }
                Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
            }
            let settings = next_thread_settings_updated(&mut server, thread_id).await;
            assert_eq!(
                (
                    settings.thread_settings.model.as_str(),
                    settings.thread_settings.effort.clone(),
                    settings.thread_settings.service_tier.as_deref(),
                ),
                ("gpt-5.6-terra", Some(effort.clone()), Some("default"))
            );
            if mode == ModeKind::Default && effort == ReasoningEffortConfig::High {
                for command in [
                    "/model gpt-5.6-terra fast",
                    "/model gpt-5.6-terra --speeed=fast",
                    "/model gpt-5.6-terra --reasoning=invalid",
                    "/model --speed",
                    "/model --reasoning=",
                    "/model --reasoning --speed=standard",
                    "/model --speed=fast --speed standard",
                ] {
                    app.chat_widget.apply_external_edit(command.into());
                    app.chat_widget.handle_key_event(KeyCode::Esc.into());
                    app.chat_widget.handle_key_event(KeyCode::Enter.into());
                }
                while let Ok(event) = events.try_recv() {
                    assert!(!matches!(
                        event,
                        AppEvent::SelectSessionModel { .. }
                            | AppEvent::AstraSelectedFromModelPicker { .. }
                    ));
                    if let AppEvent::InsertHistoryCell(cell) = &event {
                        messages.push(lines_to_single_string(&cell.transcript_lines(/*width*/ 80)));
                    }
                    Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
                }
                insta::assert_snapshot!("session_only_model_confirmation", messages.join("\n"));
            }
            app.enqueue_thread_notification(
                thread_id,
                ServerNotification::ThreadSettingsUpdated(settings),
            )
            .await?;
            assert_eq!(
                (
                    app.chat_widget.current_model(),
                    app.chat_widget.current_reasoning_effort()
                ),
                ("gpt-5.6-terra", Some(effort))
            );
            assert_eq!(std::fs::read(&config_path)?, original.as_bytes());
        }
        let fresh = server.start_thread(&app.config).await?;
        assert_eq!(
            (fresh.session.model.as_str(), fresh.session.reasoning_effort),
            ("gpt-5.5", Some(ReasoningEffortConfig::Medium))
        );
        assert_eq!(
            app.config.plan_mode_reasoning_effort,
            Some(ReasoningEffortConfig::Low)
        );
        server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn standard_speed_selection_falls_back_for_older_app_servers() -> Result<()> {
    let (mut app, mut events, mut ops) = make_test_app_with_channels().await;
    app.config.service_tier = Some("flex".into());
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let started = server.start_thread(&app.config).await?;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app.chat_widget.set_service_tier(Some("flex".into()));
    server.thread_settings_update_supported = false;
    while events.try_recv().is_ok() {}
    while ops.try_recv().is_ok() {}

    app.select_session_model(
        &mut server,
        "gpt-5.6-terra".into(),
        Some(ReasoningEffortConfig::High),
        Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE.into()),
    )
    .await;

    assert_eq!(
        app.chat_widget.current_service_tier(),
        Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE)
    );
    assert_eq!(app.config.service_tier.as_deref(), Some("flex"));
    app.chat_widget.apply_external_edit("continue".into());
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    assert_matches!(
        next_user_turn_op(&mut ops),
        Op::UserTurn { service_tier, .. }
            if service_tier == Some(Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE.into()))
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn cloud_model_selection_waits_for_native_admission() -> Result<()> {
    let turn = test_turn("cloud-turn", TurnStatus::InProgress, Vec::new());
    for receipt in [
        Ok(CloudUserMessageReceipt::Started(turn.clone())),
        Ok(CloudUserMessageReceipt::Steered(turn)),
        Err(codex_app_server_protocol::JSONRPCErrorError {
            code: -32602,
            message: "Cloud reply attachment failed local preflight".into(),
            data: None,
        }),
        Err(codex_app_server_protocol::JSONRPCErrorError {
            code: -32000,
            message: "Native mutation outcome unknown".into(),
            data: None,
        }),
    ] {
        let admitted = receipt.is_ok();
        let unconfirmed = receipt.as_ref().is_err_and(|error| error.code == -32000);
        let (mut app, mut events, mut ops) = make_test_app_with_channels().await;
        let thread_id = ThreadId::new();
        app.chat_widget
            .handle_thread_session(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
        app.active_thread_id = Some(thread_id);
        let original_model = app.chat_widget.current_model().to_string();
        let original_effort = app.chat_widget.current_reasoning_effort();
        let (client, mut commands, _server_events) =
            codex_app_server_client::CloudClient::channel(/*capacity*/ 1);
        let mut server = AppServerSession::new(
            crate::backend_id::BackendId::new(),
            AppServerClient::Cloud(client),
            crate::app_server_session::ThreadParamsMode::Cloud,
        );
        let mut tui = crate::tui::test_support::make_test_tui()?;
        while events.try_recv().is_ok() {}

        let event = AppEvent::ApplyModelSelection {
            model: "gpt-5.6-terra".into(),
            effort: Some(ReasoningEffortConfig::High),
        };
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
        assert!(commands.try_recv().is_err());
        assert!(app.chat_widget.cloud_picker_selection.is_none());
        let messages = std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => {
                    Some(lines_to_single_string(&cell.transcript_lines(/*width*/ 80)))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        insta::allow_duplicates! {
            insta::assert_snapshot!(
                messages.join("\n"),
                @"• Selected gpt-5.6-terra high for the next new turn in this conversation"
            );
        }
        app.chat_widget.apply_external_edit("continue".into());
        app.chat_widget.handle_key_event(KeyCode::Esc.into());
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        let op = next_user_turn_op(&mut ops);
        let admit = async {
            let command = commands.recv().await.expect("native admission request");
            let CloudOperation::SubmitUserMessage(params) = command.operation else {
                panic!("expected StartOrSteer");
            };
            assert_eq!(
                (params.model.as_deref(), params.effort),
                (Some("gpt-5.6-terra"), Some(ReasoningEffortConfig::High))
            );
            let response = receipt.map(|receipt| serde_json::to_value(receipt).unwrap());
            command.response.send(Ok(response)).unwrap();
        };
        let submit = Box::pin(super::super::user_turn_submission::try_submit_user_turn(
            &mut app,
            &mut server,
            thread_id,
            &op,
        ));
        let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(submit, admit)
        })
        .await?;
        assert!(result?);
        assert_eq!(
            (
                app.chat_widget.current_model(),
                app.chat_widget.current_reasoning_effort(),
            ),
            (original_model.as_str(), original_effort),
        );
        let input = app.chat_widget.capture_thread_input_state();
        let held = input.is_some_and(|input| input.has_unconfirmed_messages());
        assert_eq!(held, unconfirmed);
        let rejected = !admitted && !unconfirmed;
        let restored = if rejected { "continue" } else { "" };
        assert_eq!(app.chat_widget.composer_text_with_pending(), restored);
        let accepted = app.chat_widget.cloud_picker_selection.as_ref();
        assert_eq!(
            accepted.map(|selection| (selection.model.as_str(), selection.effort.clone())),
            admitted.then_some(("gpt-5.6-terra", Some(ReasoningEffortConfig::High))),
        );
        if accepted.is_some() {
            app.chat_widget.open_model_popup();
            let popup = render_bottom_popup(&app.chat_widget, /*width*/ 80);
            let label = popup.lines().find(|line| line.contains("Accepted for"));
            insta::allow_duplicates! {
                insta::assert_snapshot!(label.unwrap().trim(), @"Accepted for future turns: gpt-5.6-terra · High");
            }
            app.chat_widget.handle_thread_session(test_thread_session(
                thread_id,
                app.config.cwd.to_path_buf(),
            ));
            assert!(app.chat_widget.cloud_picker_selection.is_none());
        }
    }
    Ok(())
}

#[tokio::test]
async fn model_default_saves_report_server_outcomes_and_target_server_profile() -> Result<()> {
    for outcome in ["saved", "overridden", "rejected"] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        app.config.model_reasoning_effort = Some(ReasoningEffortConfig::Medium);
        let local_config_path = app.config.codex_home.join("config.toml");
        let local_config_before = std::fs::read(&local_config_path).ok();
        let server_home = tempfile::tempdir()?;
        let base_path = server_home.path().join("config.toml");
        let profile_path = server_home.path().join("work.config.toml");
        std::fs::write(&base_path, "# Base configuration stays unchanged.\n")?;
        std::fs::write(&profile_path, "")?;
        let mut loader_overrides = LoaderOverrides::without_managed_config_for_tests();
        loader_overrides.user_config_path =
            Some(AbsolutePathBuf::from_absolute_path(&profile_path)?);
        loader_overrides.user_config_profile = Some("work".parse()?);
        let overrides = if outcome == "overridden" {
            vec![
                ("model".into(), toml::Value::String("gpt-5.6-terra".into())),
                (
                    "model_reasoning_effort".into(),
                    toml::Value::String("low".into()),
                ),
                (
                    "plan_mode_reasoning_effort".into(),
                    toml::Value::String("low".into()),
                ),
                ("service_tier".into(), toml::Value::String("flex".into())),
            ]
        } else {
            Vec::new()
        };
        let config = ConfigBuilder::default()
            .codex_home(server_home.path().to_path_buf())
            .cli_overrides(overrides.clone())
            .loader_overrides(loader_overrides.clone())
            .harness_overrides(ConfigOverrides {
                cwd: Some(server_home.path().to_path_buf()),
                ..Default::default()
            })
            .build()
            .await?;
        let client = crate::start_embedded_app_server(
            codex_arg0::Arg0DispatchPaths::default(),
            config,
            overrides,
            loader_overrides,
            /*strict_config*/ false,
            codex_config::CloudConfigBundleLoader::default(),
            codex_feedback::CodexFeedback::new(),
            /*log_db*/ None,
            /*state_db*/ None,
            Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
            Default::default(),
        )
        .await?;
        let mut server = AppServerSession::new(
            crate::backend_id::BackendId::new(),
            AppServerClient::InProcess(client),
            crate::app_server_session::ThreadParamsMode::Remote,
        );
        if outcome == "rejected" {
            std::fs::write(&profile_path, "[broken")?;
        }
        while events.try_recv().is_ok() {}
        let mut tui = crate::tui::test_support::make_test_tui()?;
        for event in [
            AppEvent::PersistModelSelection {
                model: "gpt-5.5".into(),
                effort: Some(ReasoningEffortConfig::High),
            },
            AppEvent::PersistPlanModeReasoningEffort(Some(ReasoningEffortConfig::High)),
            AppEvent::PersistServiceTierSelection {
                service_tier: Some(ServiceTier::Fast.request_value().into()),
            },
            AppEvent::ApplyAdvancedReasoning {
                model: "gpt-5.5".into(),
                effort: ReasoningEffortConfig::Ultra,
            },
        ] {
            Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
        }
        let mut service_tier_error = None;
        let messages = std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => {
                    if outcome == "rejected" {
                        let text = lines_to_single_string(&cell.transcript_lines(/*width*/ 200));
                        if text.contains("Failed to save default service tier") {
                            service_tier_error = Some(
                                text.replace(&profile_path.display().to_string(), "<config-path>"),
                            );
                        }
                    }
                    Some(lines_to_single_string(&cell.transcript_lines(/*width*/ 80)))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            messages.matches("higher-priority").count(),
            if outcome == "overridden" { 4 } else { 0 }
        );
        assert_eq!(
            messages.matches("Failed to save").count(),
            if outcome == "rejected" { 4 } else { 0 }
        );
        if outcome == "overridden" {
            insta::assert_snapshot!("overridden_model_defaults", messages);
        }
        let persisted = std::fs::read_to_string(&profile_path)?;
        if outcome == "rejected" {
            insta::assert_snapshot!(
                "service_tier_default_save_error",
                service_tier_error.expect("expected a service-tier save error")
            );
            assert_eq!(persisted, "[broken");
            assert!(!messages.contains("Model changed"));
            assert!(!messages.contains("Service tier set"));
        } else {
            assert_eq!(
                toml::from_str::<toml::Value>(&persisted)?,
                toml::Value::Table(toml::toml! {
                    model = "gpt-5.5"
                    model_reasoning_effort = "medium"
                    plan_mode_reasoning_effort = "high"
                    service_tier = "fast"
                })
            );
        }
        assert_eq!(std::fs::read(&local_config_path).ok(), local_config_before);
        assert_eq!(
            std::fs::read_to_string(&base_path)?,
            "# Base configuration stays unchanged.\n"
        );
        server.shutdown().await?;
    }
    Ok(())
}
