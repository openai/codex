use super::*;
use codex_arg0::Arg0DispatchPaths;
use pretty_assertions::assert_eq;

fn read_only_selection() -> PermissionProfileSelection {
    PermissionProfileSelection {
        profile_id: ":read-only".to_string(),
        approval_policy: Some(AskForApproval::OnRequest),
        approvals_reviewer: Some(ApprovalsReviewer::User),
        display_label: "Read Only".to_string(),
    }
}

#[tokio::test]
async fn permission_shortcut_rejections_leave_state_unchanged() -> Result<()> {
    for experimental_api in [false, true] {
        let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let thread_id = ThreadId::new();
        app.active_thread_id = Some(thread_id);
        app.chat_widget
            .handle_thread_session_quiet(test_thread_session(
                thread_id,
                app.config.cwd.to_path_buf(),
            ));
        let original = RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref());
        let original_reviewer = app.config.approvals_reviewer;
        let client = crate::start_embedded_app_server_with(
            Arg0DispatchPaths::default(),
            app.config.clone(),
            Vec::new(),
            LoaderOverrides::without_managed_config_for_tests(),
            /*strict_config*/ false,
            CloudConfigBundleLoader::default(),
            codex_feedback::CodexFeedback::new(),
            /*log_db*/ None,
            /*state_db*/ None,
            Arc::clone(&app.environment_manager),
            |mut args| {
                args.experimental_api = experimental_api;
                codex_app_server_client::InProcessAppServerClient::start(args)
            },
        )
        .await?;
        let mut app_server = AppServerSession::new(
            codex_app_server_client::AppServerClient::InProcess(client),
            crate::app_server_session::ThreadParamsMode::Embedded,
        );
        while events.try_recv().is_ok() {}
        let transcript_len = app.transcript_cells.len();
        app.apply_permission_shortcut(
            &mut app_server,
            &mut tui,
            ThreadId::new(),
            read_only_selection(),
        )
        .await;
        assert_eq!(app.transcript_cells.len(), transcript_len);
        assert!(events.try_recv().is_err());
        app.apply_permission_shortcut(&mut app_server, &mut tui, thread_id, read_only_selection())
            .await;
        assert_eq!(
            RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref()),
            original
        );
        assert_eq!(app.config.approvals_reviewer, original_reviewer);
        let cell = app.transcript_cells.last().expect("rejection notice");
        insta::assert_snapshot!(
            if experimental_api {
                "permission_shortcut_server_error"
            } else {
                "permission_shortcut_unsupported"
            },
            lines_to_single_string(&cell.display_lines(/*width*/ 120))
                .replace(&thread_id.to_string(), "<THREAD_ID>")
        );
        assert!(events.try_recv().is_err());
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn permission_shortcut_confirms_without_persisting() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config
        .permissions
        .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
            PermissionProfile::workspace_write(),
            ActivePermissionProfile::new(":workspace"),
        ))?;
    let config_path = codex_home.path().join("config.toml");
    let contents = "approvals_reviewer = \"auto_review\"\n";
    std::fs::write(&config_path, contents)?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let started = app_server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.chat_widget
        .handle_thread_session_quiet(started.session.clone());
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    let contents = std::fs::read_to_string(&config_path)?;
    while events.try_recv().is_ok() {}

    app.apply_permission_shortcut(&mut app_server, &mut tui, thread_id, read_only_selection())
        .await;

    let cell = app.transcript_cells.last().expect("confirmed notice");
    insta::assert_snapshot!(
        lines_to_single_string(&cell.display_lines(/*width*/ 80)),
        @"• Permissions updated to Read Only"
    );

    let settings = next_thread_settings_updated(&mut app_server, thread_id)
        .await
        .thread_settings;
    let profile = app
        .chat_widget
        .config_ref()
        .permissions
        .active_permission_profile();
    assert_eq!(
        settings.active_permission_profile,
        profile.clone().map(Into::into)
    );
    assert_eq!(profile, Some(ActivePermissionProfile::new(":read-only")));
    assert_eq!(app.config.approvals_reviewer, ApprovalsReviewer::User);
    assert_eq!(std::fs::read_to_string(config_path)?, contents);
    assert!(
        events.try_recv().is_err(),
        "must not queue another update or config write"
    );
    app_server.shutdown().await?;
    Ok(())
}
