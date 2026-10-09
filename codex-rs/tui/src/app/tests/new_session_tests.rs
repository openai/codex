//! Replacement starts use server defaults and preserve the current task on failure.

use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn new_sessions_preserve_yolo_launch_and_later_permission_choices() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    app.harness_overrides.approval_policy = Some(AskForApproval::Never.to_core());
    app.harness_overrides.sandbox_mode =
        Some(codex_protocol::config_types::SandboxMode::DangerFullAccess);
    let (mut server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut loaded_config = app.config.clone();
    loaded_config
        .permissions
        .approval_policy
        .set(AskForApproval::OnRequest.to_core())?;
    loaded_config
        .permissions
        .set_permission_profile(PermissionProfile::read_only())?;
    let loaded = server.start_thread(&loaded_config).await?;
    app.select_agents_overview_thread(&mut tui, &mut server, loaded.session.thread_id)
        .await?;
    for picker_change in [false, true] {
        if picker_change {
            app.chat_widget
                .set_approval_policy(AskForApproval::OnRequest);
            app.chat_widget
                .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
                    PermissionProfile::read_only(),
                    ActivePermissionProfile::new(":read-only"),
                ))?;
            app.adopt_server_permissions();
        }
        app.start_fresh_session(
            &mut tui,
            &mut server,
            /*session_start_source*/ None,
            /*initial_user_message*/ None,
            /*new_thread_name*/ None,
        )
        .await;
        let cached = app.primary_session_configured.as_ref().unwrap().clone();
        let mut resumed = cached.clone();
        resumed.approval_policy = AskForApproval::OnRequest;
        resumed.permission_profile = PermissionProfile::read_only();
        resumed.active_permission_profile = None;
        app.restore_runtime_permissions(&mut resumed, &cached);
        assert_eq!(resumed, cached);
    }
    let params = recorded_params(&requests, "thread/start");
    assert_eq!(
        params
            .iter()
            .skip(1)
            .map(|params| serde_json::json!({
                "approvalPolicy": params["approvalPolicy"],
                "sandbox": params["sandbox"],
                "permissions": params["permissions"],
            }))
            .collect::<Vec<_>>(),
        vec![
            serde_json::json!({"approvalPolicy": "never", "sandbox": "danger-full-access", "permissions": null}),
            serde_json::json!({"approvalPolicy": "on-request", "sandbox": null, "permissions": ":read-only"}),
        ]
    );
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn new_session_preserves_vim_line_yank() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let home = tempdir()?;
    app.config.codex_home = home.path().to_path_buf().abs();
    app.config.sqlite = SqliteConfig::new_for_testing(home.path().abs());
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("saved line");
    for code in [KeyCode::Esc, KeyCode::Char('d'), KeyCode::Char('d')] {
        app.chat_widget
            .handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
    }
    assert_eq!(app.chat_widget.composer_text_with_pending(), "");

    let (mut server, _requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::Current,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.start_fresh_session(
        &mut tui,
        &mut server,
        /*session_start_source*/ None,
        /*initial_user_message*/ None,
        /*new_thread_name*/ None,
    )
    .await;

    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("new line");
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "new line\nsaved line"
    );
    let composer_lines = render_bottom_popup(&app.chat_widget, /*width*/ 80)
        .lines()
        .take(2)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(composer_lines);
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn replacement_uses_server_defaults_and_preserves_explicit_launch_settings() -> Result<()> {
    for (managed_effort, explicit, expected_model, expected_effort) in [
        (None, "saved", "server-model", "high"),
        (Some("medium"), "model", "cli-model", "high"),
        (Some("medium"), "effort", "server-model", "low"),
        (None, "profile_model", "profile-model", "high"),
        (None, "profile_effort", "server-model", "low"),
        (None, "profile_summary", "server-model", "high"),
        (None, "profile_summary_off", "server-model", "high"),
        (None, "profile_summary_cli", "server-model", "high"),
        (Some("medium"), "profile_model", "profile-model", "high"),
        (Some("medium"), "profile_effort", "server-model", "low"),
        (Some(""), "profile", "profile-model", "low"),
        (Some("medium"), "profile", "profile-model", "low"),
        (
            Some("medium"),
            "profile_unrelated",
            "managed-model",
            "medium",
        ),
    ] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let server_home = tempdir()?;
        let client_home = tempdir()?;
        std::fs::write(
            server_home.path().join("work.config.toml"),
            "model = \"server-model\"\nmodel_reasoning_effort = \"high\"\n",
        )?;
        std::fs::write(
            client_home.path().join("config.toml"),
            "model = \"stale-client-model\"\nmodel_reasoning_effort = \"low\"\nmodel_reasoning_summary = \"detailed\"\nfeatures.concurrent_reasoning_summaries = true\n",
        )?;
        if let Some(effort) = managed_effort {
            let effort = if effort.is_empty() {
                String::new()
            } else {
                format!("model_reasoning_effort = {effort:?}\n")
            };
            std::fs::write(
                server_home.path().join("requirements.toml"),
                format!("[models.new_thread]\nmodel = \"managed-model\"\n{effort}"),
            )?;
        }
        let mut server_config = app.config.clone();
        server_config.codex_home = server_home.path().to_path_buf().abs();
        server_config.sqlite = SqliteConfig::new_for_testing(server_home.path().abs());
        let (mut server, requests, proxy) = start_recording_app_server_with_history(
            &server_config,
            HistoryCapabilities::Current,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            if explicit.starts_with("profile_summary") {
                crate::app_server_session::ThreadParamsMode::Embedded
            } else {
                crate::app_server_session::ThreadParamsMode::Remote
            },
            LoaderOverrides {
                user_config_path: Some(server_home.path().join("work.config.toml").abs()),
                user_config_profile: Some("work".parse()?),
                system_requirements_path: Some(server_home.path().join("requirements.toml")),
                ..LoaderOverrides::default()
            },
        )
        .await?;
        server = server.with_remote_cwd_override(Some(server_config.cwd.to_path_buf()));
        server.bootstrap(&server_config).await?;
        app.config.codex_home = client_home.path().to_path_buf().abs();
        app.config.sqlite = SqliteConfig::new_for_testing(client_home.path().abs());
        app.chat_widget
            .handle_thread_session_quiet(test_thread_session(
                ThreadId::new(),
                client_home.path().to_path_buf(),
            ));
        match explicit {
            "model" => app.harness_overrides.model = Some("cli-model".to_string()),
            "effort" => app.cli_kv_overrides.push((
                "model_reasoning_effort".to_string(),
                TomlValue::String("low".to_string()),
            )),
            profile @ ("profile"
            | "profile_model"
            | "profile_effort"
            | "profile_unrelated"
            | "profile_summary"
            | "profile_summary_off"
            | "profile_summary_cli") => {
                let path = client_home.path().join("work.config.toml");
                std::fs::write(
                    &path,
                    match profile {
                        "profile_model" => "model = \"profile-model\"\n",
                        "profile_effort" => "model_reasoning_effort = \"low\"\n",
                        "profile_unrelated" => "model_verbosity = \"low\"\n",
                        "profile_summary" => {
                            "model_reasoning_summary = \"detailed\"\nfeatures.concurrent_reasoning_summaries = true\n"
                        }
                        "profile_summary_off" | "profile_summary_cli" => {
                            "model_reasoning_summary = \"detailed\"\nfeatures.concurrent_reasoning_summaries = false\n"
                        }
                        _ => "model = \"profile-model\"\nmodel_reasoning_effort = \"low\"\n",
                    },
                )?;
                app.loader_overrides.user_config_path = Some(path.abs());
                app.loader_overrides.user_config_profile = Some("work".parse()?);
            }
            _ => {}
        }
        if explicit == "profile_summary_cli" {
            app.cli_kv_overrides.push((
                "features.concurrent_reasoning_summaries".into(),
                true.into(),
            ));
        }
        if explicit.starts_with("profile_summary") {
            app.app_server_target = AppServerTarget::LocalDaemon {
                allow_embedded_fallback: true,
                endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
                    socket_path: client_home.path().join("daemon.sock").abs(),
                },
            };
            app.open_agents_overview(&server);
        }
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.start_fresh_session(
            &mut tui,
            &mut server,
            /*session_start_source*/ None,
            /*initial_user_message*/ None,
            /*new_thread_name*/ None,
        )
        .await;
        let starts = recorded_params(&requests, "thread/start");
        assert_eq!(starts.len(), 1);
        assert_eq!(
            (
                starts[0]["config"].get("model_reasoning_summary"),
                starts[0]["config"]["features"].get("concurrent_reasoning_summaries"),
            ),
            (
                explicit
                    .starts_with("profile_summary")
                    .then_some(&serde_json::json!("detailed")),
                explicit
                    .starts_with("profile_summary")
                    .then_some(&serde_json::json!(explicit != "profile_summary_off")),
            ),
            "{explicit}",
        );
        assert_eq!(
            (
                &starts[0]["model"],
                &starts[0]["config"]["model_reasoning_effort"]
            ),
            (
                &serde_json::json!(expected_model),
                &serde_json::json!(expected_effort)
            ),
        );
        assert_eq!(
            recorded_params(&requests, "config/read"),
            vec![
                serde_json::json!({"cwd": if explicit.starts_with("profile_summary") {
                    client_home.path().display().to_string()
                } else {
                    server_config.cwd.display().to_string()
                }, "includeLayers": true})
            ],
        );
        if explicit == "saved" {
            let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 80)
                .replace(&server_config.cwd.display().to_string(), "<PROJECT>");
            insta::assert_snapshot!(rendered, @r"
            › Ask Codex to do anything

              server-model high · <PROJECT>
            ");
        }
        if explicit.starts_with("profile_summary") {
            // Opening the overview starts background RPCs. Let them finish before
            // closing the client so the proxy is not writing to a closed socket.
            loop {
                let event = events.recv().await.expect("agents overview refresh");
                if matches!(event, AppEvent::AgentsOverviewThreadsLoaded { .. }) {
                    break;
                }
            }
        }
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn replacement_failure_keeps_current_task_and_restores_input() -> Result<()> {
    for fail_read in [false, true] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let home = tempdir()?;
        app.config.codex_home = home.path().to_path_buf().abs();
        app.config.sqlite = SqliteConfig::new_for_testing(home.path().abs());
        let current = ThreadId::new();
        app.enqueue_primary_thread_session(
            test_thread_session(current, app.config.cwd.to_path_buf()),
            Vec::new(),
        )
        .await?;
        while events.try_recv().is_ok() {}
        let original_config = app.config.clone();
        let (mut server, requests, proxy) = start_recording_app_server_with_history(
            &app.config,
            HistoryCapabilities::ThreadStartFails,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            crate::app_server_session::ThreadParamsMode::Embedded,
            LoaderOverrides::default(),
        )
        .await?;
        if fail_read {
            std::fs::write(home.path().join("config.toml"), "invalid = [")?;
        }
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.start_fresh_session(
            &mut tui,
            &mut server,
            /*session_start_source*/ None,
            Some("keep this request".into()),
            /*new_thread_name*/ None,
        )
        .await;
        assert_eq!(app.chat_widget.thread_id(), Some(current));
        assert_eq!(app.active_thread_id, Some(current));
        assert!(app.active_thread_rx.is_some());
        assert_eq!(app.config, original_config);
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "keep this request"
        );
        assert!(recorded_params(&requests, "thread/unsubscribe").is_empty());
        assert_eq!(
            recorded_params(&requests, "thread/start").len(),
            usize::from(!fail_read)
        );
        let errors = std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => {
                    Some(lines_to_single_string(&cell.display_lines(/*width*/ 1000)))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        if fail_read {
            insta::assert_snapshot!(errors, @"■ Failed to read new session defaults: config/read failed in TUI");
        } else {
            insta::assert_snapshot!(errors, @"■ Failed to start a fresh session through the app server: thread/start failed during TUI bootstrap: thread/start failed: replacement unavailable (code -32603)");
        }
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn replacement_preserves_remote_launch_paths_and_older_servers() -> Result<()> {
    for (remote_cwd, capabilities) in [
        (None, HistoryCapabilities::Current),
        (Some(PathBuf::from(".")), HistoryCapabilities::Current),
        (None, HistoryCapabilities::ConfigReadUnsupported(-32601)),
        (None, HistoryCapabilities::ConfigReadUnsupported(-32600)),
    ] {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        let home = tempdir()?;
        app.config.codex_home = home.path().to_path_buf().abs();
        app.config.sqlite = SqliteConfig::new_for_testing(home.path().abs());
        std::fs::write(
            home.path().join("config.toml"),
            "model = \"configured-model\"\n",
        )?;
        let (server, requests, proxy) = start_recording_app_server_with_history(
            &app.config,
            capabilities,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            crate::app_server_session::ThreadParamsMode::Remote,
            LoaderOverrides::default(),
        )
        .await?;
        let mut server = server.with_remote_cwd_override(remote_cwd.clone());
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.start_fresh_session(
            &mut tui,
            &mut server,
            /*session_start_source*/ None,
            /*initial_user_message*/ None,
            /*new_thread_name*/ None,
        )
        .await;
        assert_eq!(
            recorded_params(&requests, "config/read"),
            vec![serde_json::json!({"cwd": ".", "includeLayers": true}),]
        );
        let starts = recorded_params(&requests, "thread/start");
        assert_eq!(starts.len(), 1);
        assert_eq!(
            (&starts[0]["cwd"], &starts[0]["model"]),
            (
                &serde_json::json!(remote_cwd),
                &serde_json::json!("configured-model")
            ),
        );
        assert_eq!(app.chat_widget.current_model(), "configured-model");
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}
