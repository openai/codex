//! Replacement starts use server defaults and preserve the current task on failure.

use super::*;
use crate::app::backend_sessions::BackendSessions;
use codex_app_server_protocol::SkillMetadata;
use codex_app_server_protocol::SkillScope;
use codex_app_server_protocol::SkillsListEntry;
use codex_config::types::HistoryPersistence;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn new_session_refreshes_account_for_the_started_provider() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let home = tempdir()?;
    app.config.codex_home = home.path().abs();
    app.config.sqlite = SqliteConfig::new_for_testing(home.path().abs());
    app.config.cli_auth_credentials_store_mode = codex_login::AuthCredentialsStoreMode::File;
    app_test_support::write_chatgpt_auth(
        home.path(),
        app_test_support::ChatGptAuthFixture::new("test-token")
            .email("user@example.com")
            .plan_type("pro"),
        codex_login::AuthCredentialsStoreMode::File,
    )
    .expect("write synthetic auth");
    let backend = wiremock::MockServer::start().await;
    let catalog_path = home.path().join("models.json");
    std::fs::write(
        &catalog_path,
        serde_json::to_vec(&codex_models_manager::bundled_models_response()?)?,
    )?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "model_provider = 'amazon-bedrock'\nmodel = 'test-model'\ncli_auth_credentials_store = 'file'\nchatgpt_base_url = {:?}\nmodel_catalog_json = {catalog_path:?}\nfeatures.apps = true\n",
            backend.uri(),
        ),
    )?;
    let (mut server, _requests, proxy) = Box::pin(start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    ))
    .await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for provider in ["amazon-bedrock", "openai", "amazon-bedrock"] {
        crate::config_update::write_config_batch(
            server.request_handle(),
            vec![crate::config_update::replace_config_value(
                "model_provider",
                provider.into(),
            )],
        )
        .await?;
        *app.model_catalog.provider_write.lock().await = true;
        Box::pin(app.start_fresh_session(
            &mut tui,
            &mut server,
            /*session_start_source*/ None,
            /*initial_user_message*/ None,
            /*new_thread_name*/ None,
        ))
        .await;
        let chatgpt = provider == "openai";
        assert_eq!(
            (
                app.chat_widget.config_ref().model_provider_id.as_str(),
                app.chat_widget.has_chatgpt_account(),
                app.chat_widget.has_codex_backend_auth(),
                app.chat_widget.requires_openai_auth,
                app.chat_widget.current_plan_type()
            ),
            (
                provider,
                chatgpt,
                chatgpt,
                chatgpt,
                chatgpt.then_some(codex_protocol::account::PlanType::Pro)
            )
        );
    }
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn thread_account_uses_only_a_matching_server_default() -> Result<()> {
    let (client, mut commands, _events) =
        codex_app_server_client::CloudClient::channel(/*capacity*/ 1);
    let mut server = AppServerSession::new(
        crate::backend_id::BackendId::new(),
        codex_app_server_client::AppServerClient::Cloud(client),
        crate::app_server_session::ThreadParamsMode::Remote,
    );
    for (provider, default_at_config_read) in [
        ("openai", "openai"),
        ("amazon-bedrock", "openai"),
        // The server switched to Bedrock after account/read returned ChatGPT.
        ("amazon-bedrock", "amazon-bedrock"),
    ] {
        let id = ThreadId::new();
        let mut session = test_thread_session(id, test_path_buf("/project"));
        session.model_provider_id = provider.to_string();
        let (account, ()) = tokio::join!(server.read_thread_account(&session), async {
            use codex_app_server_client::CloudOperation::Request;
            let command = commands.recv().await.unwrap();
            assert!(matches!(command.operation, Request(req) if matches!(
                req.as_ref(),
                ClientRequest::GetAccount { .. }
            )));
            command
                .response
                .send(Ok(Ok(serde_json::json!({
                    "account": {"type": "chatgpt", "email": "user@example.com", "planType": "pro"},
                    "requiresOpenaiAuth": true
                }))))
                .unwrap();
            let command = commands.recv().await.unwrap();
            assert!(matches!(command.operation, Request(req) if matches!(
                req.as_ref(),
                ClientRequest::ConfigRead { params, .. } if params.cwd.is_none()
            )));
            command
                .response
                .send(Ok(Ok(serde_json::json!({
                    "config": { "model_provider": default_at_config_read },
                    "origins": {}
                }))))
                .unwrap();
        });
        if provider == "openai" {
            assert!(account?.has_chatgpt_account());
        } else {
            assert!(account.is_err());
        }
    }
    Ok(())
}

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
async fn widget_replacement_preserves_mention_usage_with_history_disabled() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    app.local_settings.history.persistence = HistoryPersistence::None;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let skills = SkillsListResponse {
        data: vec![SkillsListEntry {
            cwd: app.config.cwd.to_path_buf(),
            skills: ["statsig", "status"]
                .into_iter()
                .map(|name| SkillMetadata {
                    name: name.to_owned(),
                    description: String::new(),
                    short_description: None,
                    interface: None,
                    dependencies: None,
                    path: app
                        .config
                        .cwd
                        .to_path_buf()
                        .join(name)
                        .join("SKILL.md")
                        .abs()
                        .into(),
                    scope: SkillScope::User,
                    enabled: true,
                    plugin_id: None,
                })
                .collect(),
            errors: Vec::new(),
        }],
    };
    let init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    app.chat_widget.set_skills_from_response(&skills);
    app.chat_widget.insert_str("$status");
    app.chat_widget.handle_key_event(KeyCode::Enter.into());

    let init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    app.chat_widget.set_skills_from_response(&skills);
    app.chat_widget.insert_str("$stat");
    insta::assert_snapshot!(normalize_snapshot_paths(render_bottom_popup(
        &app.chat_widget,
        /*width*/ 80
    )));
    assert!(
        !app.local_settings
            .codex_home
            .join("mention_usage.json")
            .exists()
    );
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
            let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 140)
                .replace(&server_config.cwd.display().to_string(), "<PROJECT>");
            insta::assert_snapshot!(rendered, @r"
            › Ask Codex to do anything

              server-model high · <PROJECT> · ← for agents
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
            server.shutdown().await?;
        } else {
            insta::assert_snapshot!(errors, @"■ Failed to start a fresh session through the app server: thread/start failed during TUI bootstrap: thread/start failed: replacement unavailable (code -32603)");

            let source = crate::start_embedded_app_server_for_picker(&app.config).await?;
            let source_id = source.backend_id();
            let destination = server.backend_id();
            let mut backends = BackendSessions::from_initial(source, vec![server]);
            let destination_cwd = tempdir()?;
            std::fs::create_dir(destination_cwd.path().join(".codex"))?;
            std::fs::write(
                destination_cwd.path().join(".codex/hooks.json"),
                r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"x"}]}]}}"#,
            )?;
            // Model a destination whose trust was saved before thread/start failed.
            crate::legacy_core::config::set_project_trust_level(
                home.path(),
                destination_cwd.path(),
                codex_protocol::config_types::TrustLevel::Trusted,
            )
            .map_err(std::io::Error::other)?;
            let mut settings = app.backend_settings();
            settings.config.cwd = destination_cwd.path().to_path_buf().abs();
            backends.settings.insert(destination, settings.clone());
            backends.set_preference_key(destination, "remote.work");
            let preference_path = app.local_settings.user_config_path.clone();
            let original_preferences = std::fs::read(&preference_path).ok();
            let error = app
                .start_new_session_on_other_backend(
                    &mut tui,
                    &mut backends,
                    destination,
                    crate::app::backend_selection::NewBackendSession::CarryDraft,
                )
                .await
                .unwrap_err();
            assert!(error.contains("replacement unavailable"));
            assert_eq!(std::fs::read(&preference_path).ok(), original_preferences);
            assert_eq!(backends.displayed_owner(), source_id);
            assert_eq!(app.config, original_config);
            assert_eq!(app.active_thread_id, Some(current));
            assert!(app.active_thread_rx.is_some());
            assert_eq!(app.chat_widget.thread_id(), Some(current));
            assert_eq!(
                app.chat_widget.composer_text_with_pending(),
                "keep this request"
            );
            let (retry, retry_requests, retry_proxy) = start_recording_app_server(
                &app.config,
                /*blocked_thread_list*/ None,
                /*failed_thread_name*/ None,
            )
            .await?;
            let retry_id = retry.backend_id();
            backends.insert(retry);
            backends.settings.insert(retry_id, settings);
            app.start_new_session_on_other_backend(
                &mut tui,
                &mut backends,
                retry_id,
                crate::app::backend_selection::NewBackendSession::CarryDraft,
            )
            .await
            .expect("retry switching to the trusted destination");
            assert_eq!(backends.displayed_owner(), retry_id);
            assert_eq!(recorded_params(&retry_requests, "thread/start").len(), 1);
            assert_eq!(
                recorded_params(&retry_requests, "hooks/list")[0]["cwds"],
                serde_json::json!([destination_cwd.path()])
            );
            assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("SessionStart"));
            backends.shutdown().await?;
            retry_proxy.await??;
        }
        proxy.await??;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn backend_handoff_errors_are_snapshotted() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let source = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let (destination, requests, destination_proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::ConfigReadFails,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::without_managed_config_for_tests(),
    )
    .await?;
    let destination_id = destination.backend_id();
    let target = crate::resume_picker::SessionTarget {
        backend_id: destination_id,
        path: None,
        thread_id: ThreadId::new(),
        cwd: None,
        history_mode: None,
    };
    let mut backends = BackendSessions::from_initial(source, vec![destination]);
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.resume_backend_session(&mut tui, &mut backends, target.clone())
        .await?;
    insta::assert_snapshot!(next_history_message(&mut events), @"■ Connection settings are unavailable. Open the session picker again.");

    backends
        .settings
        .insert(destination_id, app.backend_settings());
    app.resume_backend_session(&mut tui, &mut backends, target)
        .await?;
    let trust_error = next_history_message(&mut events);
    insta::assert_snapshot!(trust_error, @"■ Unable to check folder trust: config/read failed while checking remote project trust");

    // Starting a fresh thread must observe the same destination trust check as resuming one.
    app.start_new_session_on_other_backend(
        &mut tui,
        &mut backends,
        destination_id,
        crate::app::backend_selection::NewBackendSession::Fresh { name: None },
    )
    .await
    .expect("trust failure is shown in the conversation");
    assert_eq!(next_history_message(&mut events), trust_error);
    assert!(recorded_params(&requests, "thread/start").is_empty());

    backends.shutdown().await?;
    destination_proxy.await??;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resumed_backend_handoff_detaches_through_the_source_connection() -> Result<()> {
    let mut app = make_test_app().await;
    let (source, source_requests, source_proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let (mut destination, destination_requests, destination_proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let destination_id = destination.backend_id();
    let resumed = destination.start_thread(&app.config).await?;
    let resumed_id = resumed.session.thread_id;
    let saved_item = serde_json::from_str(
        r#"{"type":"message","role":"assistant","content":[{"type":"output_text","text":"saved history"}]}"#,
    )?;
    destination
        .thread_inject_items(resumed.session.thread_id, vec![saved_item])
        .await?;
    let current = ThreadId::new();
    app.ensure_thread_channel(ThreadId::new()).backend_id =
        Some(crate::backend_id::BackendId::new());
    app.enqueue_primary_thread_session(
        test_thread_session(current, app.config.cwd.to_path_buf()),
        Vec::new(),
    )
    .await?;
    let target = crate::resume_picker::SessionTarget {
        backend_id: destination_id,
        path: None,
        thread_id: resumed_id,
        cwd: Some(resumed.session.cwd.to_path_buf()),
        history_mode: None,
    };
    let mut backends = BackendSessions::from_initial(source, vec![destination]);
    backends
        .settings
        .insert(destination_id, app.backend_settings());
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut stale_input = app.chat_widget.capture_thread_input_state().unwrap();
    stale_input.provider_error = Some(
        codex_app_server_protocol::ModelProviderError::ModelProviderNotConfigured {
            model_provider: "repaired-provider".into(),
        },
    );
    app.agents_overview
        .input_states
        .insert(resumed_id, stale_input);

    app.resume_backend_session(&mut tui, &mut backends, target)
        .await?;

    assert_eq!(backends.displayed_owner(), destination_id);
    assert_eq!(app.chat_widget.thread_id(), Some(resumed_id));
    assert!(!app.chat_widget.has_provider_error());
    assert_eq!(
        recorded_params(&source_requests, "thread/unsubscribe"),
        vec![serde_json::json!({"threadId": current.to_string()})]
    );
    assert!(recorded_params(&destination_requests, "thread/unsubscribe").is_empty());
    backends.shutdown().await?;
    source_proxy.await??;
    destination_proxy.await??;
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
        app.enqueue_primary_thread_session(
            test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf()),
            Vec::new(),
        )
        .await?;
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

#[tokio::test]
async fn provider_continuation_starts_current_provider_and_drafts_saved_reference() -> Result<()> {
    use crate::app_server_session::ThreadParamsMode;
    for (mode, capabilities) in [
        (ThreadParamsMode::Embedded, HistoryCapabilities::Current),
        (ThreadParamsMode::Remote, HistoryCapabilities::Current),
        (
            ThreadParamsMode::Embedded,
            HistoryCapabilities::ThreadStartFails,
        ),
    ] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let home = tempdir()?;
        app.config.codex_home = home.path().to_path_buf().abs();
        app.config.sqlite = SqliteConfig::new_for_testing(home.path().abs());
        std::fs::write(
            home.path().join("config.toml"),
            "model = 'current-model'\n[tui]\nresume_cwd = 'current'\n",
        )?;
        crate::legacy_core::config::set_project_trust_level(
            home.path(),
            app.config.cwd.as_path(),
            codex_protocol::config_types::TrustLevel::Trusted,
        )
        .map_err(std::io::Error::other)?;
        let source_id = ThreadId::from_string(
            &create_fake_rollout(
                home.path(),
                "2025-01-05T12-00-00",
                "2025-01-05T12:00:00Z",
                "Saved provider history",
                Some("old-provider"),
                /*git_info*/ None,
            )
            .map_err(|err| color_eyre::eyre::eyre!(err.to_string()))?,
        )?;
        let (mut server, requests, proxy) = start_recording_app_server_with_history(
            &app.config,
            capabilities,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            mode,
            LoaderOverrides::default(),
        )
        .await?;
        server
            .thread_set_name(source_id, "Launch readiness".into())
            .await?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.set_owned_screen(matches!(mode, ThreadParamsMode::Embedded))?;
        let target = crate::resume_picker::SessionTarget {
            backend_id: server.backend_id(),
            path: None,
            thread_id: source_id,
            cwd: None,
            history_mode: None,
        };
        app.resume_target_session(&mut tui, &mut server, target)
            .await?;
        assert!(app.chat_widget.has_provider_error());
        let path = app.chat_widget.rollout_path().expect("saved history path");
        let original_history = std::fs::read(&path)?;
        while events.try_recv().is_ok() {}
        app.handle_tui_event(
            &mut tui,
            &mut server,
            TuiEvent::Key(KeyCode::Char('c').into()),
        )
        .await?;
        let action = std::iter::from_fn(|| events.try_recv().ok())
            .find(|event| matches!(event, AppEvent::ContinueProviderChat { .. }))
            .expect("c starts a continuation");
        Box::pin(app.handle_event(&mut tui, &mut server, action)).await?;
        if capabilities == HistoryCapabilities::ThreadStartFails {
            assert_eq!(app.chat_widget.thread_id(), Some(source_id));
            assert!(app.chat_widget.has_provider_error());
        } else {
            let new_id = app.chat_widget.thread_id().expect("new thread");
            assert_ne!(new_id, source_id);
            assert!(!app.chat_widget.has_provider_error());
            assert_eq!(app.chat_widget.current_model(), "current-model");
            assert_eq!(
                app.primary_session_configured
                    .as_ref()
                    .unwrap()
                    .model_provider_id,
                "openai"
            );
            let mut submissions = Vec::new();
            while let Ok(event) = events.try_recv() {
                match event {
                    AppEvent::SubmitThreadOp {
                        thread_id,
                        op: Op::UserTurn { items, .. },
                    } => {
                        assert_eq!(thread_id, new_id);
                        submissions.push(items);
                    }
                    AppEvent::CodexOp(Op::UserTurn { items, .. }) => submissions.push(items),
                    event => {
                        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
                    }
                }
            }
            assert!(submissions.is_empty());
            let expected_draft = format!(
                "Read “Launch readiness” and let’s continue where we left off.\n\nSaved conversation: {}",
                path.display()
            );
            assert_eq!(app.chat_widget.composer_text_with_pending(), expected_draft);
            let composer_lines = render_bottom_popup(&app.chat_widget, /*width*/ 80)
                .lines()
                .take(2)
                .collect::<Vec<_>>()
                .join("\n");
            insta::assert_snapshot!("provider_continuation_draft", composer_lines);
            // A repeated queued key cannot create a second chat or send into the original.
            Box::pin(app.handle_event(
                &mut tui,
                &mut server,
                AppEvent::ContinueProviderChat {
                    thread_id: source_id,
                },
            ))
            .await?;
            assert_eq!(app.chat_widget.thread_id(), Some(new_id));
            assert_eq!(app.chat_widget.composer_text_with_pending(), expected_draft);

            let edited_draft = format!("{expected_draft}\n\nFocus on the remaining launch checks.");
            app.chat_widget.apply_external_edit(edited_draft.clone());
            app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Enter.into()))
                .await?;
            while let Ok(event) = events.try_recv() {
                match event {
                    AppEvent::SubmitThreadOp {
                        thread_id,
                        op: Op::UserTurn { items, .. },
                    } => {
                        assert_eq!(thread_id, new_id);
                        submissions.push(items);
                    }
                    AppEvent::CodexOp(Op::UserTurn { items, .. }) => submissions.push(items),
                    event => {
                        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
                    }
                }
            }
            assert_eq!(
                submissions,
                vec![vec![codex_app_server_protocol::UserInput::Text {
                    text: edited_draft,
                    text_elements: Vec::new(),
                }]]
            );
            assert_eq!(app.chat_widget.composer_text_with_pending(), "");
        }
        assert_eq!(recorded_params(&requests, "thread/start").len(), 1);
        assert!(recorded_params(&requests, "turn/start").is_empty());
        assert_eq!(std::fs::read(&path)?, original_history);
        tui.set_owned_screen(/*owned*/ false)?;
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn backend_cycling_waits_for_foreground_and_parked_voice_calls() -> Result<()> {
    for parked in [false, true] {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        let source = crate::start_embedded_app_server_for_picker(&app.config).await?;
        let destination = crate::start_embedded_app_server_for_picker(&app.config).await?;
        let source_id = source.backend_id();
        let destination_id = destination.backend_id();
        let thread_id = ThreadId::new();
        app.enqueue_primary_thread_session(
            test_thread_session(thread_id, app.config.cwd.to_path_buf()),
            Vec::new(),
        )
        .await?;
        assert!(app.can_cycle_conversation_backend());
        crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, thread_id);
        if parked {
            let (visible, _, _, _) =
                crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
            app.background_voice = Some(Box::new(std::mem::replace(&mut app.chat_widget, visible)));
        }
        app.chat_widget
            .restore_user_message_to_composer("keep draft".into());
        assert!(!app.can_cycle_conversation_backend());
        let mut backends = BackendSessions::from_initial(source, vec![destination]);
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let error = app
            .start_new_session_on_other_backend(
                &mut tui,
                &mut backends,
                destination_id,
                crate::app::backend_selection::NewBackendSession::CarryDraft,
            )
            .await
            .unwrap_err();
        insta::allow_duplicates! {
            insta::assert_snapshot!(error, @"End the voice call before switching.");
        }
        assert_eq!(backends.displayed_owner(), source_id);
        assert_eq!(app.voice_owner_thread_id(), Some(thread_id));
        assert_eq!(app.chat_widget.composer_text_with_pending(), "keep draft");
        backends.shutdown().await?;
    }
    Ok(())
}
