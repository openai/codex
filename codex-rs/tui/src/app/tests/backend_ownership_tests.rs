//! Selection must retain backend provenance even when native thread IDs collide.

use super::start_recording_app_server;
use super::*;
use crate::app_server_session::ResumeModelSettings;
use crate::backend_id::BackendId;
use crate::session_start::SessionStartAction;
use crate::session_start::SessionStartConfig;
use crate::session_start::complete_session_start;
use codex_app_server_protocol::ThreadHistoryMode;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn foreign_backend_selection_cannot_resume_same_id_or_unarchive() -> Result<()> {
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let (mut server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let origin = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let thread_id = ThreadId::new();
    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, app.config.cwd.to_path_buf()),
        Vec::new(),
    )
    .await?;
    app.chat_widget.insert_str("keep this draft");
    let target = SessionTarget {
        backend_id: origin.backend_id(),
        thread_id,
        path: None,
        cwd: Some(test_path_buf("/wrong-backend")),
        history_mode: Some(ThreadHistoryMode::Legacy),
    };
    requests.lock().unwrap().clear();
    while events.try_recv().is_ok() {}
    app.apply_resume_picker_selection(
        &mut tui,
        &mut server,
        SessionSelection::Resume(target.clone()),
    )
    .await?;
    app.resume_target_session(&mut tui, &mut server, target.clone())
        .await?;
    assert_eq!(app.current_displayed_thread_id(), Some(thread_id));
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "keep this draft"
    );
    let errors: Vec<_> = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| {
            match event {
                AppEvent::InsertHistoryCell(cell) => {
                    Some(lines_to_single_string(&cell.display_lines(/*width*/ 100)))
                }
                _ => None,
            }
        })
        .collect();
    insta::assert_snapshot!(errors.join("\n"), @r###"
    ■ This session belongs to a different backend. Open the session picker again.
    ■ This session belongs to a different backend. Open the session picker again.
    "###);
    for action in [
        SessionStartAction::Resume(
            ResumeModelSettings::RestoreFromThread,
            crate::resume_permissions::ResumePermissions::default(),
        ),
        SessionStartAction::Fork(crate::app_server_session::ForkPermissionMode::InheritSaved),
    ] {
        let result = complete_session_start(
            &mut server,
            SessionStartConfig {
                config: &app.config,
                local_settings: &app.local_settings,
            },
            &AppServerTarget::Embedded,
            &target,
            action,
            Err(color_eyre::eyre::eyre!(
                "session {thread_id} is archived. Run `codex unarchive {thread_id}`"
            )),
            async || panic!("foreign targets must not ask to unarchive"),
        )
        .await;
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("different backend")
        );
    }
    assert!(
        requests.lock().unwrap().is_empty(),
        "a foreign selection must cause zero requests"
    );
    origin.shutdown().await?;
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn startup_fallback_requires_no_established_backend() -> Result<()> {
    let app = make_test_app().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    drop(listener);
    let daemon_target = AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::WebSocket {
            websocket_url: format!("ws://{address}"),
            auth_token: None,
        },
        allow_embedded_fallback: true,
    };
    let selected = SessionTarget {
        backend_id: BackendId::new(),
        thread_id: ThreadId::new(),
        path: None,
        cwd: None,
        history_mode: None,
    };
    for selection in [
        SessionSelection::Resume(selected.clone()),
        SessionSelection::Fork(selected),
        SessionSelection::StartFresh,
    ] {
        let mut target = daemon_target.clone();
        target.preserve_selected_backend(&selection);
        let loader = LoaderOverrides::without_managed_config_for_tests();
        let policy = codex_app_server_client::EmbeddedNetworkPolicy::load(&loader).await;
        let mut state_db = None;
        let result = crate::start_app_server(
            &mut target,
            Default::default(),
            app.config.clone(),
            Vec::new(),
            loader,
            /*strict_config*/ false,
            crate::CloudConfigBundleLoader::default(),
            codex_feedback::CodexFeedback::new(),
            /*log_db*/ None,
            &mut state_db,
            Arc::clone(&app.environment_manager),
            policy,
        )
        .await;
        if matches!(selection, SessionSelection::StartFresh) {
            let client = result?;
            assert_eq!(target, AppServerTarget::Embedded);
            client.shutdown().await?;
        } else {
            assert!(
                result.is_err(),
                "a selected session must not change backend"
            );
        }
    }
    let error = crate::start_app_server_for_picker(
        BackendId::new(),
        &app.config,
        &daemon_target,
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        /*state_db*/ None,
        Arc::clone(&app.environment_manager),
    )
    .await
    .err()
    .expect("an auxiliary picker must not switch backends");
    assert!(
        error
            .to_string()
            .contains(crate::daemon_startup::FAILURE_HINT)
    );
    Ok(())
}

#[tokio::test]
async fn auxiliary_picker_connection_preserves_backend_selection() -> Result<()> {
    let mut app = make_test_app().await;
    let backend_id = BackendId::new();
    let mut sessions = Vec::new();
    for _ in 0..2 {
        sessions.push(
            crate::start_app_server_for_picker(
                backend_id,
                &app.config,
                &AppServerTarget::Embedded,
                Vec::new(),
                LoaderOverrides::without_managed_config_for_tests(),
                /*state_db*/ None,
                Arc::clone(&app.environment_manager),
            )
            .await?,
        );
    }
    let mut consumer = sessions.pop().unwrap();
    let mut picker = sessions.pop().unwrap();
    let id = app_test_support::create_fake_rollout(
        app.config.codex_home.as_path(),
        "2026-01-01T00-00-00",
        "2026-01-01T00:00:00Z",
        "backend-owned session",
        Some(&app.config.model_provider_id),
        /*git_info*/ None,
    )
    .expect("saved rollout");
    let target = crate::lookup_session_target_with_app_server(&mut picker, &app.config, &id)
        .await?
        .expect("picker target");
    assert_eq!(target.backend_id, consumer.backend_id());
    crate::legacy_core::config::set_project_trust_level(
        app.config.codex_home.as_path(),
        app.config.cwd.as_path(),
        codex_protocol::config_types::TrustLevel::Trusted,
    )
    .map_err(std::io::Error::other)?;
    app.harness_overrides.cwd = Some(app.config.cwd.to_path_buf());
    let thread_id = target.thread_id;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.resume_target_session(&mut tui, &mut consumer, target)
        .await?;
    assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
    picker.shutdown().await?;
    consumer.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn startup_rejects_foreign_resume_and_fork_before_requests() -> Result<()> {
    let app = make_test_app().await;
    let target = SessionTarget {
        backend_id: BackendId::new(),
        thread_id: ThreadId::new(),
        path: None,
        cwd: None,
        history_mode: None,
    };
    for selection in [
        SessionSelection::Resume(target.clone()),
        SessionSelection::Fork(target.clone()),
    ] {
        let (mut server, requests, proxy) = start_recording_app_server(
            &app.config,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
        )
        .await?;
        let bootstrap = server.bootstrap(&app.config).await?;
        requests.lock().unwrap().clear();
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let result = super::startup_defaults_tests::run_startup_for_test(
            &mut tui,
            crate::app::backend_sessions::InitialBackendSessions::singleton(server),
            app.config.clone(),
            Some(bootstrap),
            selection,
            app.app_server_target.clone(),
        )
        .await;
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("different backend")
        );
        assert!(requests.lock().unwrap().is_empty());
        proxy.abort();
    }
    Ok(())
}

#[tokio::test]
async fn prepared_resume_cannot_activate_through_another_backend() -> Result<()> {
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let (mut server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let active_thread = ThreadId::new();
    app.enqueue_primary_thread_session(
        test_thread_session(active_thread, app.config.cwd.to_path_buf()),
        Vec::new(),
    )
    .await?;
    app.chat_widget.insert_str("keep this draft");
    let prepared = super::super::resume_preparation::PreparedResume {
        backend_id: BackendId::new(),
        config: app.config.clone(),
        local_settings: app.local_settings.clone(),
        thread: AppServerStartedThread {
            session: test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf()),
            turns: Vec::new(),
            blocks_direct_input: false,
            task_tools_available: true,
        },
        read_only: false,
        history_notice: None,
    };
    requests.lock().unwrap().clear();
    while events.try_recv().is_ok() {}
    app.activate_prepared_session(&mut tui, &mut server, prepared)
        .await?;
    assert_eq!(app.current_displayed_thread_id(), Some(active_thread));
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "keep this draft"
    );
    assert!(requests.lock().unwrap().is_empty());
    let Some(AppEvent::InsertHistoryCell(cell)) = events.recv().await else {
        panic!("expected backend mismatch error");
    };
    insta::assert_snapshot!(lines_to_single_string(&cell.display_lines(/*width*/ 100)), @"■ This session belongs to a different backend. Open the session picker again.");
    server.shutdown().await?;
    proxy.abort();
    Ok(())
}
