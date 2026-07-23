use super::*;
use app_test_support::create_fake_rollout;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn fork_current_session_preserves_conversation_ultra() -> Result<()> {
    let mut app = make_test_app().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = codex_state::SqliteConfig::new_for_testing(codex_home.path().abs());
    let source_thread_id = ThreadId::from_string(
        &create_fake_rollout(
            codex_home.path(),
            "2025-01-05T12-00-00",
            "2025-01-05T12:00:00Z",
            "Saved user message",
            Some(app.config.model_provider_id.as_str()),
            /*git_info*/ None,
        )
        .expect("create source rollout"),
    )?;
    app.chat_widget.handle_thread_session(ThreadSessionState {
        model: "gpt-5.4".to_string(),
        reasoning_effort: Some(ReasoningEffortConfig::Ultra),
        ..test_thread_session(source_thread_id, test_path_buf("/tmp/project"))
    });
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    let control =
        Box::pin(app.handle_event(&mut tui, &mut app_server, AppEvent::ForkCurrentSession)).await?;

    assert!(matches!(control, AppRunControl::Continue));
    assert_ne!(app.chat_widget.thread_id(), Some(source_thread_id));
    assert_eq!(app.chat_widget.current_model(), "gpt-5.4");
    assert_eq!(
        app.chat_widget.current_reasoning_effort(),
        Some(ReasoningEffortConfig::Ultra)
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn switching_from_ultra_thread_restores_configured_plan_effort() {
    let mut app = make_test_app().await;
    app.config.plan_mode_reasoning_effort = Some(ReasoningEffortConfig::High);
    app.chat_widget
        .set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let ultra_session = ThreadSessionState {
        model: "gpt-5.4".to_string(),
        reasoning_effort: Some(ReasoningEffortConfig::Ultra),
        ..test_thread_session(ThreadId::new(), test_path_buf("/tmp/ultra"))
    };
    let normal_session = ThreadSessionState {
        model: "gpt-5.4".to_string(),
        reasoning_effort: Some(ReasoningEffortConfig::Medium),
        ..test_thread_session(ThreadId::new(), test_path_buf("/tmp/normal"))
    };

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: Some(ultra_session),
            turns: Vec::new(),
            events: Vec::new(),
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );
    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: Some(normal_session),
            turns: Vec::new(),
            events: Vec::new(),
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );
    app.chat_widget
        .handle_key_event(KeyEvent::from(KeyCode::BackTab));

    assert_eq!(
        app.chat_widget.active_collaboration_mode_kind(),
        ModeKind::Plan
    );
    assert_eq!(
        app.chat_widget.current_reasoning_effort(),
        Some(ReasoningEffortConfig::High)
    );
}
