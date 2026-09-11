//! Replacement starts use server defaults and preserve the current task on failure.

use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

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
    app.start_fresh_session_with_summary_hint(
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
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        let server_home = tempdir()?;
        let client_home = tempdir()?;
        std::fs::write(
            server_home.path().join("work.config.toml"),
            "model = \"server-model\"\nmodel_reasoning_effort = \"high\"\n",
        )?;
        std::fs::write(
            client_home.path().join("config.toml"),
            "model = \"stale-client-model\"\nmodel_reasoning_effort = \"low\"\n",
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
            crate::app_server_session::ThreadParamsMode::Remote,
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
            profile @ ("profile" | "profile_model" | "profile_effort" | "profile_unrelated") => {
                let path = client_home.path().join("work.config.toml");
                std::fs::write(
                    &path,
                    match profile {
                        "profile_model" => "model = \"profile-model\"\n",
                        "profile_effort" => "model_reasoning_effort = \"low\"\n",
                        "profile_unrelated" => "model_verbosity = \"low\"\n",
                        _ => "model = \"profile-model\"\nmodel_reasoning_effort = \"low\"\n",
                    },
                )?;
                app.loader_overrides.user_config_path = Some(path.abs());
                app.loader_overrides.user_config_profile = Some("work".parse()?);
            }
            _ => {}
        }
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.start_fresh_session_with_summary_hint(
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
            vec![serde_json::json!({"cwd": server_config.cwd.display().to_string()})],
        );
        if explicit == "saved" {
            let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 80)
                .replace(&server_config.cwd.display().to_string(), "<PROJECT>");
            insta::assert_snapshot!(rendered, @r"
            › Ask Codex to do anything

              server-model high · <PROJECT>
            ");
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
        app.start_fresh_session_with_summary_hint(
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
        app.start_fresh_session_with_summary_hint(
            &mut tui,
            &mut server,
            /*session_start_source*/ None,
            /*initial_user_message*/ None,
            /*new_thread_name*/ None,
        )
        .await;
        assert_eq!(
            recorded_params(&requests, "config/read"),
            vec![serde_json::json!({"cwd": "."}),]
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
