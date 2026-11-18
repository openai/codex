use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn agents_navigation_opens_the_command_center_on_every_target() -> Result<()> {
    let endpoint = crate::RemoteAppServerEndpoint::UnixSocket {
        socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
    };
    for target in [
        AppServerTarget::Embedded,
        AppServerTarget::Remote {
            endpoint: endpoint.clone(),
        },
        AppServerTarget::LocalDaemon {
            endpoint,
            allow_embedded_fallback: true,
        },
    ] {
        let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let mut app_server = start_config_write_test_app_server(&app).await?;
        app.app_server_target = target;
        let init = app.chatwidget_init_for_forked_or_resumed_thread(
            &mut tui,
            app.config.clone(),
            /*initial_user_message*/ None,
        );
        app.replace_chat_widget(ChatWidget::new_with_app_event(init));
        while events.try_recv().is_ok() {}
        let composer = render_bottom_popup(&app.chat_widget, /*width*/ 96);
        let footer = composer.lines().last().expect("composer footer").trim();
        insta::allow_duplicates! {
            insta::assert_snapshot!(normalize_snapshot_paths(footer), @"← for agents · ? for shortcuts");
        }
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyCode::Left.into()),
        )
        .await?;
        let event = events.try_recv()?;
        assert_matches!(event, AppEvent::OpenAgentsOverview);
        app.handle_event(&mut tui, &mut app_server, event).await?;
        assert!(app.chat_widget.active_view_is(AGENTS_OVERVIEW_VIEW_ID));
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn externally_archived_thread_keeps_advertised_agents_navigation() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    app.app_server_target = AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        },
        allow_embedded_fallback: true,
    };
    let init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    app.chat_widget.remote_connection =
        crate::status::remote_connection::remote_connection_status_value(
            &app.app_server_target,
            /*server_version*/ None,
        );
    let id = ThreadId::new();
    app.active_thread_id = Some(id);
    app.primary_thread_id = Some(id);
    app.chat_widget
        .handle_thread_session(test_thread_session(id, app.config.cwd.to_path_buf()));
    app.ensure_thread_channel(id);
    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
            ServerNotification::ThreadArchived(ThreadArchivedNotification {
                thread_id: id.to_string(),
            }),
        )),
    )
    .await;
    while events.try_recv().is_ok() {}

    // The archived view retains its draft without accepting editing keys.
    app.chat_widget
        .restore_user_message_to_composer("ab".into());
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Char('!').into()),
    )
    .await?;
    assert_eq!(app.chat_widget.composer_text_with_pending(), "ab");
    assert!(app.chat_widget.no_modal_or_popup_active());
    for key in [
        KeyCode::End.into(),
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
    ] {
        app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(key))
            .await?;
    }
    assert_eq!(app.chat_widget.composer_text_with_pending(), "ab");
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 96).contains("command center"));
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new_with_kind(
            KeyCode::Left,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )),
    )
    .await?;
    assert!(app.chat_widget.no_modal_or_popup_active());
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Left.into()),
    )
    .await?;
    assert!(!app.chat_widget.no_modal_or_popup_active());
    assert_eq!(app.chat_widget.composer_text_with_pending(), "ab");
    insta::assert_snapshot!(
        "externally_archived_thread_agents",
        render_bottom_popup(&app.chat_widget, /*width*/ 96)
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn pending_windows_sandbox_setup_blocks_thread_replacement() -> Result<()> {
    use crate::app_event::WindowsSandboxEnableMode;

    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    app.chat_widget.windows_sandbox_local_server = true;
    while events.try_recv().is_ok() {}
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let current = ThreadId::new();
    let other = ThreadId::new();
    app.active_thread_id = Some(current);
    app.primary_thread_id = Some(current);
    let preset = codex_utils_approval_presets::builtin_approval_presets()
        .into_iter()
        .find(|preset| preset.id == "auto")
        .expect("auto preset");
    app.windows_sandbox.pending_setup = Some((WindowsSandboxEnableMode::Elevated, preset, None));
    let prompt = crate::chatwidget::create_initial_user_message(
        Some("review this".to_string()),
        Vec::new(),
        Vec::new(),
    );
    app.chat_widget.initial_user_message = prompt.clone();

    app.select_agent_thread(&mut tui, &mut app_server, other)
        .await?;
    assert_eq!(app.active_thread_id, Some(current));
    assert_eq!(app.chat_widget.initial_user_message, prompt);
    let cell = match events.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected setup navigation message, got {other:?}"),
    };
    let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 100));
    insta::assert_snapshot!(rendered, @"• Finish Windows sandbox setup before switching threads.");

    app.chat_widget.initial_user_message = None;
    app.run_agents_overview_action(
        &mut tui,
        &mut app_server,
        current,
        crate::app_event::AgentsOverviewAction::Archive,
    )
    .await?;
    assert_eq!(app.active_thread_id, Some(current));
    assert!(events.try_recv().is_err());

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn refresh_agent_picker_thread_liveness_prunes_closed_metadata_only_threads() -> Result<()> {
    let mut app = Box::pin(make_test_app()).await;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await
    .expect("embedded app server");
    let thread_id = ThreadId::new();
    app.agent_navigation.upsert(
        thread_id,
        Some("Ghost".to_string()),
        Some("worker".to_string()),
        /*is_closed*/ false,
    );

    let is_available =
        Box::pin(app.refresh_agent_picker_thread_liveness(&mut app_server, thread_id)).await;

    assert!(!is_available);
    assert_eq!(app.agent_navigation.get(&thread_id), None);
    assert!(!app.thread_event_channels.contains_key(&thread_id));
    Ok(())
}

#[test]
fn thread_links_navigate_and_missing_threads_preserve_the_draft() -> Result<()> {
    // The real-server navigation scenarios need more stack in debug builds.
    std::thread::Builder::new()
        .name("thread-link-navigation".into())
        .stack_size(12 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(Box::pin(async {
                for scenario in [
                    "root",
                    "child",
                    "cloud child",
                    "grandchild",
                    "grandchild v1",
                    "archived",
                    "archived child",
                    "archived tree",
                    "archived tree v2",
                ] {
                    let (mut app, mut events, _ops) = Box::pin(make_test_app_with_channels()).await;
                    std::fs::write(
                        app.local_settings.user_config_path.as_path(),
                        "[tui]\nresume_cwd = \"session\"\n",
                    )?;
                    app.cli_kv_overrides.push((
                        "projects".into(),
                        toml::Value::try_from(serde_json::json!({
                            test_path_buf("/").display().to_string(): {"trust_level": "trusted"},
                            app.config.cwd.display().to_string(): {"trust_level": "trusted"},
            }))?,
        ));
        let root = ThreadId::from_string(
            &app_test_support::create_fake_rollout(
                app.config.codex_home.as_path(),
                "2025-01-05T12-00-00",
                "2025-01-05T12:00:00Z",
                "Linked conversation",
                Some(app.config.model_provider_id.as_str()),
                /*git_info*/ None,
            )
            .expect("create linked rollout"),
        )?;
        let mut target = root;
        let mut intermediate = None;
        let mut rollouts = vec![app_test_support::rollout_path(
            app.config.codex_home.as_path(),
            "2025-01-05T12-00-00",
            &root.to_string(),
        )];
        let depth = match scenario {
            "child" | "cloud child" | "archived child" | "archived tree" | "archived tree v2" => 1,
            "grandchild" | "grandchild v1" => 2,
            _ => 0,
        };
        for depth in 1..=depth {
            target = ThreadId::from_string(
                &app_test_support::create_fake_parented_rollout_with_source(
                    app.config.codex_home.as_path(),
                    &format!("2025-01-05T12-00-0{depth}"),
                    "2025-01-05T12:00:01Z",
                    "Linked child",
                    Some(app.config.model_provider_id.as_str()),
                    /*git_info*/ None,
                    codex_protocol::protocol::SessionSource::SubAgent(
                        codex_protocol::protocol::SubAgentSource::ThreadSpawn {
                            parent_thread_id: target,
                            depth,
                            agent_path: None,
                            agent_nickname: Some(format!("agent-{depth}")),
                            agent_role: Some(format!("role-{depth}")),
                        },
                    ),
                    root.into(),
                    target,
                )
                .expect("create linked child rollout"),
            )?;
            if depth == 1 {
                intermediate = Some(target);
            }
            rollouts.push(app_test_support::rollout_path(
                app.config.codex_home.as_path(),
                &format!("2025-01-05T12-00-0{depth}"),
                &target.to_string(),
            ));
        }
        if scenario == "grandchild" || scenario == "archived tree v2" {
            for path in rollouts {
                let text = std::fs::read_to_string(&path)?;
                let (meta, rest) = text.split_once('\n').expect("rollout metadata");
                let mut meta: serde_json::Value = serde_json::from_str(meta)?;
                meta["payload"]["multi_agent_version"] =
                    serde_json::to_value(MultiAgentVersion::V2)?;
                std::fs::write(path, format!("{meta}\n{rest}"))?;
            }
        }
        let (mut app_server, proxy) = if scenario == "cloud child" {
            let (server, _requests, proxy) = Box::pin(
                session_lifecycle_requests::start_recording_app_server_with_history(
                    &app.config,
                    session_lifecycle_requests::HistoryCapabilities::Current,
                    /*blocked_thread_list*/ None,
                    /*failed_thread_name*/ None,
                    crate::ThreadParamsMode::Cloud,
                    LoaderOverrides::without_managed_config_for_tests(),
                ),
            ).await?;
            (server, Some(proxy))
        } else {
            (Box::pin(start_config_write_test_app_server(&app)).await?, None)
        };
        if scenario == "cloud child" {
            app.app_server_target = AppServerTarget::Cloud;
            app.backend_registry.observe(app_server.backend_id(), root);
            app.agents_overview.threads.insert(
                root,
                Some(app_server.thread_read(root, /*include_turns*/ false).await?),
            );
        }
        if scenario.starts_with("archived") {
            app_server.thread_archive(target).await?;
            if scenario.starts_with("archived tree") {
                app_server.thread_archive(root).await?;
            }
        }
        let mut tui = crate::tui::test_support::make_test_tui()?;
        while events.try_recv().is_ok() {}

        if scenario == "cloud child" {
            app.chat_widget.apply_external_edit("keep my draft".into());
            app.agents_overview.pending_archives.insert(
                root,
                tokio::sync::oneshot::channel().1,
            );
            Box::pin(thread_links::open_thread_link(&mut app, &mut tui, &mut app_server, target)).await?;
            assert_eq!(
                (
                    app.primary_thread_id,
                    app.active_thread_id,
                    app.chat_widget.composer_text_with_pending(),
                ),
                (None, None, "keep my draft".to_string()),
            );
            app.agents_overview.pending_archives.remove(&root);
            while events.try_recv().is_ok() {}
        }

        app.open_url_in_browser(format!("codex://threads/{target}"));
        let event = events.try_recv()?;
        assert_matches!(&event, AppEvent::OpenThreadLink { thread_id } if *thread_id == target);
        Box::pin(app.handle_event(&mut tui, &mut app_server, event)).await?;
        assert_eq!(
            (app.primary_thread_id, app.active_thread_id),
            (Some(root), Some(target))
        );
        if scenario.starts_with("grandchild") {
            let intermediate = intermediate.expect("grandchild has an intermediate parent");
            assert_eq!(
                app.agent_navigation.ordered_threads().iter().map(|(id, entry)| (
                    *id, entry.agent_nickname.as_deref(), entry.agent_role.as_deref()
                )).collect::<Vec<_>>(),
                vec![(root, None, None), (intermediate, Some("agent-1"), Some("role-1")), (target, Some("agent-2"), Some("role-2"))]
            );
        }
        if depth > 0 && !scenario.starts_with("archived") {
            // Follow the link again with its parent already loaded.
            Box::pin(app.select_agent_thread(&mut tui, &mut app_server, root)).await?;
            Box::pin(thread_links::open_thread_link(&mut app, &mut tui, &mut app_server, target)).await?;
            assert_eq!(
                (app.primary_thread_id, app.active_thread_id),
                (Some(root), Some(target))
            );
        }
        if scenario == "grandchild" {
            let intermediate = intermediate.expect("grandchild has an intermediate parent");
            Box::pin(app.select_agent_thread(&mut tui, &mut app_server, intermediate)).await?;
            app.ensure_thread_channel(intermediate).mark_replay_only();

            // Following a descendant reattaches the displayed replay-only ancestor.
            Box::pin(thread_links::open_thread_link(&mut app, &mut tui, &mut app_server, target)).await?;
            Box::pin(app.select_agent_thread(&mut tui, &mut app_server, intermediate)).await?;
            assert_eq!(app.active_thread_id, Some(intermediate));
            while app.active_thread_rx.as_mut().unwrap().try_recv().is_ok() {}
            let event = ThreadBufferedEvent::Notification(Box::new(
                agent_message_delta_notification(intermediate, "turn", "item", "fresh delivery"),
            ));
            app.thread_event_channels[&intermediate].sender.send(event).await?;
            let ThreadBufferedEvent::Notification(notification) =
                app.active_thread_rx.as_mut().unwrap().try_recv()?
            else {
                panic!("expected an agent message notification");
            };
            let ServerNotification::AgentMessageDelta(delta) = *notification else {
                panic!("expected an agent message delta");
            };
            assert_eq!(
                (delta.thread_id, delta.turn_id, delta.item_id, delta.delta),
                (intermediate.to_string(), "turn".into(), "item".into(), "fresh delivery".into())
            );
            Box::pin(app.select_agent_thread(&mut tui, &mut app_server, target)).await?;
        }
        if scenario.starts_with("archived") {
            assert!(app.chat_widget.is_archived_view());
            insta::allow_duplicates! {
                insta::assert_snapshot!(render_bottom_popup(&app.chat_widget, /*width*/ 100), @r"
                  This thread was archived                                                          u to unarchive
                  Press U to unarchive this thread and continue here.

                   u unarchive   ←/esc command center   ⌃c/q exit   ⌃t transcript
                ");
            }
            // Opening a link must not restore the conversation.
            assert!(
                Box::pin(app_server.resume_thread(
                    &app.local_settings,
                    app.config.clone(),
                    target,
                    crate::app_server_session::ResumeModelSettings::PreserveExistingThread
                ))
                .await
                .is_err()
            );
            app.chat_widget
                .restore_user_message_to_composer("archived draft".into());
            Box::pin(app.handle_key_event(&mut tui, &mut app_server, KeyCode::Char('u').into()))
                .await;
            assert!(!app.chat_widget.is_archived_view());
            if scenario.starts_with("archived tree") {
                assert!(
                    Box::pin(app_server.resume_thread(
                        &app.local_settings,
                        app.config.clone(),
                        root,
                        crate::app_server_session::ResumeModelSettings::PreserveExistingThread
                    ))
                    .await
                    .is_err()
                );
            }
            assert_eq!(
                app.chat_widget.composer_text_with_pending(),
                "archived draft"
            );
            assert_eq!(
                (app.primary_thread_id, app.active_thread_id),
                (Some(root), Some(target))
            );
        }
        app.chat_widget.apply_external_edit("keep my draft".into());

        for destination in [target, ThreadId::new()] {
            while events.try_recv().is_ok() {}
            app.open_url_in_browser(format!("codex://threads/{destination}"));
            let event = events.try_recv()?;
            Box::pin(app.handle_event(&mut tui, &mut app_server, event)).await?;
            assert_eq!(
                (
                    app.active_thread_id,
                    app.chat_widget.composer_text_with_pending()
                ),
                (Some(target), "keep my draft".to_string()),
            );
        }
        app_server.shutdown().await?;
        if let Some(proxy) = proxy {
            proxy.await??;
        }
    }
    Ok(())
            }))
        })?
        .join()
        .expect("thread-link navigation test panicked")
}
