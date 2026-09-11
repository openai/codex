//! Request-level coverage for the fresh-startup server defaults overlay.

use super::*;
use crate::app::startup::prepare_fresh_startup_config;
use crate::app::startup::startup_model;
use pretty_assertions::assert_eq;

async fn run_startup_for_test(
    tui: &mut crate::tui::Tui,
    server: AppServerSession,
    config: Config,
    bootstrap: AppServerBootstrap,
    selection: SessionSelection,
) -> Result<AppExitInfo> {
    App::run(
        tui,
        server,
        config.clone(),
        config.cwd.to_path_buf(),
        Vec::new(),
        ConfigOverrides::default(),
        LoaderOverrides::default(),
        CloudConfigBundleLoader::default(),
        /*initial_prompt*/ None,
        Vec::new(),
        selection,
        codex_feedback::CodexFeedback::new(),
        /*is_first_run*/ false,
        /*should_prompt_windows_sandbox_nux_at_startup*/ false,
        AppServerTarget::Embedded,
        /*state_db*/ None,
        Arc::new(EnvironmentManager::default_for_tests()),
        Duration::ZERO,
        Some(bootstrap),
        /*startup_hooks_browser*/ None,
        crate::startup_draft::tests::quiet_startup_test_pump(),
        /*managed_worktree*/ None,
    )
    .await
}

#[tokio::test]
async fn cli_fork_omits_implicit_model_and_effort() -> Result<()> {
    let home = tempdir()?;
    std::fs::write(
        home.path().join("config.toml"),
        "model = \"gpt-5.5\"\nmodel_reasoning_effort = \"low\"\nfeatures.fast_mode = true\n",
    )?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    let source = ThreadId::from_string(
        &app_test_support::create_fake_rollout(
            home.path(),
            "2026-01-01T00-00-00",
            "2026-01-01T00:00:00Z",
            "source prompt",
            Some(config.model_provider_id.as_str()),
            /*git_info*/ None,
        )
        .expect("create source rollout"),
    )?;
    let (mut server, requests, proxy) = start_recording_remote_app_server(&config).await?;
    let mut bootstrap = server.bootstrap(&config).await?;
    let mut target_preset = bootstrap
        .available_models
        .iter()
        .find(|preset| preset.show_in_picker)
        .expect("visible migration target")
        .clone();
    target_preset.id = "server-model".into();
    target_preset.model = "server-model".into();
    bootstrap.available_models.push(target_preset);
    let client_preset = bootstrap
        .available_models
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.5")
        .expect("client model in catalog");
    client_preset.upgrade = Some(codex_protocol::openai_models::ModelUpgrade {
        id: "server-model".into(),
        migration_config_key: "test-fork-migration".into(),
        model_link: None,
        upgrade_copy: None,
        migration_markdown: None,
        retirement_at: None,
    });
    assert!(
        crate::app::startup_prompts::should_show_model_migration_prompt(
            "gpt-5.5",
            "server-model",
            &Default::default(),
            &bootstrap.available_models,
        )
    );
    std::fs::write(
        home.path().join("config.toml"),
        "model = \"server-model\"\nmodel_reasoning_effort = \"high\"\n",
    )?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut run = Box::pin(run_startup_for_test(
        &mut tui,
        server,
        config,
        bootstrap,
        SessionSelection::Fork(crate::resume_picker::SessionTarget {
            path: None,
            thread_id: source,
            cwd: None,
            history_mode: None,
        }),
    ));
    tokio::time::timeout(Duration::from_secs(/*secs*/ 15), async {
        loop {
            if !recorded_params(&requests, "thread/fork").is_empty() {
                return Ok::<(), color_eyre::eyre::Report>(());
            }
            tokio::select! {
                result = &mut run => {
                    result?;
                    return Err(color_eyre::eyre::eyre!("startup exited before thread/fork"));
                }
                () = tokio::time::sleep(Duration::from_millis(/*millis*/ 20)) => {}
            }
        }
    })
    .await??;
    let fork = recorded_params(&requests, "thread/fork");
    assert_eq!(fork.len(), 1);
    assert_eq!(fork[0]["model"], serde_json::Value::Null);
    assert!(fork[0]["config"].get("model_reasoning_effort").is_none());
    assert_eq!(fork[0]["serviceTier"], serde_json::Value::Null);
    assert!(recorded_params(&requests, "config/read").is_empty());
    drop(run);
    proxy.abort();
    Ok(())
}

async fn run_until_thread_start(
    server: AppServerSession,
    config: Config,
    bootstrap: AppServerBootstrap,
    requests: &RecordedRequests,
) -> Result<()> {
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.pause_events();
    let mut run = Box::pin(run_startup_for_test(
        &mut tui,
        server,
        config,
        bootstrap,
        SessionSelection::StartFresh,
    ));
    tokio::time::timeout(Duration::from_secs(/*secs*/ 15), async {
        loop {
            if !recorded_params(requests, "thread/start").is_empty() {
                return Ok::<(), color_eyre::eyre::Report>(());
            }
            tokio::select! {
                result = &mut run => {
                    result?;
                    return Err(color_eyre::eyre::eyre!("startup exited before thread/start"));
                }
                () = tokio::time::sleep(Duration::from_millis(/*millis*/ 20)) => {}
            }
        }
    })
    .await??;
    Ok(())
}

#[tokio::test]
async fn fresh_startup_uses_server_defaults_with_explicit_and_managed_precedence() -> Result<()> {
    for (choice, managed, expected_model, expected_effort) in [
        ("saved", false, "server-model", "high"),
        ("cli_model", true, "cli-model", "high"),
        ("cli_effort", true, "server-model", "low"),
        ("profile_model", false, "profile-model", "high"),
        ("profile_effort", false, "server-model", "low"),
        ("profile_model", true, "profile-model", "high"),
        ("profile_effort", true, "server-model", "low"),
        ("managed", true, "managed-model", "medium"),
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
        if managed {
            std::fs::write(
                server_home.path().join("requirements.toml"),
                "[models.new_thread]\nmodel = \"managed-model\"\nmodel_reasoning_effort = \"medium\"\n",
            )?;
        }
        let mut harness_overrides = ConfigOverrides::default();
        let mut cli_kv_overrides = Vec::new();
        let mut loader_overrides = LoaderOverrides::without_managed_config_for_tests();
        match choice {
            "cli_model" => harness_overrides.model = Some("cli-model".to_string()),
            "cli_effort" => cli_kv_overrides.push((
                "model_reasoning_effort".to_string(),
                TomlValue::String("low".to_string()),
            )),
            "profile_model" | "profile_effort" => {
                let path = client_home.path().join("work.config.toml");
                std::fs::write(
                    &path,
                    if choice == "profile_model" {
                        "model = \"profile-model\"\n"
                    } else {
                        "model_reasoning_effort = \"low\"\n"
                    },
                )?;
                loader_overrides.user_config_path = Some(path.abs());
                loader_overrides.user_config_profile = Some("work".parse()?);
            }
            _ => {}
        }
        let mut config = ConfigBuilder::default()
            .codex_home(client_home.path().to_path_buf())
            .loader_overrides(loader_overrides)
            .cli_overrides(cli_kv_overrides.clone())
            .harness_overrides(harness_overrides.clone())
            .build()
            .await?;
        let mut server_config = config.clone();
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
        server = server.with_remote_cwd_override(Some(server_config.cwd.to_path_buf()));
        let bootstrap = server.bootstrap(&config).await?;
        assert!(
            prepare_fresh_startup_config(
                &mut config,
                &server,
                &cli_kv_overrides,
                &harness_overrides,
            )
            .await?
        );
        let selected_model = startup_model(&config, &bootstrap, /*server_defaults_read*/ true);
        let started = crate::app_server_session::start_thread_with_request_handle(
            server.request_handle(),
            &crate::local_settings::LocalSettings::from(&config),
            config,
            server.thread_params_mode(),
            server.remote_cwd_override().map(Path::to_path_buf),
            server.thread_tool_transport(),
        )
        .await?;
        assert_eq!(selected_model, expected_model, "{choice}");
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
        assert_eq!(started.session.model, expected_model, "{choice}");
        assert_eq!(
            recorded_params(&requests, "config/read"),
            vec![serde_json::json!({"cwd": server_config.cwd.display().to_string()})],
        );
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn fresh_startup_reads_destination_and_cleared_model_uses_catalog() -> Result<()> {
    for (remote, override_cwd) in [(false, false), (true, false), (true, true)] {
        let client_home = tempdir()?;
        let server_home = tempdir()?;
        let destination = tempdir()?;
        let launch_cwd = tempdir()?;
        std::fs::write(
            client_home.path().join("config.toml"),
            "model = \"stale-client-model\"\nmodel_reasoning_effort = \"low\"\n",
        )?;
        std::fs::write(
            server_home.path().join("config.toml"),
            "model_reasoning_effort = \"high\"\n",
        )?;
        let mut config = ConfigBuilder::default()
            .codex_home(client_home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .harness_overrides(ConfigOverrides {
                cwd: Some(destination.path().to_path_buf()),
                ..Default::default()
            })
            .build()
            .await?;
        let mut server_config = config.clone();
        server_config.codex_home = server_home.path().to_path_buf().abs();
        server_config.sqlite = SqliteConfig::new_for_testing(server_home.path().abs());
        let mode = if remote {
            crate::app_server_session::ThreadParamsMode::Remote
        } else {
            crate::app_server_session::ThreadParamsMode::Embedded
        };
        let (mut server, requests, proxy) = start_recording_app_server_with_history(
            &server_config,
            HistoryCapabilities::Current,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            mode,
            LoaderOverrides {
                user_config_path: Some(server_home.path().join("config.toml").abs()),
                ..LoaderOverrides::default()
            },
        )
        .await?;
        if override_cwd {
            server = server.with_remote_cwd_override(Some(launch_cwd.path().to_path_buf()));
        }
        let bootstrap = server.bootstrap(&config).await?;
        assert_eq!(bootstrap.default_model, "stale-client-model");
        let defaults_read =
            prepare_fresh_startup_config(&mut config, &server, &[], &ConfigOverrides::default())
                .await?;
        assert!(defaults_read);
        assert_eq!(config.model, None);
        let selected_model = startup_model(&config, &bootstrap, defaults_read);
        assert_ne!(selected_model, "stale-client-model");
        let started = crate::app_server_session::start_thread_with_request_handle(
            server.request_handle(),
            &crate::local_settings::LocalSettings::from(&config),
            config,
            server.thread_params_mode(),
            server.remote_cwd_override().map(Path::to_path_buf),
            server.thread_tool_transport(),
        )
        .await?;
        assert_eq!(started.session.model, selected_model);
        let starts = recorded_params(&requests, "thread/start");
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0]["model"], serde_json::Value::Null);
        assert_eq!(starts[0]["config"]["model_reasoning_effort"], "high");
        let (mut app, _, _) = make_test_app_with_channels().await;
        app.chat_widget.handle_thread_session_quiet(started.session);
        if !remote {
            let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 80)
                .replace(&destination.path().display().to_string(), "<PROJECT>");
            insta::assert_snapshot!(rendered, @r"
            › Ask Codex to do anything

              gpt-6-astra high · <PROJECT>
            ");
        }
        let expected_cwd = if override_cwd {
            launch_cwd.path().display().to_string()
        } else if !remote {
            destination.path().display().to_string()
        } else {
            ".".to_string()
        };
        assert_eq!(
            recorded_params(&requests, "config/read"),
            vec![serde_json::json!({"cwd": expected_cwd})]
        );
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn fresh_startup_falls_back_only_for_unsupported_config_read() -> Result<()> {
    for capabilities in [
        HistoryCapabilities::ConfigReadUnsupported(-32601),
        HistoryCapabilities::ConfigReadUnsupported(-32600),
    ] {
        let home = tempdir()?;
        std::fs::write(
            home.path().join("config.toml"),
            "model = \"client-model\"\nmodel_reasoning_effort = \"low\"\n",
        )?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await?;
        let (mut server, requests, proxy) = start_recording_app_server_with_history(
            &config,
            capabilities,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            crate::app_server_session::ThreadParamsMode::Embedded,
            LoaderOverrides::default(),
        )
        .await?;
        let bootstrap = server.bootstrap(&config).await?;
        run_until_thread_start(server, config, bootstrap, &requests).await?;
        let starts = recorded_params(&requests, "thread/start");
        assert_eq!(starts.len(), 1);
        assert_eq!(
            (
                &starts[0]["model"],
                &starts[0]["config"]["model_reasoning_effort"]
            ),
            (
                &serde_json::json!("client-model"),
                &serde_json::json!("low")
            )
        );
        tokio::time::timeout(Duration::from_secs(/*secs*/ 15), proxy).await???;
    }
    Ok(())
}

#[tokio::test]
async fn startup_reads_server_defaults_before_starting_thread() -> Result<()> {
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
    let config = ConfigBuilder::default()
        .codex_home(client_home.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    let mut server_config = config.clone();
    server_config.codex_home = server_home.path().to_path_buf().abs();
    server_config.sqlite = SqliteConfig::new_for_testing(server_home.path().abs());
    let (mut server, requests, proxy) = start_recording_app_server_with_history(
        &server_config,
        HistoryCapabilities::Current,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides {
            user_config_path: Some(server_home.path().join("config.toml").abs()),
            ..LoaderOverrides::default()
        },
    )
    .await?;
    let bootstrap = server.bootstrap(&config).await?;
    run_until_thread_start(server, config, bootstrap, &requests).await?;
    {
        let recorded = requests.lock().expect("request recorder lock");
        let read = recorded
            .iter()
            .position(|request| request.method == "config/read")
            .expect("startup must read server defaults");
        let start = recorded
            .iter()
            .position(|request| request.method == "thread/start")
            .expect("startup must start a thread");
        assert!(read < start);
    }
    let starts = recorded_params(&requests, "thread/start");
    assert_eq!(starts.len(), 1);
    assert_eq!(
        (
            &starts[0]["model"],
            &starts[0]["config"]["model_reasoning_effort"]
        ),
        (
            &serde_json::json!("server-model"),
            &serde_json::json!("high")
        ),
    );
    tokio::time::timeout(Duration::from_secs(/*secs*/ 15), proxy).await???;
    Ok(())
}

#[tokio::test]
async fn startup_read_failure_exits_before_thread_creation() -> Result<()> {
    let (app, _, _) = make_test_app_with_channels().await;
    let home = tempdir()?;
    let mut config = app.config.clone();
    config.codex_home = home.path().to_path_buf().abs();
    config.sqlite = SqliteConfig::new_for_testing(home.path().abs());
    let (mut server, requests, proxy) = start_recording_app_server_with_history(
        &config,
        HistoryCapabilities::ConfigReadFails,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;
    let bootstrap = server.bootstrap(&config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let result = run_startup_for_test(
        &mut tui,
        server,
        config,
        bootstrap,
        SessionSelection::StartFresh,
    )
    .await;
    insta::assert_snapshot!(result.expect_err("startup read must fail").to_string(), @"config/read failed in TUI");
    assert_eq!(recorded_params(&requests, "config/read").len(), 1);
    assert!(recorded_params(&requests, "thread/start").is_empty());
    proxy.await??;
    Ok(())
}
