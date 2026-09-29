//! Exercise implicit defaults and permission inheritance through TUI session requests.

use super::session_lifecycle_requests::HistoryCapabilities;
use super::session_lifecycle_requests::start_recording_app_server_with_history;
use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn projectless_permissions_survive_session_transitions_without_trust() -> Result<()> {
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let home = tempfile::tempdir()?;
    app.config.codex_home = home.path().to_path_buf().abs();
    app.config.sqlite = codex_state::SqliteConfig::new_for_testing(home.path().abs());
    let source = tempfile::tempdir()?;
    let destination = tempfile::tempdir()?;
    let model = core_test_support::responses::start_mock_server().await;
    let response = core_test_support::responses::mount_sse_once(
        &model,
        core_test_support::responses::sse(vec![
            core_test_support::responses::ev_response_created("saved-permissions"),
            core_test_support::responses::ev_completed("saved-permissions"),
        ]),
    )
    .await;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"model = "gpt-5.2"
model_provider = "test"
[model_providers.test]
name = "test"
base_url = "{}/v1"
wire_api = "responses"
[windows]
sandbox = "unelevated"
"#,
            model.uri()
        ),
    )?;
    app.config = app
        .rebuild_config_for_cwd(source.path().to_path_buf())
        .await?;
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(
            ThreadId::new(),
            source.path().to_path_buf(),
        ));
    let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.pause_events();
    app.start_fresh_session(
        &mut tui,
        &mut server,
        /*session_start_source*/ None,
        /*initial_user_message*/ None,
        /*new_thread_name*/ None,
    )
    .await;
    assert!(
        app.config
            .permissions
            .permission_profile()
            .file_system_sandbox_policy()
            .can_write_local_path_with_cwd(source.path(), source.path())
    );
    assert!(matches!(
        app.config.permissions.approval_policy.value(),
        codex_protocol::protocol::AskForApproval::Granular(_)
    ));
    // Saving history must not turn implicit writable defaults into read-only on /cd.
    server
        .thread_inject_items(
            app.chat_widget.thread_id().expect("fresh task"),
            vec![App::side_boundary_prompt_item()],
        )
        .await?;
    let fresh_destination = tempfile::tempdir()?;
    for cwd in [fresh_destination.path(), source.path()] {
        assert!(
            app.chat_widget
                .rollout_path()
                .as_deref()
                .is_some_and(rollout_path_is_resumable)
        );
        app.change_working_directory(&mut tui, &mut server, cwd.abs())
            .await;
        assert_eq!(app.config.cwd.as_path(), cwd);
        assert_eq!(
            app.config.permissions.active_permission_profile(),
            Some(ActivePermissionProfile::new(":workspace"))
        );
        assert!(matches!(
            app.config.permissions.approval_policy.value(),
            codex_protocol::protocol::AskForApproval::Granular(_)
        ));
    }
    let mut readonly = app.config.clone();
    readonly
        .permissions
        .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
            PermissionProfile::read_only(),
            ActivePermissionProfile::new(":read-only"),
        ))?;
    let saved = server.start_thread(&readonly).await?;
    server
        .thread_inject_items(
            saved.session.thread_id,
            vec![App::side_boundary_prompt_item()],
        )
        .await?;
    tui.pause_events();
    app.launch_cwd = source.path().to_path_buf();
    app.resume_target_session(
        &mut tui,
        &mut server,
        SessionTarget {
            path: None,
            thread_id: saved.session.thread_id,
            cwd: Some(source.path().to_path_buf()),
            history_mode: None,
        },
    )
    .await?;
    assert_eq!(app.chat_widget.thread_id(), Some(saved.session.thread_id));
    assert_eq!(
        app.config.permissions.active_permission_profile(),
        Some(ActivePermissionProfile::new(":read-only"))
    );
    // A normal turn must leave the saved profile on the server, even though the
    // widget knows its ID. Complete the turn so /cd exercises saved history.
    while events.try_recv().is_ok() {}
    app.chat_widget.apply_external_edit("hello".into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let op = std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::CodexOp(op @ AppCommand::UserTurn { .. }) => Some(op),
            _ => None,
        })
        .expect("user turn");
    assert!(
        app.try_submit_active_thread_op_via_app_server(&mut server, saved.session.thread_id, &op)
            .await?
    );
    tokio::time::timeout(Duration::from_secs(/*secs*/ 30), async {
        loop {
            let page = server
                .thread_turns_page(
                    saved.session.thread_id,
                    /*cursor*/ None,
                    /*limit*/ 1,
                )
                .await?;
            if page
                .data
                .first()
                .is_some_and(|turn| turn.status == codex_app_server_protocol::TurnStatus::Completed)
            {
                return Ok::<_, color_eyre::Report>(());
            }
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await??;
    response.single_request();
    let next = app.load_new_session_config(&server).await?;
    assert_eq!(
        next.permissions.permission_profile(),
        &PermissionProfile::workspace_write()
    );
    let before = app.chat_widget.thread_id();
    app.change_working_directory(
        &mut tui,
        &mut server,
        destination.path().to_path_buf().abs(),
    )
    .await;
    assert_eq!(app.config.cwd.as_path(), destination.path());
    assert_ne!(app.chat_widget.thread_id(), before);
    assert!(
        !app.config
            .permissions
            .effective_permission_profile()
            .file_system_sandbox_policy()
            .can_write_local_path_with_cwd(destination.path(), destination.path())
    );
    assert!(!std::fs::read_to_string(home.path().join("config.toml"))?.contains("trust_level"));
    let untrusted = tempfile::tempdir()?;
    let config_path = home.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            "{}\n[projects.{}]\ntrust_level = \"untrusted\"\n",
            std::fs::read_to_string(&config_path)?,
            serde_json::to_string(untrusted.path())?
        ),
    )?;
    app.change_working_directory(&mut tui, &mut server, untrusted.path().abs())
        .await;
    assert_eq!(
        (
            app.config.cwd.as_path(),
            app.config.permissions.approval_policy.value()
        ),
        (
            untrusted.path(),
            codex_protocol::protocol::AskForApproval::UnlessTrusted
        )
    );
    // A new task in a marked project still gets implicit defaults when leaving it.
    std::fs::create_dir(untrusted.path().join(".codex"))?;
    std::fs::write(untrusted.path().join(".codex/config.toml"), "")?;
    crate::config_update::write_trusted_project(server.request_handle(), untrusted.path()).await?;
    app.new_agents_overview_session(&mut tui, &mut server, Some(untrusted.path().abs()))
        .await?;
    server
        .thread_inject_items(
            app.chat_widget.thread_id().expect("new project task"),
            vec![App::side_boundary_prompt_item()],
        )
        .await?;
    app.change_working_directory(&mut tui, &mut server, source.path().abs())
        .await;
    assert_eq!(
        (
            app.config.cwd.as_path(),
            app.config.permissions.active_permission_profile()
        ),
        (
            source.path(),
            Some(ActivePermissionProfile::new(":workspace"))
        )
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn local_projectless_defaults_respect_trust_scope_and_explicit_settings() -> Result<()> {
    use codex_protocol::protocol::AskForApproval as Approval;
    for (settings, project_marker, expected_profile, expected_approval, sandbox_ready) in [
        ("", false, ":workspace", None, true),
        #[cfg(target_os = "windows")]
        ("", false, ":read-only", None, false),
        ("", true, ":read-only", None, true),
        (
            "sandbox_mode = \"read-only\"\n",
            false,
            ":read-only",
            None,
            true,
        ),
        (
            "sandbox_mode = \"read-only\"\n",
            false,
            ":read-only",
            None,
            false,
        ),
        (
            "default_permissions = \":read-only\"\n",
            false,
            ":read-only",
            None,
            false,
        ),
        (
            "approval_policy = \"on-request\"\n",
            false,
            ":workspace",
            Some(Approval::OnRequest),
            true,
        ),
    ] {
        let home = tempfile::tempdir()?;
        let parent = tempfile::tempdir()?;
        let cwd = parent.path().join("notes");
        std::fs::create_dir(&cwd)?;
        let parent_path = serde_json::to_string(parent.path())?;
        let windows = if sandbox_ready {
            "[windows]\nsandbox = \"unelevated\"\n"
        } else {
            ""
        };
        std::fs::write(
            home.path().join("config.toml"),
            format!("{settings}{windows}[projects.{parent_path}]\ntrust_level = \"untrusted\"\n"),
        )?;
        let overrides = ConfigOverrides {
            cwd: Some(cwd),
            ..Default::default()
        };
        let mut config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .harness_overrides(overrides.clone())
            .build()
            .await?;
        let original_approval = if settings.is_empty() && !project_marker {
            Approval::Granular(codex_protocol::protocol::GranularApprovalConfig {
                sandbox_approval: false,
                rules: false,
                skill_approval: false,
                request_permissions: true,
                mcp_elicitations: true,
            })
        } else {
            config.permissions.approval_policy.value()
        };
        if project_marker {
            // The server must notice a project layer added after client discovery.
            assert!(config.config_layer_stack.is_projectless());
            std::fs::create_dir(config.cwd.join(".codex"))?;
            std::fs::write(
                config.cwd.join(".codex/config.toml"),
                "model = \"untrusted-model\"\n",
            )?;
        }
        let (server, _, proxy) = start_recording_app_server_with_history(
            &config,
            HistoryCapabilities::Current,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            crate::app_server_session::ThreadParamsMode::Embedded,
            LoaderOverrides::without_managed_config_for_tests(),
        )
        .await?;
        let startup = crate::app::startup::prepare_fresh_startup_config(
            &mut config,
            &server,
            &[],
            &overrides,
            &EnvironmentManager::default_for_tests(),
        )
        .await?;
        assert_eq!(
            startup.prompt_windows_sandbox,
            cfg!(target_os = "windows") && !sandbox_ready && settings.is_empty() && !project_marker
        );
        assert_eq!(
            (
                config.permissions.permission_profile(),
                config.permissions.approval_policy.value()
            ),
            (
                &if expected_profile == ":workspace" {
                    PermissionProfile::workspace_write()
                } else {
                    PermissionProfile::read_only()
                },
                expected_approval.unwrap_or(original_approval)
            )
        );
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}
