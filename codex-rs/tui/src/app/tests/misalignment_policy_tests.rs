use super::session_lifecycle_requests::recorded_params;
use super::session_lifecycle_requests::start_recording_app_server;
use super::*;
use codex_app_server_protocol::MisalignmentErrorDetails;
use codex_app_server_protocol::MisalignmentSteer;
use codex_protocol::openai_models::ModelAccessPrograms;
use codex_protocol::turn_input::CyberAccessProgram;
use pretty_assertions::assert_eq;

fn policy_error() -> AppServerTurnError {
    AppServerTurnError {
        message: "Chat paused".into(),
        codex_error_info: Some(AppServerCodexErrorInfo::MisalignmentPolicyViolation),
        additional_details: None,
        misalignment: Some(MisalignmentErrorDetails {
            review_target: Some("RB".to_string()),
            error_type: None,
            detailed_explanation: Some(
                "The proposed action exceeded the request.\n\n".repeat(1_500),
            ),
            steer: Some(MisalignmentSteer {
                message: "Continue **only** within the requested scope.\nDo not edit files.".into(),
            }),
        }),
    }
}

fn error_notification(
    thread_id: ThreadId,
    turn_id: &str,
    error: AppServerTurnError,
) -> ServerNotification {
    ServerNotification::Error(codex_app_server_protocol::ErrorNotification {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.into(),
        error,
        will_retry: false,
    })
}

#[tokio::test]
async fn misalignment_continuation_requires_current_review_and_submits_once() -> Result<()> {
    for (reject, preserve, daybreak, rollout_enabled) in [
        (false, false, false, true),
        (false, true, true, true),
        (true, false, false, true),
        (false, false, false, false),
        (false, true, true, false),
    ] {
        let (mut app, mut rx, _) = make_test_app_with_channels().await;
        app.chat_widget
            .set_feature_enabled(Feature::CliDaybreak, rollout_enabled);
        let (mut server, requests, proxy) = start_recording_app_server(
            &app.config,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
        )
        .await?;
        let mut session = if reject {
            test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf())
        } else {
            server.start_thread(&app.config).await?.session
        };
        let thread_id = session.thread_id;
        // Resend local restrictions restored after reconnect, rather than inheriting server settings.
        session.approval_policy = AskForApproval::OnRequest;
        session.permission_profile = PermissionProfile::read_only();
        session.active_permission_profile = None;
        app.active_thread_id = Some(thread_id);
        app.chat_widget.handle_thread_session(session);
        if !reject {
            app.chat_widget.update_account_state(
                /*status_account_display*/ None, /*plan_type*/ None,
                /*has_chatgpt_account*/ true, /*has_codex_backend_auth*/ true,
            );
            app.chat_widget.open_model_popup();
            let request_id = std::iter::from_fn(|| rx.try_recv().ok())
                .find_map(|event| match event {
                    AppEvent::FetchModels { request_id } => Some(request_id),
                    _ => None,
                })
                .expect("model catalog request");
            let mut model = crate::test_support::TEST_MODEL_PRESETS[0].clone();
            model.model = app.chat_widget.current_model().to_string();
            model.available_access_programs = Some(ModelAccessPrograms {
                cyber: vec![
                    CyberAccessProgram::Standard,
                    CyberAccessProgram::DaybreakBlue,
                ],
            });
            app.chat_widget
                .on_models_loaded(request_id, Ok(vec![model]));
            app.chat_widget
                .handle_key_event(KeyEvent::from(KeyCode::Esc));
            app.chat_widget.set_daybreak_enabled(daybreak);
        }
        app.runtime_permission_profile_override = Some(if preserve {
            RuntimePermissionProfileOverride::from_restored_config(app.chat_widget.config_ref())
        } else {
            RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref())
        });
        app.chat_widget.handle_server_notification(
            error_notification(thread_id, "failed-turn", policy_error()),
            /*replay_kind*/ None,
        );
        app.chat_widget
            .handle_key_event(KeyEvent::from(KeyCode::Enter));
        let review = std::iter::from_fn(|| rx.try_recv().ok())
            .find_map(|event| match event {
                AppEvent::ReviewMisalignment(review) => Some(review),
                _ => None,
            })
            .expect("review action");
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.handle_event(
            &mut tui,
            &mut server,
            AppEvent::ReviewMisalignment(Arc::clone(&review)),
        )
        .await?;
        let Some(Overlay::Static(overlay)) = app.overlay.as_mut() else {
            panic!("findings overlay")
        };
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(
            /*width*/ 38, /*height*/ 16,
        ))?;
        terminal.draw(|frame| overlay.render(frame.area(), frame.buffer_mut()))?;
        insta::assert_snapshot!("misalignment_findings_38", terminal.backend());
        app.overlay = None;
        assert!(recorded_params(&requests, "turn/start").is_empty());
        let mut withdrawn = policy_error();
        withdrawn
            .misalignment
            .as_mut()
            .unwrap()
            .detailed_explanation = None;
        // Reject both a newer turn and withdrawn findings followed by an empty duplicate.
        for notifications in [
            vec![turn_completed_notification(
                thread_id,
                "newer-turn",
                TurnStatus::Completed,
            )],
            vec![
                error_notification(thread_id, &review.turn_id, withdrawn.clone()),
                ServerNotification::TurnCompleted(TurnCompletedNotification {
                    thread_id: thread_id.to_string(),
                    turn: Turn {
                        error: Some(AppServerTurnError {
                            misalignment: None,
                            ..withdrawn
                        }),
                        ..test_turn(&review.turn_id, TurnStatus::Failed, Vec::new())
                    },
                }),
            ],
        ] {
            for notification in notifications {
                app.enqueue_thread_notification(thread_id, notification)
                    .await?;
            }
            app.continue_misalignment(&mut server, Arc::clone(&review))
                .await;
            assert!(recorded_params(&requests, "turn/start").is_empty());
            app.thread_event_channels.remove(&thread_id);
        }

        if reject {
            app.continue_misalignment(&mut server, Arc::clone(&review))
                .await;
            assert!(app.chat_widget.has_misalignment_policy_violation());
            let errors = std::iter::from_fn(|| rx.try_recv().ok())
                .filter_map(|event| match event {
                    AppEvent::InsertHistoryCell(cell) => {
                        Some(lines_to_single_string(&cell.display_lines(/*width*/ 80)))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            insta::assert_snapshot!(
                "misalignment_continue_failed",
                format!(
                    "{errors}\n{}",
                    render_bottom_popup(&app.chat_widget, /*width*/ 80)
                )
            );
            server.shutdown().await?;
            proxy.await??;
            continue;
        }
        // A settings error is not a new turn and must not silently disable Continue.
        app.enqueue_thread_notification(
            thread_id,
            error_notification(
                thread_id,
                "settings-update",
                AppServerTurnError {
                    codex_error_info: Some(AppServerCodexErrorInfo::BadRequest),
                    ..policy_error()
                },
            ),
        )
        .await?;
        app.continue_misalignment(&mut server, Arc::clone(&review))
            .await;
        app.continue_misalignment(&mut server, Arc::clone(&review))
            .await;
        assert!(!app.chat_widget.has_misalignment_policy_violation());
        let params = recorded_params(&requests, "turn/start");
        let [params] = params.as_slice() else {
            panic!("expected one turn/start: {params:?}")
        };
        let mut params: codex_app_server_protocol::TurnStartParams =
            serde_json::from_value(params.clone())?;
        let metadata = params.responsesapi_client_metadata.take().unwrap();
        let override_value: serde_json::Value =
            serde_json::from_str(&metadata["misalignment_override"])?;
        let timestamp = override_value["timestamp"].as_u64().unwrap();
        assert_eq!(metadata.len(), 1);
        assert_eq!(override_value, serde_json::json!({"timestamp": timestamp}));
        let config = app.chat_widget.config_ref();
        assert_eq!(
            params,
            codex_app_server_protocol::TurnStartParams {
                thread_id: thread_id.to_string(),
                cwd: Some(config.cwd.to_path_buf()),
                runtime_workspace_roots: Some(
                    config.permissions.user_visible_workspace_roots().to_vec()
                ),
                approval_policy: Some(AskForApproval::OnRequest),
                approvals_reviewer: Some(config.approvals_reviewer.into()),
                sandbox_policy: (!preserve).then(|| config.legacy_sandbox_policy().into()),
                cyber_access_program: rollout_enabled.then_some(if daybreak {
                    CyberAccessProgram::DaybreakBlue.into()
                } else {
                    CyberAccessProgram::Standard.into()
                }),
                input: vec![codex_app_server_protocol::UserInput::Text {
                    text: "Continue **only** within the requested scope.\nDo not edit files."
                        .to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            }
        );
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}
