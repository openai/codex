//! Request-level coverage for background-task server defaults and dispatch recovery.

use super::*;
use crate::app::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use crate::app::tests::session_lifecycle_requests::HistoryCapabilities;
use crate::app::tests::session_lifecycle_requests::recorded_params;
use crate::app::tests::session_lifecycle_requests::start_recording_app_server_with_history;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::LocalImageAttachment;
use crate::chatwidget::UserMessage;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::model_catalog::ModelCatalog;
use crate::test_support::PathBufExt;
use crate::tui::test_support::make_test_tui;
use codex_app_server_protocol::UserInput;
use codex_protocol::openai_models::InputModality;
use codex_state::SqliteConfig;
use crossterm::event::KeyCode;
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
async fn background_task_sends_pasted_image_with_first_prompt() -> Result<()> {
    let mut tui = make_test_tui()?;
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    trust_launch_folder(&mut app);
    let image_dir = tempdir()?;
    let image_path = image_dir.path().join("pasted.png");
    image::RgbImage::new(1, 1).save(&image_path)?;
    let pasted_path = pathdiff::diff_paths(&image_path, std::env::current_dir()?)
        .unwrap_or_else(|| image_path.clone());
    let mut view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    assert!(view.handle_paste(pasted_path.to_string_lossy().into_owned()));
    assert!(view.handle_paste("Describe this".into()));
    let rendered_view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget
        .show_bottom_pane_view(Box::new(rendered_view));
    insta::assert_snapshot!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
            .lines()
            .find(|line| line.contains("[Image #1]"))
            .expect("image attachment visible"),
        @"› [Image #1] Describe this"
    );
    view.handle_key_event(KeyCode::Enter.into());
    let prompt = match events.try_recv()? {
        AppEvent::DispatchAgentsOverviewTask { prompt, .. } => prompt,
        event => panic!("expected task dispatch, got {event:?}"),
    };
    assert_eq!(prompt.local_images[0].path, pasted_path);
    assert_eq!(
        prompt.text_elements[0].placeholder(&prompt.text),
        Some("[Image #1]")
    );

    let (mut server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::Current,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;
    app.dispatch_agents_overview_task(&mut tui, &mut server, prompt.clone(), /*cwd*/ None)
        .await;
    let turns = recorded_params(&requests, "turn/start");
    assert_eq!(turns.len(), 1);
    assert_eq!(
        turns[0]["input"],
        serde_json::to_value(vec![
            UserInput::LocalImage {
                path: std::path::absolute(&pasted_path)?,
                detail: None
            },
            UserInput::Text {
                text: prompt.text.clone(),
                text_elements: prompt
                    .text_elements
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect(),
            },
        ])?
    );

    app.cli_kv_overrides = vec![("model".into(), toml::Value::Integer(1))];
    app.dispatch_agents_overview_task(
        &mut tui,
        &mut server,
        prompt.clone(),
        Some(app.config.cwd.clone()),
    )
    .await;
    let restored = app
        .agents_overview
        .view_state
        .lock()
        .unwrap()
        .composer
        .as_ref()
        .unwrap()
        .draft_snapshot();
    assert_eq!(
        (restored.text, restored.text_elements, restored.local_images),
        (
            prompt.text.clone(),
            prompt.text_elements.clone(),
            prompt.local_images.clone()
        )
    );
    view.handle_key_event(KeyCode::Enter.into());
    assert!(view.handle_paste("A newer draft".into()));
    app.dispatch_agents_overview_task(
        &mut tui,
        &mut server,
        prompt.clone(),
        Some(app.config.cwd.clone()),
    )
    .await;
    let current = app
        .agents_overview
        .view_state
        .lock()
        .unwrap()
        .composer
        .as_ref()
        .unwrap()
        .draft_snapshot();
    assert_eq!(current.text, "A newer draft");
    let notice = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                Some(lines_to_single_string(&cell.display_lines(/*width*/ 200)))
            }
            _ => None,
        })
        .find(|message| message.contains("Reattach images from:"))
        .expect("attachment recovery notice");
    assert!(notice.contains(&pasted_path.display().to_string()));
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn remote_background_task_sends_clipboard_image_bytes() -> Result<()> {
    let mut tui = make_test_tui()?;
    let mut app = make_test_app_with_channels().await.0;
    trust_launch_folder(&mut app);
    let image_dir = tempdir()?;
    let image_path = image_dir.path().join("pasted.png");
    image::RgbImage::new(1, 1).save(&image_path)?;
    let mut prompt = UserMessage::from("Describe this image");
    prompt.local_images.push(LocalImageAttachment {
        placeholder: "[Image #1]".into(),
        path: image_path,
    });
    let (mut server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::Current,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Remote,
        LoaderOverrides::default(),
    )
    .await?;
    app.dispatch_agents_overview_task(&mut tui, &mut server, prompt, /*cwd*/ None)
        .await;
    let turns = recorded_params(&requests, "turn/start");
    assert_eq!(turns.len(), 1);
    let image = &turns[0]["input"][0];
    assert_eq!(image["type"], "image");
    assert!(
        image["url"]
            .as_str()
            .is_some_and(|url| url.starts_with("data:image/png;base64,"))
    );
    assert_eq!(turns[0]["input"][1]["text"], "Describe this image");
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn background_task_rejects_images_for_text_only_model() -> Result<()> {
    let mut tui = make_test_tui()?;
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    trust_launch_folder(&mut app);
    let mut preset = app.model_catalog.models[0].clone();
    preset.model = "text-only-test-model".into();
    preset.input_modalities = vec![InputModality::Text];
    app.model_catalog = std::sync::Arc::new(ModelCatalog::new(vec![preset]));
    app.harness_overrides.model = Some("text-only-test-model".into());
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let mut prompt = UserMessage::from("[Image #1] Describe this");
    prompt.local_images.push(LocalImageAttachment {
        placeholder: "[Image #1]".into(),
        path: "/tmp/test-pasted.png".into(),
    });
    let (mut server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::Current,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;
    app.dispatch_agents_overview_task(&mut tui, &mut server, prompt.clone(), /*cwd*/ None)
        .await;
    assert!(recorded_params(&requests, "thread/start").is_empty());
    let draft = app
        .agents_overview
        .view_state
        .lock()
        .unwrap()
        .composer
        .as_ref()
        .unwrap()
        .draft_snapshot();
    assert_eq!(draft.local_images, prompt.local_images);
    let error = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                Some(lines_to_single_string(&cell.display_lines(/*width*/ 100)))
            }
            _ => None,
        })
        .find(|message| message.contains("does not support image inputs"))
        .expect("visible model rejection");
    insta::assert_snapshot!(error, @"■ Model text-only-test-model does not support image inputs. Remove images or switch models.");
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn background_task_reads_server_defaults_for_actual_destination() -> Result<()> {
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
        app.dispatch_agents_overview_task(
            &mut tui,
            &mut server,
            "background prompt".into(),
            explicit_cwd.then(|| destination.path().to_path_buf().abs()),
        )
        .await;
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
        assert_eq!(recorded_params(&requests, "turn/start").len(), 1);
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn background_task_preserves_explicit_choices_and_managed_defaults() -> Result<()> {
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
        app.dispatch_agents_overview_task(
            &mut tui,
            &mut server,
            "background prompt".into(),
            /*cwd*/ None,
        )
        .await;
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
            1,
            "{choice}"
        );
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn background_task_read_failure_keeps_prompt_and_does_not_start() -> Result<()> {
    let mut tui = make_test_tui()?;
    for capability in [
        HistoryCapabilities::ConfigReadFails,
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
        app.dispatch_agents_overview_task(
            &mut tui,
            &mut server,
            "retry background prompt".into(),
            /*cwd*/ None,
        )
        .await;
        let failed = capability == HistoryCapabilities::ConfigReadFails;
        assert_eq!(recorded_params(&requests, "config/read").len(), 1);
        assert_eq!(
            recorded_params(&requests, "thread/start").len(),
            usize::from(!failed)
        );
        assert_eq!(
            recorded_params(&requests, "turn/start").len(),
            usize::from(!failed)
        );
        if failed {
            assert!(app.agents_overview.dispatched_requests.is_empty());
            assert_eq!(
                app.agents_overview
                    .view_state
                    .lock()
                    .unwrap()
                    .composer
                    .as_ref()
                    .unwrap()
                    .current_text_with_pending(),
                "retry background prompt"
            );
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
                .find(|message| message.contains("Failed to load background task settings"))
                .expect("visible read error");
            insta::assert_snapshot!(error, @"■ Failed to load background task settings: config/read failed in TUI");
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
