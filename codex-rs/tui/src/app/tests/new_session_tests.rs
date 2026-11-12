//! Replacement starts use server defaults and preserve the current task on failure.

use super::*;
use crate::app::backend_sessions::BackendSessions;
use crate::app_server_session::AppServerSessionEvent;
use codex_app_server_protocol::SkillMetadata;
use codex_app_server_protocol::SkillScope;
use codex_app_server_protocol::SkillsListEntry;
use codex_config::types::HistoryPersistence;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
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
            insta::assert_snapshot!(rendered, @"
            › Ask Codex to do anything

              server-model high · <PROJECT>
              ← for agents · ? for shortcuts
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_implementation_reads_approved_plan_file_in_fresh_session() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    // Match the canonical CODEX_HOME used at launch.
    app.config.codex_home = app.config.codex_home.canonicalize()?;
    let model = responses::start_mock_server().await;
    app_test_support::MockResponsesConfig::new(&model.uri())
        // Exercise the local executor's file read without requiring a platform sandbox helper.
        .with_sandbox_mode("danger-full-access")
        .with_root_config("instructions = 'Help the user.'")
        .write(app.config.codex_home.as_path())?;
    let workspace = tempdir()?;
    app.config = app
        .rebuild_config_for_cwd(workspace.path().canonicalize()?)
        .await?;
    let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let plan = format!(
        "# Plan\n{}\n- Verify the final step.\n",
        "- Update the parser.\n".repeat(3_000)
    );
    let source = server.start_thread(&app.config).await?;
    let source_id = source.session.thread_id;
    app.enqueue_primary_thread_session(source.session, source.turns)
        .await?;
    Box::pin(app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::ImplementPlanInFreshSession { plan: plan.clone() },
    ))
    .await?;
    let thread_id = app.chat_widget.thread_id().expect("fresh thread");
    assert_ne!(thread_id, source_id);
    let attachments = app.config.codex_home.join("attachments");
    let mut dirs = std::fs::read_dir(&attachments)?;
    let path = dirs
        .next()
        .expect("attachment directory")?
        .path()
        .join("approved-plan.md");
    assert!(dirs.next().is_none());
    assert_eq!(std::fs::read(&path)?, plan.as_bytes());
    let path = path.to_str().expect("UTF-8 attachment path");
    let command = if cfg!(windows) {
        format!(
            "Get-Content -LiteralPath '{}' -Tail 1",
            path.replace("'", "''")
        )
    } else {
        format!("tail -n 1 {}", shlex::try_quote(path)?)
    };
    let read_args = serde_json::json!({ "cmd": command });
    let response = responses::mount_sse_sequence(
        &model,
        vec![
            responses::sse(vec![
                responses::ev_response_created("read"),
                responses::ev_function_call("read_plan", "exec_command", &read_args.to_string()),
                responses::ev_completed("read"),
            ]),
            responses::sse_completed("implemented"),
        ],
    )
    .await;

    let mut user_rows = Vec::new();
    while let Ok(event) = events.try_recv() {
        match event {
            AppEvent::InsertHistoryCell(cell) if cell.as_any().is::<UserHistoryCell>() => {
                user_rows
                    .push(lines_to_single_string(&cell.raw_lines()).replace(path, "<PLAN_FILE>"));
            }
            event @ (AppEvent::SubmitThreadOp { .. } | AppEvent::CodexOp(_)) => {
                Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
            }
            _ => {}
        }
    }
    insta::assert_snapshot!(user_rows.join("\n"), @"Read the approved plan at <PLAN_FILE> and implement it.");
    super::safety_buffering::wait_for_turn_completed(&mut app, &mut server, thread_id).await;
    let requests = response.requests();
    let [initial, after_read] = requests.as_slice() else {
        panic!("expected a plan read and a follow-up request")
    };
    let output = after_read
        .function_call_output_text("read_plan")
        .expect("plan read result");
    assert!(output.contains("- Verify the final step."), "{output}");
    let mut body = initial.body_json();
    let prompt = body["input"].as_array_mut().unwrap().last_mut().unwrap();
    assert_eq!(prompt["role"], "user");
    let text = &mut prompt["content"][0]["text"];
    *text = text.as_str().unwrap().replace(path, "<PLAN_FILE>").into();
    insta::assert_snapshot!(
        "clear_context_plan_request",
        context_snapshot::format_context_snapshot(
            "Clear context and implement the approved plan in a fresh thread.",
            &[context_snapshot::SnapshotEntry::body(&body)],
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn plan_implementation_handles_unavailable_attachment() -> Result<()> {
    for (write_fails, oversized) in [(true, false), (false, false), (false, true)] {
        crate::boxed_future!(assert_plan_implementation_with_unavailable_attachment(
            write_fails,
            oversized,
        ))
        .await?;
    }
    Ok(())
}

async fn assert_plan_implementation_with_unavailable_attachment(
    write_fails: bool,
    oversized: bool,
) -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let model = responses::start_mock_server().await;
    let response = responses::mount_sse_once(&model, responses::sse_completed("implemented")).await;
    let attachments = app.config.codex_home.join("attachments");
    let config = app_test_support::MockResponsesConfig::new(&model.uri());
    let config = if write_fails {
        std::fs::write(&attachments, "")?;
        config.with_sandbox_mode("danger-full-access")
    } else {
        let denied_path = serde_json::to_string(attachments.as_path())?;
        config
            // The embedded test executor has no sandbox helper for AGENTS.md discovery.
            .with_root_config("default_permissions = 'no-attachments'\nproject_doc_max_bytes = 0")
            .with_extra_config(&format!(
                "[permissions.no-attachments.filesystem]\n\":root\" = 'read'\n{denied_path} = 'deny'"
            ))
    };
    config.write(app.config.codex_home.as_path())?;
    let workspace = tempdir()?;
    app.config = app
        .rebuild_config_for_cwd(workspace.path().canonicalize()?)
        .await?;
    let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let source = server.start_thread(&app.config).await?;
    let source_id = source.session.thread_id;
    app.enqueue_primary_thread_session(source.session, source.turns)
        .await?;
    let steps = if oversized { 3_000 } else { 1_000 };
    let plan = format!(
        "# Plan\n{}- Verify the final step.",
        "- Update the parser.\n".repeat(steps)
    );
    crate::boxed_future!(app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::ImplementPlanInFreshSession { plan: plan.clone() },
    ))
    .await?;

    if !write_fails {
        let path = std::fs::read_dir(&attachments)?
            .next()
            .expect("the app server saved the plan")?
            .path()
            .join("approved-plan.md");
        assert_eq!(std::fs::read_to_string(path)?, plan);
    }
    let thread_id = app.chat_widget.thread_id().expect("current thread");
    if oversized {
        assert_eq!(thread_id, source_id);
        assert!(response.requests().is_empty());
    } else {
        assert_ne!(thread_id, source_id);
        while let Ok(event) = events.try_recv() {
            if matches!(
                event,
                AppEvent::SubmitThreadOp { .. } | AppEvent::CodexOp(_)
            ) {
                crate::boxed_future!(app.handle_event(&mut tui, &mut server, event)).await?;
            }
        }
        super::safety_buffering::wait_for_turn_completed(&mut app, &mut server, thread_id).await;
        let request = response.single_request();
        let messages = request.inputs_of_type("message");
        let user = messages.last().expect("implementation prompt");
        assert_eq!(user["role"], "user");
        assert_eq!(user["content"][0]["text"], "Implement the plan.");
        let recovered = request
            .inputs_of_type("function_call_output")
            .iter()
            .filter(|item| item["name"] == "proposed_plan")
            .map(|item| item["output"].as_str().expect("plan chunk"))
            .collect::<String>();
        assert_eq!(recovered, plan);
    }
    server.shutdown().await?;
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
    let destination_slot = backends
        .slot(destination_id)
        .expect("registered destination");
    backends
        .parked_inputs_mut(destination_slot)
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
async fn backend_handoff_restores_a_live_blank_session_without_resuming_it() -> Result<()> {
    check_retained_draft_handoff(
        /*saved_history*/ false,
        HistoryCapabilities::Current,
        /*retained_blank*/ true,
    )
    .await
}

#[tokio::test]
async fn backend_handoff_restores_a_live_blank_session_on_an_older_server() -> Result<()> {
    let capabilities = HistoryCapabilities::LegacyOnly;
    check_retained_draft_handoff(
        /*saved_history*/ false,
        capabilities,
        /*retained_blank*/ true,
    )
    .await
}

#[tokio::test]
async fn backend_handoff_falls_back_when_the_first_turn_page_is_unsupported() -> Result<()> {
    check_retained_draft_handoff(
        /*saved_history*/ false,
        HistoryCapabilities::FirstTurnPageUnsupported,
        /*retained_blank*/ true,
    )
    .await
}

#[tokio::test]
async fn backend_handoff_reads_saved_turns_instead_of_reusing_a_stale_blank() -> Result<()> {
    check_retained_draft_handoff(
        /*saved_history*/ true,
        HistoryCapabilities::Current,
        /*retained_blank*/ true,
    )
    .await
}

#[tokio::test]
async fn backend_handoff_keeps_resumed_settings_for_populated_task_draft() -> Result<()> {
    check_retained_draft_handoff(
        /*saved_history*/ true,
        HistoryCapabilities::Current,
        /*retained_blank*/ false,
    )
    .await
}

#[tokio::test]
async fn same_backend_reopen_recovers_history_for_a_stale_blank() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let model = core_test_support::responses::start_mock_server().await;
    app_test_support::MockResponsesConfig::new(&model.uri()).write(&app.config.codex_home)?;
    crate::legacy_core::config::set_project_trust_level(
        &app.config.codex_home,
        app.config.cwd.as_path(),
        codex_protocol::config_types::TrustLevel::Trusted,
    )
    .map_err(std::io::Error::other)?;
    app.config = app
        .rebuild_config_for_cwd(app.config.cwd.to_path_buf())
        .await?;
    let mut server =
        boxed_future!(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let blank = server.start_thread(&app.config).await?;
    let thread_id = blank.session.thread_id;
    app.backend_registry
        .register_started(server.backend_id(), &blank)?;
    app.agents_overview.blank_sessions.insert(thread_id, blank);

    // Returning to the backend through another task leaves this cached blank untouched.
    let other = server.start_thread(&app.config).await?;
    let other_id = other.session.thread_id;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.enqueue_primary_thread_session(other.session, other.turns)
        .await?;
    drain_app_events(&mut app, &mut tui, &mut server, &mut events).await?;
    assert_eq!(app.current_displayed_thread_id(), Some(other_id));

    // The helper consumes the turn notifications without delivering them to App.
    let prompt = "Sent while the backend was parked";
    side_chat_policy_tests::ask(&mut server, &model, thread_id, prompt).await?;
    assert!(app.agents_overview.blank_sessions.contains_key(&thread_id));
    let mut expected_mode = app.chat_widget.effective_collaboration_mode();
    app.chat_widget.set_model("stale-model");
    app.chat_widget.apply_external_edit("saved draft".into());
    let draft = app.chat_widget.capture_thread_input_state().unwrap();
    app.agents_overview.input_states.insert(thread_id, draft);
    expected_mode.mode = ModeKind::Plan;
    expected_mode.settings.developer_instructions = Some("Updated by the other client".into());
    assert!(
        server
            .thread_settings_update(ThreadSettingsUpdateParams {
                thread_id: thread_id.to_string(),
                collaboration_mode: Some(expected_mode.clone()),
                ..Default::default()
            })
            .await?
    );
    let _ = next_thread_settings_updated(&mut server, thread_id).await;

    boxed_future!(app.select_agents_overview_thread(&mut tui, &mut server, thread_id)).await?;
    assert_eq!(app.current_displayed_thread_id(), Some(thread_id));
    let history = boxed_future!(active_reconnect::drain_history(
        &mut app,
        &mut tui,
        &mut server,
        &mut events
    ))
    .await?;
    assert!(history.contains(prompt), "{history}");
    assert_eq!(app.chat_widget.composer_text_with_pending(), "saved draft");
    assert_eq!(
        app.chat_widget.effective_collaboration_mode(),
        expected_mode
    );
    assert!(!app.agents_overview.blank_sessions.contains_key(&thread_id));
    server.shutdown().await?;
    Ok(())
}

async fn check_retained_draft_handoff(
    saved_history: bool,
    history_capabilities: HistoryCapabilities,
    retained_blank: bool,
) -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let source = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let (mut destination, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        history_capabilities,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::without_managed_config_for_tests(),
    )
    .await?;
    let destination_id = destination.backend_id();
    let mut blank = if saved_history {
        let timestamp = "2025-01-05T12-00-00";
        let thread_id = create_fake_rollout(
            &app.config.codex_home,
            timestamp,
            "2025-01-05T12:00:00Z",
            "Sent while the backend was parked",
            Some(&app.config.model_provider_id),
            /*git_info*/ None,
        )
        .expect("materialize the retained thread");
        let path = app_test_support::rollout_path(&app.config.codex_home, timestamp, &thread_id);
        let contents = std::fs::read_to_string(&path)?;
        let (metadata, history) = contents.split_once('\n').expect("rollout with history");
        let mut metadata: serde_json::Value = serde_json::from_str(metadata)?;
        metadata["payload"]["cwd"] = serde_json::to_value(&app.config.cwd)?;
        std::fs::write(path, format!("{metadata}\n{history}"))?;
        let thread_id = ThreadId::from_string(&thread_id)?;
        destination
            .resume_thread(
                &app.local_settings,
                app.config.clone(),
                thread_id,
                crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
            )
            .await?
    } else {
        destination.start_thread(&app.config).await?
    };
    let thread_id = blank.session.thread_id;
    let mut server_mode = blank.session.collaboration_mode.as_deref().cloned();
    let initial_policy = blank.session.approval_policy;
    blank.turns.clear();
    let resume_count = recorded_params(&requests, "thread/resume").len();
    let target = crate::resume_picker::SessionTarget {
        backend_id: destination_id,
        path: None,
        thread_id,
        cwd: Some(app.config.cwd.to_path_buf()),
        history_mode: None,
    };
    app.backend_registry
        .register_started(destination_id, &blank)?;
    if retained_blank {
        app.agents_overview.blank_sessions.insert(thread_id, blank);
    }
    app.chat_widget
        .restore_user_message_to_composer("draft for the retained task".into());
    let parked_policy = (!saved_history && history_capabilities == HistoryCapabilities::Current)
        .then_some(if initial_policy == AskForApproval::Never {
            AskForApproval::OnRequest
        } else {
            AskForApproval::Never
        });
    if saved_history {
        app.chat_widget.set_model("old-model");
    }
    if parked_policy.is_some() {
        server_mode = Some(app.chat_widget.effective_collaboration_mode());
    }
    let collaboration_mode = if saved_history || parked_policy.is_some() {
        let mode = server_mode.as_mut().expect("test has a mode to update");
        mode.mode = ModeKind::Plan;
        mode.settings.developer_instructions = Some("Instructions from the parked task".into());
        if parked_policy.is_some() {
            mode.settings.reasoning_effort = Some(ReasoningEffortConfig::High);
        }
        Some(mode.clone())
    } else {
        None
    };
    let retained = app.chat_widget.capture_thread_input_state().unwrap();
    app.chat_widget
        .restore_user_message_to_composer("draft for the displayed task".into());
    let mut backends = BackendSessions::from_initial(source, vec![destination]);
    let destination_slot = backends
        .slot(destination_id)
        .expect("registered destination");
    backends
        .parked_inputs_mut(destination_slot)
        .insert(thread_id, retained);
    let outgoing = backends.displayed_owner();

    if collaboration_mode.is_some() || parked_policy.is_some() {
        let destination = backends.session_at_mut(destination_slot);
        let settings = ThreadSettingsUpdateParams {
            thread_id: thread_id.to_string(),
            collaboration_mode,
            effort: parked_policy.map(|_| ReasoningEffortConfig::High),
            approval_policy: parked_policy,
            ..Default::default()
        };
        assert!(destination.thread_settings_update(settings).await?);
        let update = next_thread_settings_updated(destination, thread_id).await;
        if parked_policy.is_some() {
            let event = AppServerEvent::ServerNotification(Box::new(
                ServerNotification::ThreadSettingsUpdated(update),
            ));
            let event = AppServerSessionEvent {
                backend_id: destination_id,
                event,
            };
            let (destination, parked_inputs) = backends.session_and_parked_inputs(destination_slot);
            app.handle_polled_backend_event(destination, outgoing, parked_inputs, event)
                .await;
        } else if saved_history && retained_blank {
            // A formerly blank thread must ignore settings cached before it acquired history.
            let mut stale_update = update;
            stale_update.thread_settings.model = "old-model".into();
            backends
                .parked_inputs_mut(destination_slot)
                .get_mut(&thread_id)
                .unwrap()
                .pending_thread_settings = Some(stale_update);
        }
    }
    let settings = app.backend_settings();
    backends.settings.insert(destination_id, settings);
    if history_capabilities == HistoryCapabilities::LegacyOnly {
        app.app_server_target = AppServerTarget::Cloud;
        app.app_server_backend_id = Some(backends.displayed_owner());
        assert!(app.begin_reconnect());
    }

    app.agents_overview.initialized.insert(outgoing);
    app.agents_overview.show_more_requested = true;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let pages_before = recorded_params(&requests, "thread/turns/list").len();
    let reads_before = recorded_params(&requests, "thread/read").len();
    app.resume_backend_session(&mut tui, &mut backends, target)
        .await?;

    assert_eq!(
        backends.displayed_owner(),
        destination_id,
        "handoff errors: {:?}; reads: {:?}; pages: {:?}; resumes: {:?}",
        std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) =>
                    Some(lines_to_single_string(&cell.raw_lines())),
                _ => None,
            })
            .collect::<Vec<_>>(),
        recorded_params(&requests, "thread/read"),
        recorded_params(&requests, "thread/turns/list"),
        recorded_params(&requests, "thread/resume"),
    );
    assert!(app.agents_overview.initialized.contains(&outgoing));
    assert!(!app.agents_overview.initialized.contains(&destination_id));
    assert!(!app.agents_overview.show_more_requested);
    assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
    if saved_history || parked_policy.is_some() {
        assert_eq!(
            app.chat_widget.effective_collaboration_mode(),
            server_mode.unwrap()
        );
    }
    if let Some(parked_policy) = parked_policy {
        let policy = &app.chat_widget.config_ref().permissions.approval_policy;
        assert_eq!(AskForApproval::from(policy.value()), parked_policy);
    }
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "draft for the retained task"
    );
    let page_probe_count = recorded_params(&requests, "thread/turns/list")[pages_before..]
        .iter()
        .filter(|params| params["limit"] == 1)
        .count();
    // The saved-history case uses a legacy rollout even on the current server.
    assert_eq!(
        page_probe_count,
        usize::from(
            !saved_history
                && matches!(
                    history_capabilities,
                    HistoryCapabilities::Current | HistoryCapabilities::FirstTurnPageUnsupported
                )
        )
    );
    if retained_blank
        && (page_probe_count == 0
            || history_capabilities == HistoryCapabilities::FirstTurnPageUnsupported)
    {
        assert!(
            recorded_params(&requests, "thread/read")[reads_before..]
                .iter()
                .any(|params| params["includeTurns"] == true)
        );
    }
    assert_eq!(
        recorded_params(&requests, "thread/resume").len(),
        resume_count + usize::from(saved_history),
    );
    assert_eq!(
        app.agents_overview.blank_sessions.contains_key(&thread_id),
        !saved_history,
    );
    backends.shutdown().await?;
    proxy.await??;
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

#[tokio::test(start_paused = true)]
async fn cloud_switch_wait_keeps_draft_editable_without_submitting_to_local() -> Result<()> {
    use crate::app::cloud_switch_wait::ClipboardHandoff;
    use crate::app::cloud_switch_wait::wait_for_cloud_switch;

    let (mut app, mut app_events, mut operations) = make_test_app_with_channels().await;
    let session = test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf());
    app.chat_widget.handle_thread_session(session);
    app.chat_widget
        .set_new_backend_control(Some("Cloud".into()));
    app.chat_widget
        .apply_external_edit("/review first draft".into());
    app.keymap
        .composer
        .queue
        .push(crate::key_hint::plain(KeyCode::Char('q')));
    app.chat_widget
        .apply_keymap_update(codex_config::types::TuiKeymap::default(), &app.keymap);
    while operations.try_recv().is_ok() {}
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
    let late_input_at = std::cell::Cell::new(None);
    let input = [
        TuiEvent::Key(KeyCode::Enter.into()),
        TuiEvent::Key(KeyCode::Char('q').into()),
        TuiEvent::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT)),
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
        TuiEvent::Key(KeyCode::Char('b').into()),
        TuiEvent::Paste(" more".into()),
        TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
        TuiEvent::Key(KeyCode::Char('a').into()),
        TuiEvent::Key(KeyCode::Char('a').into()),
    ];
    let paste_keys = [
        KeyCode::Char('b'),
        KeyCode::Char('b'),
        KeyCode::Tab,
        KeyCode::Char('c'),
        KeyCode::Char('c'),
        KeyCode::Char('?'),
    ];
    let events = tokio_stream::iter(input)
        .chain(futures::stream::once(async move {
            finish_tx.send(()).expect("finish Cloud switch");
            TuiEvent::Draw
        }))
        .chain(futures::stream::once(async {
            tokio::time::sleep(std::time::Duration::from_millis(70)).await;
            TuiEvent::Draw
        }))
        .chain(futures::stream::once(async {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            late_input_at.set(Some(std::time::Instant::now()));
            TuiEvent::Key(KeyCode::Enter.into())
        }))
        .chain(tokio_stream::iter(
            paste_keys.map(|key| TuiEvent::Key(key.into())),
        ));
    let result = wait_for_cloud_switch(
        &mut app,
        &mut tui,
        async {
            finish_rx.await.expect("release Cloud switch");
            Err::<(), _>("Cloud unavailable")
        },
        Box::pin(events),
        ClipboardHandoff::SettleBeforeNextView,
    )
    .await;

    assert_eq!(result, Err("Cloud unavailable"));
    let rendered = buffer_text(tui.terminal.previous_buffer());
    insta::assert_snapshot!(
        rendered
            .lines()
            .find(|line| line.contains("Cloud conversation did not open"))
            .unwrap_or_default()
            .trim(),
        @"Cloud conversation did not open draft kept"
    );
    let draft = app.chat_widget.composer_draft_snapshot();
    assert_eq!(draft.text, "/review first draft more\naa\nbb\tcc?");
    assert!(
        draft.last_composer_activity_at
            >= Some(late_input_at.get().expect("late input was delivered"))
    );
    assert!(operations.try_recv().is_err());
    assert!(
        !std::iter::from_fn(|| app_events.try_recv().ok()).any(|event| matches!(
            event,
            AppEvent::CodexOp(AppCommand::UserTurn { .. }) | AppEvent::UpdateReasoningEffort(_)
        ))
    );
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn cloud_switch_wait_routes_ctrl_d_to_the_draft_or_queues_shutdown() -> Result<()> {
    use crate::app::cloud_switch_wait::ClipboardHandoff;
    use crate::app::cloud_switch_wait::wait_for_cloud_switch;
    use crate::app_event::ExitMode;

    let (mut app, mut app_events, _operations) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let ctrl_d = TuiEvent::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    let input = [
        TuiEvent::Key(KeyCode::Char('a').into()),
        ctrl_d.clone(), // Buffered typing counts as draft content.
        TuiEvent::Key(KeyCode::Home.into()),
        ctrl_d.clone(), // Forward-delete the draft's only character.
        ctrl_d,         // The draft is now empty.
    ];
    {
        let wait = wait_for_cloud_switch(
            &mut app,
            &mut tui,
            std::future::pending::<Result<(), std::convert::Infallible>>(),
            tokio_stream::iter(input).chain(tokio_stream::pending()),
            ClipboardHandoff::SettleBeforeNextView,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(&mut wait).is_pending());
    }
    assert_eq!(app.chat_widget.composer_draft_snapshot().text, "");
    let exits: Vec<_> = std::iter::from_fn(|| app_events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::Exit(mode) => Some(mode),
            _ => None,
        })
        .collect();
    assert_eq!(exits, [ExitMode::ShutdownFirst]);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn cloud_switch_wait_preserves_legacy_vim_escape_and_search_acceptance() -> Result<()> {
    use crate::app::cloud_switch_wait::ClipboardHandoff;
    use crate::app::cloud_switch_wait::wait_for_cloud_switch;

    let (mut app, _app_events, _operations) = make_test_app_with_channels().await;
    app.enhanced_keys_supported = false;
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("a");
    let mut tui = crate::tui::test_support::make_test_tui()?;
    wait_for_cloud_switch(
        &mut app,
        &mut tui,
        std::future::ready(Ok::<(), std::convert::Infallible>(())),
        tokio_stream::iter([
            TuiEvent::Key(KeyCode::Char('i').into()),
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT)),
            TuiEvent::Paste("alpha beta".into()),
            TuiEvent::Key(KeyCode::Char('?').into()),
            TuiEvent::Key(KeyCode::Char('l').into()),
            TuiEvent::Key(KeyCode::Enter.into()),
            TuiEvent::Key(KeyCode::Char('x').into()),
        ]),
        ClipboardHandoff::SettleBeforeNextView,
    )
    .await?;
    assert_eq!(app.chat_widget.composer_draft_snapshot().text, "apha beta");
    let draft_mode = crate::bottom_pane::RestrictedInputMode::DraftHandoff;
    for (key, expected) in [
        (KeyCode::Char('u').into(), "alpha beta"),
        (
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            "apha beta",
        ),
    ] {
        app.chat_widget.handle_restricted_key(key, draft_mode);
        assert_eq!(app.chat_widget.composer_draft_snapshot().text, expected);
    }
    let draft = app.chat_widget.composer_draft_snapshot();
    assert!(draft.vim_normal_mode);
    // Replace the composer as a successful Cloud switch does. Startup adoption alone
    // would return this draft to Insert, so the next x would insert instead of delete.
    let (mut destination, _app_events, _operations) = make_test_app_with_channels().await;
    destination.chat_widget.local_settings.tui.vim_mode_default = true;
    destination.chat_widget.toggle_vim_mode_and_notify();
    destination.chat_widget.restore_cloud_switch_draft(draft);
    destination
        .chat_widget
        .handle_restricted_key(KeyCode::Char('x').into(), draft_mode);
    insta::assert_snapshot!(destination.chat_widget.composer_draft_snapshot().text, @"aha beta");
    Ok(())
}

#[tokio::test]
async fn cloud_switch_wait_displays_pending_submission_hint() -> Result<()> {
    use crate::app::cloud_switch_wait::ClipboardHandoff;
    use crate::app::cloud_switch_wait::wait_for_cloud_switch;

    let (mut app, _app_events, _operations) = make_test_app_with_channels().await;
    let session = test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf());
    app.chat_widget.handle_thread_session(session);
    app.chat_widget
        .set_new_backend_control(Some("Cloud".into()));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    {
        let wait = wait_for_cloud_switch(
            &mut app,
            &mut tui,
            std::future::pending::<Result<(), std::convert::Infallible>>(),
            tokio_stream::iter([
                TuiEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
                TuiEvent::Key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)),
                TuiEvent::Draw,
            ])
            .chain(tokio_stream::pending()),
            ClipboardHandoff::SettleBeforeNextView,
        );
        tokio::pin!(wait);
        assert!(futures::poll!(&mut wait).is_pending());
    }
    let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 80);
    insta::assert_snapshot!(
        rendered
            .lines()
            .find(|line| line.contains("Connecting to Cloud"))
            .expect("Cloud connection hint")
            .trim(),
        @"Connecting to Cloud…"
    );
    Ok(())
}
