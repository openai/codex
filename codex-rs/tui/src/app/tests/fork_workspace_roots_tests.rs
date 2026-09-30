//! Remote forks must retain server roots after reloading the client's configuration.

use super::session_lifecycle_requests::recorded_params;
use super::session_lifecycle_requests::start_recording_app_server;
use super::session_lifecycle_requests::start_recording_remote_app_server;
use super::*;
use crate::app_server_session::ThreadParamsMode;
use app_test_support::create_fake_rollout;
use codex_app_server_client::AppServerEvent;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn remote_fork_dispatch_preserves_server_workspace_roots() -> Result<()> {
    check_fork_dispatch(ThreadParamsMode::Remote).await
}

#[tokio::test]
async fn local_daemon_fork_dispatch_preserves_idle_source_and_children() -> Result<()> {
    check_fork_dispatch(ThreadParamsMode::Embedded).await
}

async fn check_fork_dispatch(mode: ThreadParamsMode) -> Result<()> {
    let mut app = Box::pin(make_test_app()).await;
    let client_home = tempdir()?;
    let server_home = tempdir()?;
    let workspace = tempdir()?;
    let remote_cwd = workspace.path().canonicalize()?.abs();
    let server_root = server_home.path().canonicalize()?.abs();
    for home in [&client_home, &server_home] {
        let root = serde_json::to_string(&home.path().canonicalize()?)?;
        std::fs::write(
            home.path().join("config.toml"),
            format!(
                "sandbox_mode = \"workspace-write\"\n[sandbox_workspace_write]\nwritable_roots = [{root}]\n"
            ),
        )?;
    }
    app.config.codex_home = client_home.path().to_path_buf().abs();
    app.config.sqlite = codex_state::SqliteConfig::new_for_testing(client_home.path().abs());
    let server_config = ConfigBuilder::default()
        .codex_home(server_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(remote_cwd.to_path_buf()),
            ..Default::default()
        })
        .build()
        .await?;
    let (server, requests, proxy) = match mode {
        ThreadParamsMode::Remote => {
            Box::pin(start_recording_remote_app_server(&server_config)).await?
        }
        ThreadParamsMode::Embedded => {
            app.config = server_config.clone();
            Box::pin(start_recording_app_server(
                &server_config,
                /*blocked_thread_list*/ None,
                /*failed_thread_name*/ None,
            ))
            .await?
        }
    };
    let mut server = server.with_remote_cwd_override(Some(remote_cwd.to_path_buf()));
    let source_thread_id = ThreadId::from_string(
        &create_fake_rollout(
            server_home.path(),
            "2026-01-01T00-00-00",
            "2026-01-01T00:00:00Z",
            "Saved user message",
            Some(server_config.model_provider_id.as_str()),
            /*git_info*/ None,
        )
        .expect("create source rollout"),
    )?;
    let started = server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            source_thread_id,
            app.resume_model_settings(),
        )
        .await?;
    let expected_roots = vec![remote_cwd, server_root];
    assert_eq!(started.session.runtime_workspace_roots, expected_roots);
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app.thread_event_listener_tasks
        .insert(source_thread_id, tokio::spawn(std::future::pending::<()>()));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let pending = if mode == ThreadParamsMode::Remote {
        let approval = exec_approval_request(
            source_thread_id,
            "source-turn",
            "command",
            /*approval_id*/ None,
        );
        app.enqueue_thread_request(source_thread_id, approval.clone())
            .await?;
        app.drain_active_thread_events(&mut tui).await?;
        vec![approval]
    } else {
        Vec::new()
    };
    let child_id = ThreadId::new();
    app.upsert_agent_picker_thread(
        child_id, /*agent_nickname*/ None, /*agent_role*/ None, /*is_closed*/ false,
    );
    Box::pin(app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::ForkCurrentSession { name: None },
    ))
    .await?;

    assert_ne!(app.chat_widget.thread_id(), Some(source_thread_id));
    assert_eq!(app.chat_widget.config_ref().workspace_roots, expected_roots);
    let fork_roots: Vec<_> = recorded_params(&requests, "thread/fork")
        .into_iter()
        .map(|params| params["runtimeWorkspaceRoots"].clone())
        .collect();
    assert_eq!(fork_roots, vec![serde_json::json!(expected_roots)]);
    assert!(recorded_params(&requests, "thread/unsubscribe").is_empty());
    assert!(!app.thread_event_listener_tasks[&source_thread_id].is_finished());
    assert_eq!(
        app.agents_overview.dispatched_requests[&source_thread_id],
        pending
    );
    assert!(app.agents_overview.dispatched_requests[&child_id].is_empty());
    let question = request_user_input_request(source_thread_id, "source-turn", "question");
    app.handle_app_server_event(
        &server,
        AppServerEvent::ServerRequest(Box::new(question.clone())),
    )
    .await;
    assert_eq!(
        app.agents_overview.dispatched_requests[&source_thread_id].last(),
        Some(&question)
    );
    assert!(!app.chat_widget.has_active_view());
    Box::pin(app.select_agents_overview_thread(&mut tui, &mut server, source_thread_id)).await?;
    app.drain_active_thread_events(&mut tui).await?;
    assert!(
        app.pending_app_server_requests
            .contains_server_request(&question)
    );
    app.discard_thread_local_state(source_thread_id).await;
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}
