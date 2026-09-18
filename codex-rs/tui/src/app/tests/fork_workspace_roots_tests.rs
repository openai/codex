//! Remote forks must retain server roots after reloading the client's configuration.

use super::session_lifecycle_requests::recorded_params;
use super::session_lifecycle_requests::start_recording_remote_app_server;
use super::*;
use app_test_support::create_fake_rollout;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn remote_fork_dispatch_preserves_server_workspace_roots() -> Result<()> {
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
    let (server, requests, proxy) =
        Box::pin(start_recording_remote_app_server(&server_config)).await?;
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
    let mut tui = crate::tui::test_support::make_test_tui()?;

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
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}
