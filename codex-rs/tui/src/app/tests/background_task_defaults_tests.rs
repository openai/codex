//! Request-level coverage for command-center session defaults and worktree creation.

use super::*;
use crate::app::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use crate::app::tests::session_lifecycle_requests::HistoryCapabilities;
use crate::app::tests::session_lifecycle_requests::recorded_params;
use crate::app::tests::session_lifecycle_requests::start_recording_app_server_with_history;
use crate::model_catalog::ModelCatalog;
use crate::test_support::PathBufExt;
use crate::tui::test_support::make_test_tui;
use codex_state::SqliteConfig;
use pretty_assertions::assert_eq;

fn trust_launch_folder(app: &mut App) {
    let projects = serde_json::json!({
        app.config.cwd.display().to_string(): {"trust_level": "trusted"},
        app.chat_widget.config_ref().cwd.display().to_string(): {"trust_level": "trusted"},
    });
    app.cli_kv_overrides.push((
        "projects".into(),
        TomlValue::try_from(projects).expect("trust fixture"),
    ));
    app.config.active_project.trust_level = Some(codex_protocol::config_types::TrustLevel::Trusted);
}

#[tokio::test]
async fn command_center_new_reads_server_defaults_for_actual_destination() -> Result<()> {
    let mut tui = make_test_tui()?;
    for (mode, explicit_cwd, launch_override, expected_cwd, expected_model) in [
        ("local", false, false, "launch", "server-model"),
        ("local", true, false, "destination", "destination-model"),
        ("local-cli-provider", false, false, "launch", "server-model"),
        ("local-cli-model", false, false, "launch", "cli-model"),
        (
            "local-default-provider",
            false,
            false,
            "launch",
            "server-model",
        ),
        ("remote", true, true, "destination", "destination-model"),
        ("remote", true, false, "destination", "destination-model"),
        ("remote", false, true, "launch", "server-model"),
        ("remote", false, false, ".", "server-model"),
        ("remote-null-fast", false, false, ".", ""),
    ] {
        let client_home = tempdir()?;
        let server_home = tempdir()?;
        let launch = tempdir()?;
        let destination = tempdir()?;
        std::fs::write(
            client_home.path().join("config.toml"),
            format!(
                "model = \"{}\"\nmodel_reasoning_effort = \"low\"\n{}",
                if mode == "remote-null-fast" {
                    "gpt-5.2"
                } else {
                    "client-model"
                },
                if mode == "local-default-provider" {
                    "model_provider = \"ollama\"\n"
                } else {
                    ""
                }
            ),
        )?;
        std::fs::write(
            server_home.path().join("config.toml"),
            format!(
                "{}model_reasoning_effort = \"high\"\n{}",
                if mode == "remote-null-fast" {
                    ""
                } else {
                    "model = \"server-model\"\n"
                },
                if mode == "local" || mode == "local-cli-provider" || mode == "local-cli-model" {
                    "model_provider = \"ollama\"\n"
                } else {
                    ""
                }
            ),
        )?;
        std::fs::create_dir(destination.path().join(".codex"))?;
        std::fs::write(
            destination.path().join(".codex/config.toml"),
            "model = \"destination-model\"\nservice_tier = \"flex\"\n",
        )?;
        for home in [client_home.path(), server_home.path()] {
            crate::legacy_core::config::set_project_trust_level(
                home,
                destination.path(),
                codex_protocol::config_types::TrustLevel::Trusted,
            )
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
        }
        let mut app = make_test_app_with_channels().await.0;
        trust_launch_folder(&mut app);
        if mode == "local" && explicit_cwd {
            app.harness_overrides.model_provider = Some("openai".into());
        }
        if mode == "local-cli-provider" {
            app.cli_kv_overrides
                .push(("model_provider".into(), TomlValue::String("openai".into())));
        }
        if mode == "local-cli-model" {
            app.harness_overrides.model = Some("cli-model".into());
        }
        app.chat_widget.set_service_tier(Some("priority".into()));
        app.harness_overrides.cwd = Some(launch.path().to_path_buf());
        app.config = ConfigBuilder::default()
            .codex_home(client_home.path().to_path_buf())
            .loader_overrides(app.loader_overrides.clone())
            .cli_overrides(app.cli_kv_overrides.clone())
            .harness_overrides(app.harness_overrides.clone())
            .build()
            .await?;
        if mode == "remote-null-fast" {
            app.config.features.enable(Feature::FastMode)?;
        }
        app.chat_widget
            .handle_thread_session_quiet(test_thread_session(
                ThreadId::new(),
                launch.path().to_path_buf(),
            ));
        trust_launch_folder(&mut app);
        let mut server_config = app.config.clone();
        server_config.codex_home = server_home.path().to_path_buf().abs();
        server_config.sqlite = SqliteConfig::new_for_testing(server_home.path().abs());
        let thread_mode = if mode.starts_with("remote") {
            crate::app_server_session::ThreadParamsMode::Remote
        } else {
            crate::app_server_session::ThreadParamsMode::Embedded
        };
        let (mut server, requests, proxy) = start_recording_app_server_with_history(
            &server_config,
            HistoryCapabilities::Current,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            thread_mode,
            LoaderOverrides {
                user_config_path: Some(server_home.path().join("config.toml").abs()),
                ..LoaderOverrides::default()
            },
        )
        .await?;
        if launch_override {
            server = server.with_remote_cwd_override(Some(launch.path().to_path_buf()));
        }
        if mode.starts_with("remote") && !launch_override {
            app.app_server_target = AppServerTarget::Remote {
                endpoint: crate::RemoteAppServerEndpoint::WebSocket {
                    websocket_url: "ws://127.0.0.1:1".into(),
                    auth_token: None,
                },
            };
            let mut restored = app.config.clone();
            restored
                .permissions
                .set_permission_profile(codex_protocol::models::PermissionProfile::Disabled)?;
            app.runtime_permission_profile_override = Some(
                RuntimePermissionProfileOverride::from_restored_config(&restored),
            );
            app.runtime_approval_policy_override = Some(RuntimeApprovalPolicyOverride::Restored(
                codex_app_server_protocol::AskForApproval::Never,
            ));
        }
        let bootstrap = server.bootstrap(&app.config).await?;
        let expected_model = if mode == "remote-null-fast" {
            let default_model = bootstrap
                .available_models
                .iter()
                .find(|model| model.is_default)
                .or_else(|| bootstrap.available_models.first())
                .expect("server catalog model")
                .model
                .clone();
            app.model_catalog = Arc::new(ModelCatalog::new(bootstrap.available_models));
            default_model
        } else {
            expected_model.to_string()
        };
        app.new_agents_overview_session(
            &mut tui,
            &mut server,
            explicit_cwd.then(|| destination.path().to_path_buf().abs()),
        )
        .await?;
        let cwd = match expected_cwd {
            "launch" => launch.path().display().to_string(),
            "destination" => destination.path().display().to_string(),
            "." => ".".to_string(),
            _ => unreachable!(),
        };
        assert_eq!(
            recorded_params(&requests, "config/read"),
            vec![serde_json::json!({"cwd": cwd})],
            "{mode} {expected_cwd}"
        );
        let starts = recorded_params(&requests, "thread/start");
        assert_eq!(starts.len(), 1, "{mode} {expected_cwd}");
        assert_eq!(
            (
                &starts[0]["cwd"],
                &starts[0]["model"],
                &starts[0]["modelProvider"],
                &starts[0]["config"]["model_reasoning_effort"],
                &starts[0]["serviceTier"],
            ),
            (
                &if mode.starts_with("remote") && !explicit_cwd && !launch_override {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(cwd)
                },
                &serde_json::json!(expected_model),
                &if mode.starts_with("remote") {
                    serde_json::Value::Null
                } else if mode == "local" && !explicit_cwd {
                    serde_json::json!("ollama")
                } else {
                    serde_json::json!("openai")
                },
                &serde_json::json!("high"),
                &serde_json::json!(if mode == "local" && explicit_cwd {
                    "flex"
                } else {
                    "priority"
                }),
            ),
            "{mode} {expected_cwd}"
        );
        if mode.starts_with("remote") && !launch_override {
            assert_eq!(app.config.cwd, launch.path().to_path_buf().abs());
            assert_ne!(starts[0]["sandbox"], "danger-full-access");
            assert_ne!(starts[0]["approvalPolicy"], "never");
        }
        assert_eq!(recorded_params(&requests, "turn/start").len(), 0);
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn command_center_new_preserves_explicit_choices_and_managed_defaults() -> Result<()> {
    let mut tui = make_test_tui()?;
    for (choice, expected_model, expected_effort) in [
        ("saved", "server-model", "high"),
        ("cli_effort", "server-model", "low"),
        ("profile_model", "profile-model", "high"),
        ("profile_managed", "profile-model", "low"),
        ("managed", "managed-model", "medium"),
    ] {
        let client_home = tempdir()?;
        let server_home = tempdir()?;
        std::fs::write(
            client_home.path().join("config.toml"),
            "model = \"client-model\"\nmodel_reasoning_effort = \"low\"\n",
        )?;
        std::fs::write(
            server_home.path().join("config.toml"),
            "model = \"server-model\"\nmodel_reasoning_effort = \"high\"\n",
        )?;
        if choice == "managed" || choice == "profile_managed" || choice.starts_with("cli_") {
            std::fs::write(
                server_home.path().join("requirements.toml"),
                "[models.new_thread]\nmodel = \"managed-model\"\nmodel_reasoning_effort = \"medium\"\n",
            )?;
        }
        let mut app = make_test_app_with_channels().await.0;
        trust_launch_folder(&mut app);
        match choice {
            "cli_effort" => app.cli_kv_overrides.push((
                "model_reasoning_effort".into(),
                TomlValue::String("low".into()),
            )),
            "profile_model" | "profile_managed" => {
                let path = client_home.path().join("work.config.toml");
                std::fs::write(
                    &path,
                    if choice == "profile_managed" {
                        "model = \"profile-model\"\nmodel_reasoning_effort = \"low\"\n"
                    } else {
                        "model = \"profile-model\"\n"
                    },
                )?;
                app.loader_overrides.user_config_path = Some(path.abs());
                app.loader_overrides.user_config_profile = Some("work".parse()?);
            }
            _ => {}
        }
        app.config = ConfigBuilder::default()
            .codex_home(client_home.path().to_path_buf())
            .loader_overrides(app.loader_overrides.clone())
            .cli_overrides(app.cli_kv_overrides.clone())
            .harness_overrides(app.harness_overrides.clone())
            .build()
            .await?;
        trust_launch_folder(&mut app);
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
                user_config_path: Some(server_home.path().join("config.toml").abs()),
                system_requirements_path: Some(server_home.path().join("requirements.toml")),
                ..LoaderOverrides::default()
            },
        )
        .await?;
        server.bootstrap(&app.config).await?;
        app.new_agents_overview_session(&mut tui, &mut server, /*cwd*/ None)
            .await?;
        let starts = recorded_params(&requests, "thread/start");
        assert_eq!(starts.len(), 1, "{choice}");
        assert_eq!(
            (
                &starts[0]["model"],
                &starts[0]["config"]["model_reasoning_effort"]
            ),
            (
                &serde_json::json!(expected_model),
                &serde_json::json!(expected_effort)
            ),
            "{choice}"
        );
        assert_eq!(
            recorded_params(&requests, "turn/start").len(),
            0,
            "{choice}"
        );
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn command_center_new_read_failure_keeps_overview_and_does_not_start() -> Result<()> {
    let mut tui = make_test_tui()?;
    for capability in [
        HistoryCapabilities::ConfigReadFails,
        HistoryCapabilities::ThreadStartFails,
        HistoryCapabilities::ConfigReadUnsupported(-32600),
        HistoryCapabilities::ConfigReadUnsupported(-32601),
    ] {
        let (mut app, mut events, _) = make_test_app_with_channels().await;
        trust_launch_folder(&mut app);
        app.harness_overrides.model = Some("local-model".into());
        let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
        app.chat_widget.show_bottom_pane_view(Box::new(view));
        let (mut server, requests, proxy) = start_recording_app_server_with_history(
            &app.config,
            capability,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            crate::app_server_session::ThreadParamsMode::Embedded,
            LoaderOverrides::default(),
        )
        .await?;
        let source_colors = !app.config.tui_status_line_use_colors;
        app.local_settings.tui.status_line_use_colors = source_colors;
        app.new_agents_overview_session(&mut tui, &mut server, /*cwd*/ None)
            .await?;
        let failed = matches!(
            capability,
            HistoryCapabilities::ConfigReadFails | HistoryCapabilities::ThreadStartFails
        );
        assert_eq!(recorded_params(&requests, "config/read").len(), 1);
        assert_eq!(
            recorded_params(&requests, "thread/start").len(),
            usize::from(capability != HistoryCapabilities::ConfigReadFails)
        );
        assert_eq!(recorded_params(&requests, "turn/start").len(), 0);
        if failed {
            assert_eq!(app.local_settings.tui.status_line_use_colors, source_colors);
            assert!(app.agents_overview.dispatched_requests.is_empty());
            assert!(
                app.chat_widget
                    .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
                    .is_some()
            );
            let error = std::iter::from_fn(|| events.try_recv().ok())
                .filter_map(|event| match event {
                    AppEvent::InsertHistoryCell(cell) => {
                        Some(lines_to_single_string(&cell.display_lines(/*width*/ 80)))
                    }
                    _ => None,
                })
                .find(|message| message.contains("Failed to"))
                .expect("visible read error");
            if capability == HistoryCapabilities::ConfigReadFails {
                insta::assert_snapshot!(error, @"■ Failed to load new session settings: config/read failed in TUI");
            } else {
                insta::assert_snapshot!(
                    "command_center_session_start_error",
                    crate::chatwidget::tests::helpers::render_bottom_popup(
                        &app.chat_widget,
                        /*width*/ 80,
                    )
                );
            }
        } else {
            assert_eq!(
                recorded_params(&requests, "thread/start")[0]["model"],
                "local-model"
            );
        }
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn command_center_new_preserves_permissions_across_sessions() -> Result<()> {
    let mut app = make_test_app_with_channels().await.0;
    trust_launch_folder(&mut app);
    let (mut server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::Current,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;
    assert!(
        app.apply_permission_profile_selection(PermissionProfileSelection {
            profile_id: ":read-only".into(),
            approval_policy: Some(AskForApproval::UnlessTrusted),
            approvals_reviewer: Some(ApprovalsReviewer::User),
            display_label: "Read Only".into(),
        })
        .await
    );
    let mut tui = make_test_tui()?;
    for _ in 0..2 {
        app.new_agents_overview_session(&mut tui, &mut server, /*cwd*/ None)
            .await?;
        assert_eq!(
            (
                app.chat_widget
                    .config_ref()
                    .permissions
                    .permission_profile(),
                app.chat_widget
                    .config_ref()
                    .permissions
                    .approval_policy
                    .value(),
                app.chat_widget
                    .config_ref()
                    .permissions
                    .active_permission_profile(),
            ),
            (
                &codex_protocol::models::PermissionProfile::read_only(),
                codex_protocol::protocol::AskForApproval::UnlessTrusted,
                Some(ActivePermissionProfile::new(":read-only")),
            ),
        );
    }
    assert_eq!(recorded_params(&requests, "thread/start").len(), 2);
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn command_center_new_preserves_only_selected_server_profiles() -> Result<()> {
    let mut app = make_test_app_with_channels().await.0;
    trust_launch_folder(&mut app);
    let home = tempdir()?;
    std::fs::write(
        home.path().join("config.toml"),
        "default_permissions = \":workspace\"\n[permissions.server-only]\nextends = \":read-only\"\n",
    )?;
    let server_config = ConfigBuilder::default()
        .codex_home(home.path().into())
        .build()
        .await?;
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr("ws://127.0.0.1:8765")?,
    };
    let client = crate::start_embedded_app_server(
        codex_arg0::Arg0DispatchPaths::default(),
        server_config,
        Vec::new(),
        LoaderOverrides::default(),
        /*strict_config*/ false,
        CloudConfigBundleLoader::default(),
        codex_feedback::CodexFeedback::new(),
        /*log_db*/ None,
        /*state_db*/ None,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
    )
    .await?;
    let mut server = AppServerSession::new(
        codex_app_server_client::AppServerClient::InProcess(client),
        crate::app_server_session::ThreadParamsMode::Remote,
    );
    let selection = PermissionProfileSelection {
        profile_id: "server-only".into(),
        approval_policy: Some(AskForApproval::OnRequest),
        approvals_reviewer: Some(ApprovalsReviewer::User),
        display_label: "server-only".into(),
    };
    let started = server
        .start_thread_with_session_start_source(
            &app.local_settings,
            &app.config,
            /*session_start_source*/ None,
            /*remote_cwd_override*/ None,
            Some(&selection),
        )
        .await?;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app.runtime_permission_profile_override = Some(
        RuntimePermissionProfileOverride::from_restored_config(app.chat_widget.config_ref()),
    );
    let mut tui = make_test_tui()?;
    app.new_agents_overview_session(&mut tui, &mut server, /*cwd*/ None)
        .await?;
    assert_eq!(
        app.chat_widget
            .config_ref()
            .permissions
            .active_permission_profile(),
        None
    );
    let thread_id = app.chat_widget.thread_id().expect("new session");
    let other = server.start_thread(&app.config).await?.session.thread_id;
    // Exercise the persisted-history attachment path rather than blank-task reuse.
    app.agents_overview.blank_sessions.remove(&thread_id);
    for id in [thread_id, other] {
        server.thread_inject_items(id, vec![serde_json::from_value(serde_json::json!({
            "type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "saved history"}]
        }))?]).await?;
    }
    app.select_permission_profile(&mut server, selection.clone())
        .await;
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    app.new_agents_overview_session(&mut tui, &mut server, /*cwd*/ None)
        .await?;
    assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
    insta::assert_snapshot!(
        "command_center_pending_server_permissions",
        crate::chatwidget::tests::helpers::render_bottom_popup(&app.chat_widget, /*width*/ 80)
    );
    let settings = next_thread_settings_updated(&mut server, thread_id).await;
    app.enqueue_thread_notification(
        thread_id,
        ServerNotification::ThreadSettingsUpdated(settings),
    )
    .await?;
    for target in [other, thread_id] {
        app.select_agents_overview_thread(&mut tui, &mut server, target)
            .await?;
        assert_eq!(
            app.chat_widget.thread_id(),
            Some(target),
            "{}",
            crate::chatwidget::tests::helpers::render_bottom_popup(
                &app.chat_widget,
                /*width*/ 120
            )
        );
    }
    for attempt in 0..3 {
        let previous_thread_id = app.chat_widget.thread_id();
        app.new_agents_overview_session(&mut tui, &mut server, /*cwd*/ None)
            .await?;
        assert_ne!(app.chat_widget.thread_id(), previous_thread_id);
        let config = app.chat_widget.config_ref();
        assert_eq!(
            (
                config.permissions.active_permission_profile(),
                config.permissions.permission_profile(),
                config.permissions.approval_policy.value(),
            ),
            (
                Some(ActivePermissionProfile {
                    id: "server-only".into(),
                    extends: Some(":read-only".into()),
                }),
                &PermissionProfile::read_only(),
                codex_protocol::protocol::AskForApproval::OnRequest,
            )
        );
        if attempt == 1 {
            app.agents_overview
                .selected_permission_profiles
                .remove(&app.chat_widget.thread_id().unwrap());
            app.select_permission_profile(&mut server, selection.clone())
                .await;
        }
    }
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn command_center_new_restores_blank_drafts_and_builtin_permissions() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    trust_launch_folder(&mut app);
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr("ws://127.0.0.1:8765")?,
    };
    app.cli_kv_overrides.push((
        "approvals_reviewer".into(),
        TomlValue::String("auto_review".into()),
    ));
    let (mut server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::Current,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Remote,
        LoaderOverrides::default(),
    )
    .await?;
    let mut tui = make_test_tui()?;
    app.new_agents_overview_session(&mut tui, &mut server, /*cwd*/ None)
        .await?;
    let first = app.chat_widget.thread_id().unwrap();
    app.chat_widget.insert_str("Keep this unsent draft");
    app.new_agents_overview_session(&mut tui, &mut server, /*cwd*/ None)
        .await?;
    let other = app.chat_widget.thread_id().unwrap();
    app.select_agents_overview_thread(&mut tui, &mut server, first)
        .await?;
    assert_eq!(app.chat_widget.thread_id(), Some(first));
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "Keep this unsent draft"
    );
    insta::assert_snapshot!(
        crate::chatwidget::tests::helpers::render_bottom_popup(&app.chat_widget, /*width*/ 80)
            .lines().next().unwrap(), @"› Keep this unsent draft");
    // Seed persisted history without sending the user's draft. Subsequent navigation
    // must exercise thread/resume, including its restoration of permission settings.
    for id in [first, other] {
        server.thread_inject_items(id, vec![serde_json::from_value(serde_json::json!({
            "type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "saved history"}]
        }))?]).await?;
        app.agents_overview.blank_sessions.remove(&id);
    }
    for profile_id in [":read-only", ":workspace"] {
        app.select_agents_overview_thread(&mut tui, &mut server, first)
            .await?;
        assert_eq!(app.chat_widget.thread_id(), Some(first));
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "Keep this unsent draft"
        );
        app.select_permission_profile(
            &mut server,
            PermissionProfileSelection {
                profile_id: profile_id.into(),
                approval_policy: Some(AskForApproval::OnRequest),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                display_label: profile_id.into(),
            },
        )
        .await;
        while let Ok(event) = events.try_recv() {
            if matches!(event, AppEvent::CodexOp(_)) {
                Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
            }
        }
        // Test both immediate creation and creation after A -> B -> A.
        for switch in [false, true] {
            if switch {
                for target in [other, first] {
                    app.select_agents_overview_thread(&mut tui, &mut server, target)
                        .await?;
                    assert_eq!(app.chat_widget.thread_id(), Some(target));
                }
            }
            app.new_agents_overview_session(&mut tui, &mut server, /*cwd*/ None)
                .await?;
            let starts = recorded_params(&requests, "thread/start");
            let params = starts.last().unwrap();
            assert_eq!(
                (&params["permissions"], &params["approvalsReviewer"]),
                (&serde_json::json!(profile_id), &serde_json::json!("user"))
            );
            assert_eq!(
                (
                    app.chat_widget
                        .config_ref()
                        .permissions
                        .active_permission_profile(),
                    app.chat_widget.config_ref().approvals_reviewer
                ),
                (
                    Some(ActivePermissionProfile::new(profile_id)),
                    ApprovalsReviewer::User
                ),
            );
        }
    }
    app.select_agents_overview_thread(&mut tui, &mut server, first)
        .await?;
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "Keep this unsent draft"
    );
    assert!(recorded_params(&requests, "turn/start").is_empty());
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn command_center_new_checkout_and_worktree_preserve_source_and_default_branch() -> Result<()>
{
    use codex_worktree::CreateWorktree;
    use codex_worktree::WorktreeManager;
    use codex_worktree::WorktreeSettings;
    for (remote_only, stale_local) in [(false, false), (true, false), (false, true)] {
        let directory = tempdir()?;
        let source = directory.path().join("project");
        std::fs::create_dir_all(source.join("subdir"))?;
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .args(["-c", "commit.gpgSign=false"])
                .args(args)
                .current_dir(&source)
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "-b", "main"]);
        std::fs::write(source.join("subdir/file"), "default branch")?;
        git(&["add", "."]);
        git(&["commit", "-m", "default"]);
        if remote_only {
            git(&["update-ref", "refs/remotes/origin/release/default", "HEAD"]);
            git(&[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/release/default",
            ]);
        }
        git(&["checkout", "-b", "feature"]);
        std::fs::write(source.join("subdir/file"), "feature commit")?;
        git(&["commit", "-am", "feature"]);
        if remote_only {
            git(&["branch", "-D", "main"]);
        }
        if stale_local {
            git(&["update-ref", "refs/remotes/origin/main", "main"]);
            git(&[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ]);
            git(&["branch", "-f", "main", "feature"]);
        }
        let home = directory.path().join("home");
        std::fs::create_dir(&home)?;
        std::fs::write(
            home.join("config.toml"),
            format!(
                "approvals_reviewer = \"auto_review\"\nsandbox_mode = \"workspace-write\"\napproval_policy = \"on-request\"\nwindows.sandbox = \"unelevated\"\n[projects.{:?}]\ntrust_level = \"trusted\"\n",
                source.canonicalize()?.display().to_string(),
            ),
        )?;
        let manager =
            WorktreeManager::new(WorktreeSettings::for_cli(&home, /*desktop*/ None).unwrap());
        let selected = manager
            .create(&CreateWorktree {
                source_cwd: source.join("subdir"),
                base: Some("feature".into()),
            })
            .unwrap();
        manager
            .bind_thread(&selected.root, "existing-owner")
            .unwrap();
        std::fs::write(selected.cwd.join("file"), "dirty edit")?;
        std::fs::write(selected.cwd.join("untracked"), "untracked")?;
        for receiver_closed in [true, false] {
            let unclaimed = manager
                .create(&CreateWorktree {
                    source_cwd: source.join("subdir"),
                    base: Some("feature".into()),
                })
                .unwrap();
            let root = unclaimed.root.clone();
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let receiver = (!receiver_closed).then_some(rx);
            AppEventSender::new(tx).send(AppEvent::AgentsOverviewWorktreeCreated(Ok(
                PendingWorktree {
                    manager: manager.clone(),
                    checkout: Some(unclaimed),
                },
            )));
            drop(receiver);
            assert!(!root.exists());
            assert_eq!(manager.list(&source).unwrap().len(), 1);
        }
        let (mut app, mut events, _) = make_test_app_with_channels().await;
        app.config.codex_home = home.clone().abs();
        app.config.cwd = selected.cwd.clone().abs();
        app.chat_widget.windows_sandbox_local_server = cfg!(target_os = "windows");
        app.harness_overrides.cwd = Some(selected.cwd.clone());
        app.cli_kv_overrides
            .push(("features.worktrees".into(), TomlValue::Boolean(true)));
        app.config.features.enable(Feature::Worktrees)?;
        trust_launch_folder(&mut app);
        let (mut server, requests, proxy) = start_recording_app_server_with_history(
            &app.config,
            HistoryCapabilities::Current,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            crate::app_server_session::ThreadParamsMode::Embedded,
            LoaderOverrides::default(),
        )
        .await?;
        let mut tui = make_test_tui()?;
        let new_session = app.new_agents_overview_session(
            &mut tui,
            &mut server,
            Some(selected.cwd.clone().abs()),
        );
        let size = std::mem::size_of_val(&new_session);
        assert!(
            size < 64 * 1024,
            "new-session wrapper future is {size} bytes"
        );
        new_session.await?;
        let first = app.chat_widget.thread_id().expect("new checkout session");
        assert!(!crate::session_resume::cwds_differ(
            app.chat_widget.config_ref().cwd.as_path(),
            &selected.cwd
        ));
        assert_eq!(
            manager.owner(&selected.root).unwrap(),
            Some("existing-owner".into())
        );
        // Exercise the menu's ordered events, not the separate profile-selection API.
        app.chat_widget
            .set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
        Box::pin(app.handle_event(&mut tui, &mut server, AppEvent::OpenPermissionsPopup)).await?;
        app.chat_widget.handle_key_event(KeyCode::Up.into());
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        while let Ok(event) = events.try_recv() {
            Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
        }
        assert_eq!(
            app.chat_widget.config_ref().approvals_reviewer,
            ApprovalsReviewer::User
        );
        app.new_agents_overview_worktree(&mut tui, &mut server, Some(selected.cwd.clone().abs()))
            .await;
        assert!(app.pending_managed_worktree_creation);
        let result = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let Some(AppEvent::AgentsOverviewWorktreeCreated(result)) = events.recv().await {
                    break result;
                }
            }
        })
        .await?
        .map_err(|message| color_eyre::eyre::eyre!(message))?;
        let checkout = result.checkout.as_ref().unwrap().clone();
        assert_eq!(
            std::fs::read_to_string(checkout.cwd.join("file"))?,
            "default branch"
        );
        assert!(!checkout.cwd.join("untracked").exists());
        assert_eq!(checkout.cwd, checkout.root.join("subdir"));
        Box::pin(app.handle_event(
            &mut tui,
            &mut server,
            AppEvent::AgentsOverviewWorktreeCreated(Ok(result)),
        ))
        .await?;
        let second = app.chat_widget.thread_id().expect("new worktree session");
        assert_ne!(first, second);
        assert_eq!(
            recorded_params(&requests, "thread/start").last().unwrap()["approvalsReviewer"],
            serde_json::json!("user")
        );
        assert_eq!(
            app.chat_widget.config_ref().approvals_reviewer,
            ApprovalsReviewer::User
        );
        assert_eq!(
            manager.owner(&checkout.root).unwrap(),
            Some(second.to_string())
        );
        assert_eq!(recorded_params(&requests, "turn/start").len(), 0);
        assert_eq!(
            std::fs::read_to_string(selected.cwd.join("file"))?,
            "dirty edit"
        );
        server.shutdown().await?;
        proxy.await??;
        if !remote_only && !stale_local {
            let unused = manager
                .create(&CreateWorktree {
                    source_cwd: source.join("subdir"),
                    base: Some("main".into()),
                })
                .unwrap();
            let (mut failed_server, _, failed_proxy) = start_recording_app_server_with_history(
                &app.config,
                HistoryCapabilities::ThreadStartFails,
                /*blocked_thread_list*/ None,
                /*failed_thread_name*/ None,
                crate::app_server_session::ThreadParamsMode::Embedded,
                LoaderOverrides::default(),
            )
            .await?;
            let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
            app.chat_widget.show_bottom_pane_view(Box::new(view));
            app.start_agents_overview_session(
                &mut tui,
                &mut failed_server,
                Some(unused.cwd.clone().abs()),
                Some((manager.clone(), unused.clone())),
            )
            .await?;
            assert_eq!(app.chat_widget.thread_id(), Some(second));
            assert_eq!(manager.owner(&unused.root).unwrap(), None);
            assert!(unused.root.exists());
            let rendered = crate::chatwidget::tests::helpers::render_bottom_popup(
                &app.chat_widget,
                /*width*/ 500,
            );
            insta::assert_snapshot!(
                "command_center_retained_worktree_error",
                rendered.replace(&unused.root.display().to_string(), "<worktree>")
            );
            failed_server.shutdown().await?;
            failed_proxy.await??;
        }
    }
    Ok(())
}
