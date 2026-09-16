//! Permission selection through the existing app-server settings API.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn builtin_permission_selection_adopts_server_settings() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut server = start_config_write_test_app_server(&app).await?;
    let started = server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    let other = server.start_thread(&app.config).await?.session.thread_id;
    // A previously selected custom profile may have left a profile-specific proxy cached.
    let home = tempdir()?;
    std::fs::write(
        home.path().join("config.toml"),
        r#"default_permissions = "proxied"
[features]
network_proxy = true
[permissions.proxied]
extends = ":workspace"
[permissions.proxied.network]
enabled = true
proxy_url = "http://127.0.0.1:43128"
"#,
    )?;
    let proxied = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(codex_config::LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    assert!(proxied.permissions.network.is_some());
    app.chat_widget
        .set_permission_network(proxied.permissions.network);
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, "turn-1"),
        /*replay_kind*/ None,
    );
    assert!(app.chat_widget.is_user_turn_pending_or_running());
    let before = RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref());
    let profile_id = if before
        .active_permission_profile
        .as_ref()
        .is_some_and(|profile| profile.id == ":workspace")
    {
        ":read-only"
    } else {
        ":workspace"
    };
    while events.try_recv().is_ok() {}
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
    assert_eq!(
        RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref()),
        before,
    );
    assert!(app.pending_server_profiles.contains_key(&thread_id));
    insta::assert_snapshot!(next_history_message(&mut events).replace(profile_id, "<PROFILE>"), @"• Permission selection requested: <PROFILE>");
    while events.try_recv().is_ok() {}
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.handle_event(&mut tui, &mut server, AppEvent::NewSession { name: None })
        .await?;
    assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
    insta::assert_snapshot!(next_history_message(&mut events), @"■ Wait for permissions to update before switching tasks.");
    app.select_agents_overview_thread(&mut tui, &mut server, other)
        .await?;
    assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
    let settings = next_thread_settings_updated(&mut server, thread_id).await;
    app.enqueue_thread_notification(
        thread_id,
        ServerNotification::ThreadSettingsUpdated(settings),
    )
    .await?;
    assert!(!app.pending_server_profiles.contains_key(&thread_id));
    assert_eq!(
        (
            app.chat_widget
                .config_ref()
                .permissions
                .active_permission_profile(),
            app.config.approvals_reviewer
        ),
        (
            Some(ActivePermissionProfile::new(profile_id)),
            ApprovalsReviewer::User
        ),
    );
    assert_eq!(app.chat_widget.config_ref().permissions.network, None);
    assert_eq!(app.config.permissions.network, None);
    assert_eq!(
        app.runtime_permission_profile_override,
        Some(RuntimePermissionProfileOverride::from_config(&app.config))
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn matching_builtin_permission_selection_submits_pending_prompt() -> Result<()> {
    let (mut app, mut events, mut ops) = make_test_app_with_channels().await;
    let mut server = start_config_write_test_app_server(&app).await?;
    let started = server.start_thread(&app.config).await?;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app.chat_widget.initial_user_message =
        create_initial_user_message(Some("review this".to_string()), Vec::new(), Vec::new());
    let config = app.chat_widget.config_ref();
    let selection = PermissionProfileSelection {
        profile_id: config.permissions.active_permission_profile().unwrap().id,
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        approvals_reviewer: Some(config.approvals_reviewer),
        display_label: "Current permissions".to_string(),
    };
    while events.try_recv().is_ok() {}
    app.select_permission_profile(&mut server, selection).await;
    assert!(app.chat_widget.initial_user_message.is_none());
    assert!(app.pending_server_profiles.is_empty());
    let Op::UserTurn { items, .. } = next_user_turn_op(&mut ops) else {
        panic!("expected initial user turn");
    };
    assert_eq!(
        items,
        vec![AppServerUserInput::Text {
            text: "review this".to_string(),
            text_elements: Vec::new(),
        }]
    );
    server.shutdown().await?;
    Ok(())
}
