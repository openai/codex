use super::*;
use crate::app::tests::make_test_app_with_channels;
use codex_app_server_client::AppServerClient;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn experimental_features_use_selected_server_profile_and_preserve_task_settings() -> Result<()>
{
    let home = tempfile::tempdir()?;
    let selected = AbsolutePathBuf::from_absolute_path(home.path().join("work.config.toml"))?;
    let managed = home.path().join("managed.toml");
    std::fs::write(home.path().join("config.toml"), "# unselected\n")?;
    std::fs::write(&selected, "[features]\nnetwork_proxy = true\n")?;
    std::fs::write(&managed, "")?;
    let loader = LoaderOverrides {
        user_config_path: Some(selected.clone()),
        user_config_profile: Some("work".parse()?),
        managed_config_path: Some(managed.clone()),
        ignore_project_config: true,
        ..LoaderOverrides::without_managed_config_for_tests()
    };
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(loader.clone())
        .build()
        .await?;
    let client = Box::pin(crate::start_embedded_app_server(
        Default::default(),
        config.clone(),
        Vec::new(),
        loader,
        /*strict_config*/ false,
        Default::default(),
        codex_feedback::CodexFeedback::new(),
        /*log_db*/ None,
        /*state_db*/ None,
        Arc::new(EnvironmentManager::default_for_tests()),
        Default::default(),
    ))
    .await?;
    let mut server = AppServerSession::new(
        AppServerClient::InProcess(client),
        crate::app_server_session::ThreadParamsMode::Remote,
    );
    let thread = server.start_thread(&config).await?.session.thread_id;
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let before = (app.config.clone(), app.chat_widget.config_ref().clone());
    let updates = vec![
        ("network_proxy".to_string(), false),
        ("multi_agent".to_string(), false),
        ("memories".to_string(), true),
    ];
    let (response_tx, response_rx) = oneshot::channel();
    app.save_experimental_features(&server, thread, updates.clone(), response_tx);
    let (retry_tx, retry_rx) = oneshot::channel();
    app.save_experimental_features(&server, thread, updates.clone(), retry_tx);
    assert!(retry_rx.await?.unwrap_err().contains("still in progress"));
    assert!(matches!(events.try_recv()?, AppEvent::InsertHistoryCell(_)));
    let (catalog_tx, catalog_rx) = oneshot::channel();
    app.fetch_experimental_features(&server, thread, catalog_tx);
    // Navigation while the write is pending must not change the new task's settings.
    app.active_thread_id = Some(ThreadId::new());
    let result = tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 15), response_rx)
        .await??
        .map_err(|error| color_eyre::eyre::eyre!(error))?;
    assert_eq!(result.warning, None);
    assert_eq!(
        catalog_rx
            .await?
            .map_err(|error| color_eyre::eyre::eyre!(error))?,
        result.features
    );
    let saved: toml::Value = toml::from_str(&std::fs::read_to_string(&selected)?)?;
    assert_eq!(
        serde_json::to_value(saved)?,
        serde_json::json!({"features": {"multi_agent": false, "memories": true}})
    );
    assert_eq!(
        (app.config.clone(), app.chat_widget.config_ref().clone()),
        before
    );
    assert_eq!(
        std::fs::read_to_string(home.path().join("config.toml"))?,
        "# unselected\n"
    );
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let notice_text = |app: &App| {
        app.transcript_cells
            .last()
            .unwrap()
            .transcript_lines(/*width*/ 120)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };
    app.enable_feature_for_new_threads(&mut tui, &server, Feature::Collab)
        .await;
    insta::assert_snapshot!("subagents_enable_notice", notice_text(&app));
    std::fs::write(
        &selected,
        format!(
            "{}memory_tool = false\n",
            std::fs::read_to_string(&selected)?
        ),
    )?;
    app.enable_feature_for_new_threads(&mut tui, &server, Feature::MemoryTool)
        .await;
    insta::assert_snapshot!("memories_enable_notice", notice_text(&app));
    let saved: toml::Value = toml::from_str(&std::fs::read_to_string(&selected)?)?;
    assert_eq!(saved["features"]["multi_agent"].as_bool(), Some(true));
    assert_eq!(saved["features"]["memories"].as_bool(), Some(true));
    assert_eq!(saved["features"]["memory_tool"].as_bool(), Some(true));
    assert_eq!(
        (app.config.clone(), app.chat_widget.config_ref().clone()),
        before
    );
    assert_eq!(
        std::fs::read_to_string(home.path().join("config.toml"))?,
        "# unselected\n"
    );
    std::fs::write(&managed, "[features]\nmulti_agent = false\n")?;
    app.enable_feature_for_new_threads(&mut tui, &server, Feature::Collab)
        .await;
    insta::assert_snapshot!(
        "subagents_enable_overridden",
        notice_text(&app).replace(managed.display().to_string().as_str(), "<managed config>")
    );
    // A failed save still reports to history after its popup has closed.
    let (tx, rx) = oneshot::channel();
    drop(rx);
    app.save_experimental_features(&server, thread, vec![("missing_feature".into(), true)], tx);
    assert!(matches!(
        events.recv().await,
        Some(AppEvent::InsertHistoryCell(_))
    ));
    server.shutdown().await?;
    Ok(())
}
