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
    app.runtime_approvals_reviewer_override = Some(ApprovalsReviewer::AutoReview);
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
    assert!(
        app.agents_overview
            .requested_permission_profiles
            .contains_key(&thread_id)
    );
    insta::assert_snapshot!(next_history_message(&mut events).replace(profile_id, "<PROFILE>"), @"• Permission selection requested: <PROFILE>");
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

#[tokio::test]
async fn custom_permission_selection_uses_server_definition_and_preserves_state_on_rejection()
-> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    app.config.ephemeral = false;
    let home = tempdir()?;
    std::fs::write(
        home.path().join("config.toml"),
        "default_permissions = \":workspace\"\n[permissions.shared]\nextends = \":workspace\"\n",
    )?;
    let server_config = ConfigBuilder::default()
        .codex_home(home.path().into())
        .build()
        .await?;
    let mut server = crate::start_embedded_app_server_for_picker(&server_config).await?;
    let started = server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    let client_home = tempdir()?;
    std::fs::write(
        client_home.path().join("config.toml"),
        "default_permissions = \":workspace\"\n[permissions.shared]\nextends = \":danger-full-access\"\n",
    )?;
    let client_config = ConfigBuilder::default()
        .codex_home(client_home.path().into())
        .build()
        .await?;
    app.config.codex_home = client_config.codex_home;
    app.config.custom_permission_profiles = client_config.custom_permission_profiles;
    let selection = PermissionProfileSelection {
        profile_id: "shared".into(),
        approval_policy: Some(AskForApproval::Never),
        approvals_reviewer: None,
        display_label: "shared".into(),
    };
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, "turn-1"),
        /*replay_kind*/ None,
    );
    assert!(app.chat_widget.is_user_turn_pending_or_running());
    let before = RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref());
    app.select_permission_profile(&mut server, selection.clone())
        .await;
    assert_eq!(
        RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref()),
        before
    );
    let settings = next_thread_settings_updated(&mut server, thread_id).await;
    let expected = PermissionProfile::from_legacy_sandbox_policy_for_cwd(
        &settings.thread_settings.sandbox_policy.to_core(),
        settings.thread_settings.cwd.as_path(),
    );
    assert!(matches!(
        settings.thread_settings.sandbox_policy,
        codex_app_server_protocol::SandboxPolicy::WorkspaceWrite { .. }
    ));
    app.enqueue_thread_notification(
        thread_id,
        ServerNotification::ThreadSettingsUpdated(settings),
    )
    .await?;
    assert_eq!(
        (
            app.chat_widget
                .config_ref()
                .permissions
                .permission_profile(),
            app.chat_widget
                .config_ref()
                .permissions
                .approval_policy
                .value()
        ),
        (&expected, AskForApproval::Never.to_core())
    );
    std::fs::write(
        home.path().join("config.toml"),
        "default_permissions = \":workspace\"\n[permissions.shared]\nextends = \":read-only\"\n",
    )?;
    app.select_permission_profile(
        &mut server,
        PermissionProfileSelection {
            approval_policy: None,
            ..selection.clone()
        },
    )
    .await;
    let settings = next_thread_settings_updated(&mut server, thread_id).await;
    assert!(matches!(
        settings.thread_settings.sandbox_policy,
        codex_app_server_protocol::SandboxPolicy::ReadOnly { .. }
    ));
    app.enqueue_thread_notification(
        thread_id,
        ServerNotification::ThreadSettingsUpdated(settings),
    )
    .await?;
    app.chat_widget.handle_server_notification(
        turn_completed_notification(thread_id, "turn-1", TurnStatus::Completed),
        /*replay_kind*/ None,
    );
    // Unchanged re-selection is accepted without requiring a notification.
    app.select_permission_profile(&mut server, selection.clone())
        .await;
    assert!(!app.reject_pending_permission_change());
    let confirmed = RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref());
    let confirmed_config = app.config.clone();
    app.select_permission_profile(
        &mut server,
        PermissionProfileSelection {
            profile_id: "missing-on-server".into(),
            ..selection
        },
    )
    .await;
    assert_eq!(
        RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref()),
        confirmed
    );
    assert!(!app.pending_server_profiles.contains_key(&thread_id));
    assert_eq!(app.config, confirmed_config);
    assert_eq!(
        app.agents_overview
            .selected_permission_profiles
            .get(&thread_id)
            .map(String::as_str),
        Some("shared")
    );
    // Materialize the task so /fork exercises persisted history with the confirmed profile.
    server.thread_inject_items(thread_id, vec![serde_json::from_value(serde_json::json!({
        "type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "saved history"}]
    }))?]).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    while events.try_recv().is_ok() {}
    app.change_working_directory(&mut tui, &mut server, client_home.path().abs())
        .await;
    assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
    insta::assert_snapshot!(next_history_message(&mut events), @"■ Changing directories with an unconfirmed named profile is not supported.");
    let side = server
        .fork_side_thread(
            &app.local_settings,
            app.config.clone(),
            thread_id,
            app.agents_overview
                .requested_permission_profiles
                .get(&thread_id),
        )
        .await?;
    assert_eq!(
        (
            side.session.permission_profile,
            side.session.approval_policy
        ),
        (PermissionProfile::read_only(), AskForApproval::Never)
    );
    app.cli_kv_overrides.push((
        "permissions.shared.extends".into(),
        TomlValue::String(":danger-full-access".into()),
    ));
    // Both the accepted request and its confirmed descendant remain server-owned.
    for _ in 0..2 {
        let previous_thread_id = app.chat_widget.thread_id();
        app.handle_event(
            &mut tui,
            &mut server,
            AppEvent::ForkCurrentSession { name: None },
        )
        .await?;
        assert_ne!(app.chat_widget.thread_id(), previous_thread_id);
        assert_eq!(
            RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref()),
            confirmed
        );
    }
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn accepted_permissions_survive_directory_change_before_notification() -> Result<()> {
    for preserve_history in [false, true] {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        app.config.ephemeral = false;
        app.config
            .permissions
            .set_permission_profile(PermissionProfile::Disabled)?;
        let destination = app.config.codex_home.join("destination");
        std::fs::create_dir(&destination)?;
        crate::legacy_core::config::set_project_trust_level(
            app.config.codex_home.as_path(),
            &destination,
            codex_protocol::config_types::TrustLevel::Trusted,
        )
        .map_err(|err| color_eyre::eyre::eyre!(err.to_string()))?;
        let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
        let started = server.start_thread(&app.config).await?;
        let source = started.session.thread_id;
        app.enqueue_primary_thread_session(started.session, started.turns)
            .await?;
        app.runtime_permission_profile_override =
            Some(RuntimePermissionProfileOverride::from_config(&app.config));
        if preserve_history {
            server.thread_inject_items(source, vec![serde_json::from_value(serde_json::json!({
                "type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "saved history"}]
            }))?]).await?;
        }
        app.select_permission_profile(
            &mut server,
            PermissionProfileSelection {
                profile_id: ":read-only".into(),
                approval_policy: Some(AskForApproval::OnRequest),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                display_label: "Read Only".into(),
            },
        )
        .await;
        assert!(
            app.agents_overview
                .requested_permission_profiles
                .contains_key(&source),
            "selection must be unconfirmed"
        );
        // Destination constraints belong to the client; the server's result is authoritative.
        let requirements = app.config.codex_home.join("client-requirements.toml");
        std::fs::write(&requirements, "allowed_approval_policies = [\"never\"]")?;
        app.loader_overrides.system_requirements_path = Some(requirements.to_path_buf());
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.change_working_directory(&mut tui, &mut server, destination.clone())
            .await;
        assert_ne!(app.chat_widget.thread_id(), Some(source));
        assert_eq!(
            (&app.config.cwd, app.config.permissions.permission_profile()),
            (&destination, &PermissionProfile::read_only())
        );
        assert_eq!(
            app.config.permissions.approval_policy.value(),
            AskForApproval::OnRequest.to_core()
        );
        assert_eq!(
            app.runtime_permission_profile_override,
            Some(RuntimePermissionProfileOverride::from_config(&app.config))
        );
        server.shutdown().await?;
    }
    Ok(())
}
