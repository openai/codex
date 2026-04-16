use super::*;
use codex_app_server_protocol::AppInfo;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn realtime_error_closes_without_followup_closed_info() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;

    chat.on_realtime_conversation_realtime(RealtimeConversationRealtimeEvent {
        payload: RealtimeEvent::Error("boom".to_string()),
    });
    next_realtime_close_op(&mut op_rx);

    chat.on_realtime_conversation_closed(RealtimeConversationClosedEvent {
        reason: Some("error".to_string()),
    });

    let rendered = drain_insert_history(&mut rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<Vec<_>>();
    insta::assert_snapshot!(rendered.join("\n\n"), @"■ Realtime voice error: boom");
}

#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn deleted_realtime_meter_uses_shared_stop_path() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;
    let placeholder_id = chat.bottom_pane.insert_recording_meter_placeholder("⠤⠤⠤⠤");
    chat.realtime_conversation.meter_placeholder_id = Some(placeholder_id.clone());

    assert!(chat.stop_realtime_conversation_for_deleted_meter(&placeholder_id));

    next_realtime_close_op(&mut op_rx);
    assert_eq!(chat.realtime_conversation.meter_placeholder_id, None);
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Stopping
    );
}

#[tokio::test]
async fn experimental_mode_plan_is_ignored_on_startup() {
    let codex_home = tempdir().expect("tempdir");
    let cfg = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cli_overrides(vec![
            (
                "features.collaboration_modes".to_string(),
                TomlValue::Boolean(true),
            ),
            (
                "tui.experimental_mode".to_string(),
                TomlValue::String("plan".to_string()),
            ),
        ])
        .build()
        .await
        .expect("config");
    let resolved_model = crate::legacy_core::test_support::get_model_offline(cfg.model.as_deref());
    let session_telemetry = test_session_telemetry(&cfg, resolved_model.as_str());
    let init = ChatWidgetInit {
        config: cfg.clone(),
        frame_requester: FrameRequester::test_dummy(),
        app_event_tx: AppEventSender::new(unbounded_channel::<AppEvent>().0),
        initial_user_message: None,
        enhanced_keys_supported: false,
        has_chatgpt_account: false,
        model_catalog: test_model_catalog(&cfg),
        feedback: codex_feedback::CodexFeedback::new(),
        is_first_run: true,
        status_account_display: None,
        initial_plan_type: None,
        model: Some(resolved_model.clone()),
        startup_tooltip_override: None,
        status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        session_telemetry,
    };

    let chat = ChatWidget::new_with_app_event(init);
    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Default);
    assert_eq!(chat.current_model(), resolved_model);
}

#[tokio::test]
async fn plugins_popup_loading_state_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    chat.add_plugins_output();

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        popup.contains("Loading available plugins..."),
        "expected /plugins to open in a loading state before the marketplace arrives, got:\n{popup}"
    );
    assert_chatwidget_snapshot!("plugins_popup_loading_state", popup);
}

#[tokio::test]
async fn plugins_popup_snapshot_shows_all_marketplaces_and_sorts_installed_then_name() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let mut response = plugins_test_response(vec![
        plugins_test_curated_marketplace(vec![
            plugins_test_summary(
                "plugin-bravo",
                "bravo",
                Some("Bravo Search"),
                Some("Search docs and tickets."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
            plugins_test_summary(
                "plugin-alpha",
                "alpha",
                Some("Alpha Sync"),
                Some("Already installed but disabled."),
                /*installed*/ true,
                /*enabled*/ false,
                PluginInstallPolicy::Available,
            ),
            plugins_test_summary(
                "plugin-starter",
                "starter",
                Some("Starter"),
                Some("Included by default."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::InstalledByDefault,
            ),
        ]),
        plugins_test_repo_marketplace(vec![plugins_test_summary(
            "plugin-hidden",
            "hidden",
            Some("Hidden Repo Plugin"),
            Some("Should not be shown in /plugins."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        )]),
    ]);
    response.remote_sync_error = Some("remote sync timed out".to_string());

    let popup = render_loaded_plugins_popup(&mut chat, response);
    assert_chatwidget_snapshot!("plugins_popup_curated_marketplace", popup);
    assert!(
        popup.contains("Hidden Repo Plugin"),
        "expected /plugins to include non-curated marketplaces, got:\n{popup}"
    );
    assert!(
        plugins_test_popup_row_position(&popup, "Alpha Sync")
            < plugins_test_popup_row_position(&popup, "Bravo Search")
            && plugins_test_popup_row_position(&popup, "Bravo Search")
                < plugins_test_popup_row_position(&popup, "Hidden Repo Plugin")
            && plugins_test_popup_row_position(&popup, "Hidden Repo Plugin")
                < plugins_test_popup_row_position(&popup, "Starter"),
        "expected /plugins rows to sort installed plugins first, then alphabetically, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugins_popup_truncates_long_descriptions_in_list_rows() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let response = plugins_test_response(vec![plugins_test_curated_marketplace(vec![
        plugins_test_summary(
            "plugin-alpha",
            "alpha",
            Some("Alpha"),
            Some("Short description."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
        plugins_test_summary(
            "plugin-verbose",
            "verbose",
            Some("Verbose Plugin"),
            Some("This description keeps going and going until the row would normally wrap."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
    ])]);

    let cwd = chat.config.cwd.to_path_buf();
    chat.on_plugins_loaded(cwd, Ok(response));
    chat.add_plugins_output();

    let popup = render_bottom_popup(&chat, /*width*/ 70);
    let verbose_row = popup
        .lines()
        .find(|line| line.contains("Verbose Plugin"))
        .expect("expected verbose plugin row in popup");
    insta::assert_snapshot!(
        verbose_row,
        @"  [-] Verbose Plugin  Available · ChatGPT Marketplace · This descri…"
    );
    assert!(
        !popup
            .contains("This description keeps going and going until the row would normally wrap."),
        "expected the long plugin description to truncate instead of wrapping, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugins_popup_add_marketplace_tab_opens_prompt_and_submits_source() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let cwd = chat.config.cwd.to_path_buf();
    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![plugins_test_curated_marketplace(Vec::new())]),
    );

    while rx.try_recv().is_ok() {}
    for _ in 0..3 {
        chat.handle_key_event(KeyEvent::from(KeyCode::Right));
    }

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        popup.contains("Add a marketplace from a Git repo or local root."),
        "expected Add Marketplace tab, got:\n{popup}"
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    match rx.try_recv() {
        Ok(AppEvent::OpenMarketplaceAddPrompt) => {}
        other => panic!("expected OpenMarketplaceAddPrompt event, got {other:?}"),
    }

    chat.open_marketplace_add_prompt();
    let prompt = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        prompt.contains("owner/repo, git URL, or local marketplace path"),
        "expected marketplace source prompt, got:\n{prompt}"
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    match rx.try_recv() {
        Ok(AppEvent::OpenMarketplaceAddLoading { source }) => {
            assert_eq!(source, "owner/repo");
        }
        other => panic!("expected OpenMarketplaceAddLoading event, got {other:?}"),
    }
    match rx.try_recv() {
        Ok(AppEvent::FetchMarketplaceAdd {
            cwd: event_cwd,
            source,
        }) => {
            assert_eq!(event_cwd, cwd);
            assert_eq!(source, "owner/repo");
        }
        other => panic!("expected FetchMarketplaceAdd event, got {other:?}"),
    }
}

#[tokio::test]
async fn marketplace_add_success_refreshes_to_new_marketplace_tab() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let cwd = chat.config.cwd.to_path_buf();
    let marketplace_root = plugins_test_absolute_path("marketplaces/debug");
    let marketplace_path =
        plugins_test_absolute_path("marketplaces/debug/.agents/plugins/marketplace.json");
    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![plugins_test_curated_marketplace(Vec::new())]),
    );
    chat.open_marketplace_add_loading_popup("owner/repo");
    chat.on_marketplace_add_loaded(
        cwd.clone(),
        "owner/repo".to_string(),
        Ok(MarketplaceAddResponse {
            marketplace_name: "debug".to_string(),
            installed_root: marketplace_root,
            already_added: false,
        }),
    );
    chat.on_plugins_loaded(
        cwd,
        Ok(plugins_test_response(vec![
            plugins_test_curated_marketplace(Vec::new()),
            PluginMarketplaceEntry {
                name: "debug".to_string(),
                path: marketplace_path,
                interface: Some(MarketplaceInterface {
                    display_name: Some("Debug Marketplace".to_string()),
                }),
                plugins: vec![plugins_test_summary(
                    "plugin-debug",
                    "debug",
                    Some("Debug Plugin"),
                    Some("Debug marketplace plugin."),
                    /*installed*/ false,
                    /*enabled*/ true,
                    PluginInstallPolicy::Available,
                )],
            },
        ])),
    );

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        popup.contains("Debug Marketplace.") && popup.contains("Debug Plugin"),
        "expected marketplace add refresh to switch to the new marketplace tab, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugin_detail_popup_snapshot_shows_install_actions_and_capability_summaries() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let summary = plugins_test_summary(
        "plugin-figma",
        "figma",
        Some("Figma"),
        Some("Design handoff."),
        /*installed*/ false,
        /*enabled*/ true,
        PluginInstallPolicy::Available,
    );
    let response = plugins_test_response(vec![plugins_test_curated_marketplace(vec![
        summary.clone(),
    ])]);
    let cwd = chat.config.cwd.clone();
    chat.on_plugins_loaded(cwd.to_path_buf(), Ok(response));
    chat.add_plugins_output();
    chat.on_plugin_detail_loaded(
        cwd.to_path_buf(),
        Ok(PluginReadResponse {
            plugin: plugins_test_detail(
                summary,
                Some("Turn Figma files into implementation context."),
                &["design-review", "extract-copy"],
                &[("Figma", true), ("Slack", false)],
                &["figma-mcp", "docs-mcp"],
            ),
        }),
    );

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert_chatwidget_snapshot!(
        "plugin_detail_popup_installable",
        strip_osc8_for_snapshot(&popup)
    );
}

#[tokio::test]
async fn plugin_detail_popup_hides_disclosure_for_installed_plugins() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let summary = plugins_test_summary(
        "plugin-figma",
        "figma",
        Some("Figma"),
        Some("Design handoff."),
        /*installed*/ true,
        /*enabled*/ true,
        PluginInstallPolicy::Available,
    );
    let response = plugins_test_response(vec![plugins_test_curated_marketplace(vec![
        summary.clone(),
    ])]);
    let cwd = chat.config.cwd.clone();
    chat.on_plugins_loaded(cwd.to_path_buf(), Ok(response));
    chat.add_plugins_output();
    chat.on_plugin_detail_loaded(
        cwd.to_path_buf(),
        Ok(PluginReadResponse {
            plugin: plugins_test_detail(
                summary,
                Some("Turn Figma files into implementation context."),
                &["design-review", "extract-copy"],
                &[("Figma", true), ("Slack", false)],
                &["figma-mcp", "docs-mcp"],
            ),
        }),
    );

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        !popup.contains("Data shared with this app is subject to the app's"),
        "expected installed plugin details to hide the disclosure line, got:\n{popup}"
    );
    assert_chatwidget_snapshot!(
        "plugin_detail_popup_installed",
        strip_osc8_for_snapshot(&popup)
    );
}

#[tokio::test]
async fn plugins_popup_refresh_preserves_selected_row_position() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let initial = plugins_test_response(vec![plugins_test_curated_marketplace(vec![
        plugins_test_summary(
            "plugin-notion",
            "notion",
            Some("Notion"),
            Some("Workspace docs."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
        plugins_test_summary(
            "plugin-slack",
            "slack",
            Some("Slack"),
            Some("Team chat."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
    ])]);
    render_loaded_plugins_popup(&mut chat, initial);
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));

    let before = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        before.contains("› [-] Slack"),
        "expected Slack to be selected before refresh, got:\n{before}"
    );

    let refreshed = plugins_test_response(vec![plugins_test_curated_marketplace(vec![
        plugins_test_summary(
            "plugin-airtable",
            "airtable",
            Some("Airtable"),
            Some("Structured records."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
        plugins_test_summary(
            "plugin-notion",
            "notion",
            Some("Notion"),
            Some("Workspace docs."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
        plugins_test_summary(
            "plugin-slack",
            "slack",
            Some("Slack"),
            Some("Team chat."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
    ])]);
    let cwd = chat.config.cwd.clone();
    chat.on_plugins_loaded(cwd.to_path_buf(), Ok(refreshed));

    let after = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        after.contains("› [-] Notion"),
        "expected refresh to preserve the selected row position, got:\n{after}"
    );
    assert!(
        after.contains("Airtable"),
        "expected refreshed popup to include the updated plugin list, got:\n{after}"
    );
    assert!(
        after.contains("Slack"),
        "expected refreshed popup to include the updated plugin list, got:\n{after}"
    );
}

#[tokio::test]
async fn plugins_popup_refreshes_installed_counts_after_install() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let initial = plugins_test_response(vec![plugins_test_curated_marketplace(vec![
        plugins_test_summary(
            "plugin-calendar",
            "calendar",
            Some("Calendar"),
            Some("Schedule management."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
        plugins_test_summary(
            "plugin-drive",
            "drive",
            Some("Drive"),
            Some("Document access."),
            /*installed*/ true,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
    ])]);
    let before = render_loaded_plugins_popup(&mut chat, initial);
    assert!(
        before.contains("Installed 1 of 2 available plugins."),
        "expected initial installed count before refresh, got:\n{before}"
    );
    assert!(
        before.contains("Available"),
        "expected pre-install popup copy before refresh, got:\n{before}"
    );

    let refreshed = plugins_test_response(vec![plugins_test_curated_marketplace(vec![
        plugins_test_summary(
            "plugin-calendar",
            "calendar",
            Some("Calendar"),
            Some("Schedule management."),
            /*installed*/ true,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
        plugins_test_summary(
            "plugin-drive",
            "drive",
            Some("Drive"),
            Some("Document access."),
            /*installed*/ true,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        ),
    ])]);
    let cwd = chat.config.cwd.clone();
    chat.on_plugins_loaded(cwd.to_path_buf(), Ok(refreshed));

    let after = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        after.contains("Installed 2 of 2 available plugins."),
        "expected /plugins to refresh installed counts after install, got:\n{after}"
    );
    assert!(
        after.contains("Installed   Space to disable; Enter view details."),
        "expected refreshed selected row copy to reflect the installed plugin state, got:\n{after}"
    );
}

#[tokio::test]
async fn plugins_popup_space_toggles_installed_plugin_from_list() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let cwd = chat.config.cwd.to_path_buf();
    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![plugins_test_curated_marketplace(vec![
            plugins_test_summary(
                "plugin-calendar",
                "calendar",
                Some("Calendar"),
                Some("Schedule management."),
                /*installed*/ true,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
            plugins_test_summary(
                "plugin-drive",
                "drive",
                Some("Drive"),
                Some("Document access."),
                /*installed*/ true,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
        ])]),
    );

    while rx.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

    match rx.try_recv() {
        Ok(AppEvent::SetPluginEnabled {
            cwd: event_cwd,
            plugin_id,
            enabled,
        }) => {
            assert_eq!(event_cwd, cwd);
            assert_eq!(plugin_id, "plugin-drive");
            assert!(!enabled);
        }
        other => panic!("expected SetPluginEnabled event, got {other:?}"),
    }

    chat.on_plugin_enabled_set(
        cwd,
        "plugin-drive".to_string(),
        /*enabled*/ false,
        Ok(()),
    );

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        popup.contains("› [ ] Drive"),
        "expected selected plugin row to stay selected after refresh, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugins_popup_space_on_uninstalled_row_does_not_start_search() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![plugins_test_curated_marketplace(vec![
            plugins_test_summary(
                "plugin-calendar",
                "calendar",
                Some("Calendar"),
                Some("Schedule management."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
            plugins_test_summary(
                "plugin-drive",
                "drive",
                Some("Drive"),
                Some("Document access."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
        ])]),
    );

    while rx.try_recv().is_ok() {}
    let before = render_bottom_popup(&chat, /*width*/ 100);
    chat.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    let after = render_bottom_popup(&chat, /*width*/ 100);

    assert!(
        rx.try_recv().is_err(),
        "did not expect Space on an uninstalled plugin to emit an event"
    );
    assert_eq!(after, before);
}

#[tokio::test]
async fn plugins_popup_space_with_active_search_does_not_toggle_installed_plugin() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![plugins_test_curated_marketplace(vec![
            plugins_test_summary(
                "plugin-calendar",
                "calendar",
                Some("Calendar"),
                Some("Schedule management."),
                /*installed*/ true,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
            plugins_test_summary(
                "plugin-drive",
                "drive",
                Some("Drive"),
                Some("Document access."),
                /*installed*/ true,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
        ])]),
    );

    while rx.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    type_plugins_search_query(&mut chat, "dr");
    chat.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

    assert!(
        rx.try_recv().is_err(),
        "did not expect Space with an active plugin search to emit a toggle event"
    );
}

#[tokio::test]
async fn plugins_popup_search_filters_visible_rows_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![plugins_test_curated_marketplace(vec![
            plugins_test_summary(
                "plugin-calendar",
                "calendar",
                Some("Calendar"),
                Some("Schedule management."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
            plugins_test_summary(
                "plugin-slack",
                "slack",
                Some("Slack"),
                Some("Team chat."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
            plugins_test_summary(
                "plugin-drive",
                "drive",
                Some("Drive"),
                Some("Document access."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
        ])]),
    );

    type_plugins_search_query(&mut chat, "sla");

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert_chatwidget_snapshot!("plugins_popup_search_filtered", popup);
    assert!(
        !popup.contains("Calendar") && !popup.contains("Drive"),
        "expected search to leave only matching rows visible, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugins_popup_installed_tab_filters_rows_and_clears_search() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![plugins_test_curated_marketplace(vec![
            plugins_test_summary(
                "plugin-calendar",
                "calendar",
                Some("Calendar"),
                Some("Schedule management."),
                /*installed*/ true,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
            plugins_test_summary(
                "plugin-slack",
                "slack",
                Some("Slack"),
                Some("Team chat."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
        ])]),
    );

    type_plugins_search_query(&mut chat, "sla");
    chat.handle_key_event(KeyEvent::from(KeyCode::Right));

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        popup.contains("Installed plugins.") && popup.contains("Showing 1 installed plugins."),
        "expected Installed tab header, got:\n{popup}"
    );
    assert!(
        popup.contains("Calendar") && !popup.contains("Slack"),
        "expected Installed tab to show only installed plugins, got:\n{popup}"
    );
    assert!(
        !popup.contains("sla"),
        "expected tab switch to clear search query, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugins_popup_openai_curated_tab_omits_marketplace_in_rows() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![
            plugins_test_curated_marketplace(vec![plugins_test_summary(
                "plugin-calendar",
                "calendar",
                Some("Calendar"),
                Some("Schedule management."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            )]),
            plugins_test_repo_marketplace(vec![plugins_test_summary(
                "plugin-repo",
                "repo",
                Some("Repo Plugin"),
                Some("Repo-only plugin."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            )]),
        ]),
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Right));
    chat.handle_key_event(KeyEvent::from(KeyCode::Right));

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        popup.contains("OpenAI Curated marketplace."),
        "expected OpenAI Curated tab header, got:\n{popup}"
    );
    assert!(
        popup.contains("Calendar") && !popup.contains("Repo Plugin"),
        "expected OpenAI Curated tab to show only official marketplace plugins, got:\n{popup}"
    );
    assert!(
        !popup.contains("ChatGPT Marketplace ·"),
        "expected marketplace-specific rows to omit marketplace labels, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugins_popup_refresh_preserves_duplicate_marketplace_tab_by_path() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let response = plugins_test_response(vec![
        PluginMarketplaceEntry {
            name: "duplicate".to_string(),
            path: plugins_test_absolute_path("marketplaces/home/marketplace.json"),
            interface: Some(MarketplaceInterface {
                display_name: Some("Duplicate Marketplace".to_string()),
            }),
            plugins: vec![plugins_test_summary(
                "plugin-home",
                "home",
                Some("Home Plugin"),
                Some("Home marketplace plugin."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            )],
        },
        PluginMarketplaceEntry {
            name: "duplicate".to_string(),
            path: plugins_test_absolute_path("marketplaces/repo/marketplace.json"),
            interface: Some(MarketplaceInterface {
                display_name: Some("Duplicate Marketplace".to_string()),
            }),
            plugins: vec![plugins_test_summary(
                "plugin-repo",
                "repo",
                Some("Repo Plugin"),
                Some("Repo marketplace plugin."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            )],
        },
    ]);
    let cwd = chat.config.cwd.to_path_buf();
    chat.on_plugins_loaded(cwd.clone(), Ok(response.clone()));
    chat.add_plugins_output();

    for _ in 0..4 {
        chat.handle_key_event(KeyEvent::from(KeyCode::Right));
    }

    chat.on_plugins_loaded(cwd, Ok(response));

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        popup.contains("Duplicate Marketplace (2/2)."),
        "expected refresh to preserve the second duplicate marketplace tab, got:\n{popup}"
    );
    assert!(
        popup.contains("Repo Plugin") && !popup.contains("Home Plugin"),
        "expected second duplicate marketplace rows after refresh, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugins_popup_search_no_matches_and_backspace_restores_results() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![plugins_test_curated_marketplace(vec![
            plugins_test_summary(
                "plugin-calendar",
                "calendar",
                Some("Calendar"),
                Some("Schedule management."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
            plugins_test_summary(
                "plugin-slack",
                "slack",
                Some("Slack"),
                Some("Team chat."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            ),
        ])]),
    );

    type_plugins_search_query(&mut chat, "zzz");

    let no_matches = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        no_matches.contains("zzz"),
        "expected popup to show the typed search query, got:\n{no_matches}"
    );
    assert!(
        no_matches.contains("no matches"),
        "expected popup to render the no-matches UX, got:\n{no_matches}"
    );

    for _ in 0..3 {
        chat.handle_key_event(KeyEvent::from(KeyCode::Backspace));
    }

    let restored = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        restored.contains("Calendar") && restored.contains("Slack"),
        "expected clearing the query to restore the plugin rows, got:\n{restored}"
    );
    assert!(
        !restored.contains("no matches"),
        "did not expect the no-matches state after clearing the query, got:\n{restored}"
    );
}

#[tokio::test]
async fn apps_popup_stays_loading_until_final_snapshot_updates() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);
    let notion_id = "unit_test_apps_popup_refresh_connector_1";
    let linear_id = "unit_test_apps_popup_refresh_connector_2";

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: notion_id.to_string(),
                name: "Notion".to_string(),
                description: Some("Workspace docs".to_string()),
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/notion".to_string()),
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ false,
    );
    chat.add_connectors_output();
    assert!(
        chat.connectors_prefetch_in_flight,
        "expected /apps to trigger a forced connectors refresh"
    );

    let before = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        before.contains("Loading installed and available apps..."),
        "expected /apps to stay in the loading state until the full list arrives, got:\n{before}"
    );
    assert_chatwidget_snapshot!("apps_popup_loading_state", before);

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![
                AppInfo {
                    id: notion_id.to_string(),
                    name: "Notion".to_string(),
                    description: Some("Workspace docs".to_string()),
                    logo_url: None,
                    logo_url_dark: None,
                    distribution_channel: None,
                    branding: None,
                    app_metadata: None,
                    labels: None,
                    install_url: Some("https://example.test/notion".to_string()),
                    is_accessible: true,
                    is_enabled: true,
                    plugin_display_names: Vec::new(),
                },
                AppInfo {
                    id: linear_id.to_string(),
                    name: "Linear".to_string(),
                    description: Some("Project tracking".to_string()),
                    logo_url: None,
                    logo_url_dark: None,
                    distribution_channel: None,
                    branding: None,
                    app_metadata: None,
                    labels: None,
                    install_url: Some("https://example.test/linear".to_string()),
                    is_accessible: true,
                    is_enabled: true,
                    plugin_display_names: Vec::new(),
                },
            ],
        }),
        /*is_final*/ true,
    );

    let after = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        after.contains("Installed 2 of 2 available apps."),
        "expected refreshed apps popup snapshot, got:\n{after}"
    );
    assert!(
        after.contains("Linear"),
        "expected refreshed popup to include new connector, got:\n{after}"
    );
}

#[tokio::test]
async fn apps_refresh_failure_keeps_existing_full_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);
    let notion_id = "unit_test_apps_refresh_failure_connector_1";
    let linear_id = "unit_test_apps_refresh_failure_connector_2";

    let full_connectors = vec![
        AppInfo {
            id: notion_id.to_string(),
            name: "Notion".to_string(),
            description: Some("Workspace docs".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some("https://example.test/notion".to_string()),
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        },
        AppInfo {
            id: linear_id.to_string(),
            name: "Linear".to_string(),
            description: Some("Project tracking".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some("https://example.test/linear".to_string()),
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        },
    ];
    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: full_connectors.clone(),
        }),
        /*is_final*/ true,
    );

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: notion_id.to_string(),
                name: "Notion".to_string(),
                description: Some("Workspace docs".to_string()),
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/notion".to_string()),
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ false,
    );
    chat.on_connectors_loaded(
        Err("failed to load apps".to_string()),
        /*is_final*/ true,
    );

    assert_matches!(
        &chat.connectors_cache,
        ConnectorsCacheState::Ready(snapshot) if snapshot.connectors == full_connectors
    );

    chat.add_connectors_output();
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Installed 1 of 2 available apps."),
        "expected previous full snapshot to be preserved, got:\n{popup}"
    );
}

#[tokio::test]
async fn apps_popup_preserves_selected_app_across_refresh() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![
                AppInfo {
                    id: "notion".to_string(),
                    name: "Notion".to_string(),
                    description: Some("Workspace docs".to_string()),
                    logo_url: None,
                    logo_url_dark: None,
                    distribution_channel: None,
                    branding: None,
                    app_metadata: None,
                    labels: None,
                    install_url: Some("https://example.test/notion".to_string()),
                    is_accessible: true,
                    is_enabled: true,
                    plugin_display_names: Vec::new(),
                },
                AppInfo {
                    id: "slack".to_string(),
                    name: "Slack".to_string(),
                    description: Some("Team chat".to_string()),
                    logo_url: None,
                    logo_url_dark: None,
                    distribution_channel: None,
                    branding: None,
                    app_metadata: None,
                    labels: None,
                    install_url: Some("https://example.test/slack".to_string()),
                    is_accessible: true,
                    is_enabled: true,
                    plugin_display_names: Vec::new(),
                },
            ],
        }),
        /*is_final*/ true,
    );
    chat.add_connectors_output();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));

    let before = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        before.contains("› Slack"),
        "expected Slack to be selected before refresh, got:\n{before}"
    );

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![
                AppInfo {
                    id: "airtable".to_string(),
                    name: "Airtable".to_string(),
                    description: Some("Spreadsheets".to_string()),
                    logo_url: None,
                    logo_url_dark: None,
                    distribution_channel: None,
                    branding: None,
                    app_metadata: None,
                    labels: None,
                    install_url: Some("https://example.test/airtable".to_string()),
                    is_accessible: true,
                    is_enabled: true,
                    plugin_display_names: Vec::new(),
                },
                AppInfo {
                    id: "notion".to_string(),
                    name: "Notion".to_string(),
                    description: Some("Workspace docs".to_string()),
                    logo_url: None,
                    logo_url_dark: None,
                    distribution_channel: None,
                    branding: None,
                    app_metadata: None,
                    labels: None,
                    install_url: Some("https://example.test/notion".to_string()),
                    is_accessible: true,
                    is_enabled: true,
                    plugin_display_names: Vec::new(),
                },
                AppInfo {
                    id: "slack".to_string(),
                    name: "Slack".to_string(),
                    description: Some("Team chat".to_string()),
                    logo_url: None,
                    logo_url_dark: None,
                    distribution_channel: None,
                    branding: None,
                    app_metadata: None,
                    labels: None,
                    install_url: Some("https://example.test/slack".to_string()),
                    is_accessible: true,
                    is_enabled: true,
                    plugin_display_names: Vec::new(),
                },
            ],
        }),
        /*is_final*/ true,
    );

    let after = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        after.contains("› Slack"),
        "expected Slack to stay selected after refresh, got:\n{after}"
    );
    assert!(
        !after.contains("› Notion"),
        "did not expect selection to reset to Notion after refresh, got:\n{after}"
    );
}

#[tokio::test]
async fn apps_refresh_failure_with_cached_snapshot_triggers_pending_force_refetch() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);
    chat.connectors_prefetch_in_flight = true;
    chat.connectors_force_refetch_pending = true;

    let full_connectors = vec![AppInfo {
        id: "unit_test_apps_refresh_failure_pending_connector".to_string(),
        name: "Notion".to_string(),
        description: Some("Workspace docs".to_string()),
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: Some("https://example.test/notion".to_string()),
        is_accessible: true,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }];
    chat.connectors_cache = ConnectorsCacheState::Ready(ConnectorsSnapshot {
        connectors: full_connectors.clone(),
    });

    chat.on_connectors_loaded(
        Err("failed to load apps".to_string()),
        /*is_final*/ true,
    );

    assert!(chat.connectors_prefetch_in_flight);
    assert!(!chat.connectors_force_refetch_pending);
    assert_matches!(
        &chat.connectors_cache,
        ConnectorsCacheState::Ready(snapshot) if snapshot.connectors == full_connectors
    );
}

#[tokio::test]
async fn apps_popup_keeps_existing_full_snapshot_while_partial_refresh_loads() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);

    let full_connectors = vec![
        AppInfo {
            id: "unit_test_connector_1".to_string(),
            name: "Notion".to_string(),
            description: Some("Workspace docs".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some("https://example.test/notion".to_string()),
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        },
        AppInfo {
            id: "unit_test_connector_2".to_string(),
            name: "Linear".to_string(),
            description: Some("Project tracking".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some("https://example.test/linear".to_string()),
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        },
    ];
    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: full_connectors.clone(),
        }),
        /*is_final*/ true,
    );
    chat.add_connectors_output();

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![
                AppInfo {
                    id: "unit_test_connector_1".to_string(),
                    name: "Notion".to_string(),
                    description: Some("Workspace docs".to_string()),
                    logo_url: None,
                    logo_url_dark: None,
                    distribution_channel: None,
                    branding: None,
                    app_metadata: None,
                    labels: None,
                    install_url: Some("https://example.test/notion".to_string()),
                    is_accessible: true,
                    is_enabled: true,
                    plugin_display_names: Vec::new(),
                },
                AppInfo {
                    id: "connector_openai_hidden".to_string(),
                    name: "Hidden OpenAI".to_string(),
                    description: Some("Should be filtered".to_string()),
                    logo_url: None,
                    logo_url_dark: None,
                    distribution_channel: None,
                    branding: None,
                    app_metadata: None,
                    labels: None,
                    install_url: Some("https://example.test/hidden-openai".to_string()),
                    is_accessible: true,
                    is_enabled: true,
                    plugin_display_names: Vec::new(),
                },
            ],
        }),
        /*is_final*/ false,
    );

    assert_matches!(
        &chat.connectors_cache,
        ConnectorsCacheState::Ready(snapshot) if snapshot.connectors == full_connectors
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Installed 1 of 2 available apps."),
        "expected popup to keep the last full snapshot while partial refresh loads, got:\n{popup}"
    );
    assert!(
        !popup.contains("Hidden OpenAI"),
        "expected popup to ignore partial refresh rows until the full list arrives, got:\n{popup}"
    );
}

#[tokio::test]
async fn apps_refresh_failure_without_full_snapshot_falls_back_to_installed_apps() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: "unit_test_apps_refresh_failure_fallback_connector".to_string(),
                name: "Notion".to_string(),
                description: Some("Workspace docs".to_string()),
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/notion".to_string()),
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ false,
    );

    chat.add_connectors_output();
    let loading_popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        loading_popup.contains("Loading installed and available apps..."),
        "expected /apps to keep showing loading before the final result, got:\n{loading_popup}"
    );

    chat.on_connectors_loaded(
        Err("failed to load apps".to_string()),
        /*is_final*/ true,
    );

    assert_matches!(
        &chat.connectors_cache,
        ConnectorsCacheState::Ready(snapshot) if snapshot.connectors.len() == 1
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Installed 1 of 1 available apps."),
        "expected /apps to fall back to the installed apps snapshot, got:\n{popup}"
    );
    assert!(
        popup.contains("Installed. Press Enter to open the app page"),
        "expected the fallback popup to behave like the installed apps view, got:\n{popup}"
    );
}

#[tokio::test]
async fn apps_popup_shows_disabled_status_for_installed_but_disabled_apps() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: "connector_1".to_string(),
                name: "Notion".to_string(),
                description: Some("Workspace docs".to_string()),
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/notion".to_string()),
                is_accessible: true,
                is_enabled: false,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ true,
    );

    chat.add_connectors_output();
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Installed · Disabled. Press Enter to open the app page"),
        "expected selected app description to include disabled status, got:\n{popup}"
    );
    assert!(
        popup.contains("enable/disable this app."),
        "expected selected app description to mention enable/disable action, got:\n{popup}"
    );
}

#[tokio::test]
async fn apps_initial_load_applies_enabled_state_from_config() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);

    let temp = tempdir().expect("tempdir");
    let config_toml_path = temp.path().join("config.toml").abs();
    let user_config = toml::from_str::<TomlValue>(
        "[apps.connector_1]\nenabled = false\ndisabled_reason = \"user\"\n",
    )
    .expect("apps config");
    chat.config.config_layer_stack = chat
        .config
        .config_layer_stack
        .with_user_config(&config_toml_path, user_config);

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: "connector_1".to_string(),
                name: "Notion".to_string(),
                description: Some("Workspace docs".to_string()),
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/notion".to_string()),
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ true,
    );

    assert_matches!(
        &chat.connectors_cache,
        ConnectorsCacheState::Ready(snapshot)
            if snapshot
                .connectors
                .iter()
                .find(|connector| connector.id == "connector_1")
                .is_some_and(|connector| !connector.is_enabled)
    );
}

#[tokio::test]
async fn apps_initial_load_applies_enabled_state_from_requirements_with_user_override() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);

    let requirements = ConfigRequirementsToml {
        apps: Some(AppsRequirementsToml {
            apps: BTreeMap::from([(
                "connector_1".to_string(),
                AppRequirementToml {
                    enabled: Some(false),
                },
            )]),
        }),
        ..Default::default()
    };
    let temp = tempdir().expect("tempdir");
    let config_toml_path = temp.path().join("config.toml").abs();
    chat.config.config_layer_stack =
        ConfigLayerStack::new(Vec::new(), ConfigRequirements::default(), requirements)
            .expect("requirements stack")
            .with_user_config(
                &config_toml_path,
                toml::from_str::<TomlValue>(
                    "[apps.connector_1]\nenabled = true\ndisabled_reason = \"user\"\n",
                )
                .expect("apps config"),
            );

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: "connector_1".to_string(),
                name: "Notion".to_string(),
                description: Some("Workspace docs".to_string()),
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/notion".to_string()),
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ true,
    );

    assert_matches!(
        &chat.connectors_cache,
        ConnectorsCacheState::Ready(snapshot)
            if snapshot
                .connectors
                .iter()
                .find(|connector| connector.id == "connector_1")
                .is_some_and(|connector| !connector.is_enabled)
    );

    chat.add_connectors_output();
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Installed · Disabled. Press Enter to open the app page"),
        "expected requirements-disabled connector to render as disabled, got:\n{popup}"
    );
}

#[tokio::test]
async fn apps_initial_load_applies_enabled_state_from_requirements_without_user_entry() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);

    let requirements = ConfigRequirementsToml {
        apps: Some(AppsRequirementsToml {
            apps: BTreeMap::from([(
                "connector_1".to_string(),
                AppRequirementToml {
                    enabled: Some(false),
                },
            )]),
        }),
        ..Default::default()
    };
    chat.config.config_layer_stack =
        ConfigLayerStack::new(Vec::new(), ConfigRequirements::default(), requirements)
            .expect("requirements stack");

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: "connector_1".to_string(),
                name: "Notion".to_string(),
                description: Some("Workspace docs".to_string()),
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/notion".to_string()),
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ true,
    );

    assert_matches!(
        &chat.connectors_cache,
        ConnectorsCacheState::Ready(snapshot)
            if snapshot
                .connectors
                .iter()
                .find(|connector| connector.id == "connector_1")
                .is_some_and(|connector| !connector.is_enabled)
    );

    chat.add_connectors_output();
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Installed · Disabled. Press Enter to open the app page"),
        "expected requirements-disabled connector to render as disabled, got:\n{popup}"
    );
}

#[tokio::test]
async fn apps_refresh_preserves_toggled_enabled_state() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: "connector_1".to_string(),
                name: "Notion".to_string(),
                description: Some("Workspace docs".to_string()),
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/notion".to_string()),
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ true,
    );
    chat.update_connector_enabled("connector_1", /*enabled*/ false);

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: "connector_1".to_string(),
                name: "Notion".to_string(),
                description: Some("Workspace docs".to_string()),
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/notion".to_string()),
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ true,
    );

    assert_matches!(
        &chat.connectors_cache,
        ConnectorsCacheState::Ready(snapshot)
            if snapshot
                .connectors
                .iter()
                .find(|connector| connector.id == "connector_1")
                .is_some_and(|connector| !connector.is_enabled)
    );

    chat.add_connectors_output();
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Installed · Disabled. Press Enter to open the app page"),
        "expected disabled status to persist after reload, got:\n{popup}"
    );
}

#[tokio::test]
async fn apps_popup_for_not_installed_app_uses_install_only_selected_description() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    chat.bottom_pane.set_connectors_enabled(/*enabled*/ true);

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: "connector_2".to_string(),
                name: "Linear".to_string(),
                description: Some("Project tracking".to_string()),
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/linear".to_string()),
                is_accessible: false,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ true,
    );

    chat.add_connectors_output();
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Can be installed. Press Enter to open the app page to install"),
        "expected selected app description to be install-only for not-installed apps, got:\n{popup}"
    );
    assert!(
        !popup.contains("enable/disable this app."),
        "did not expect enable/disable text for not-installed apps, got:\n{popup}"
    );
}

#[tokio::test]
async fn experimental_features_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let features = vec![
        ExperimentalFeatureItem {
            feature: Feature::GhostCommit,
            name: "Ghost snapshots".to_string(),
            description: "Capture undo snapshots each turn.".to_string(),
            enabled: false,
        },
        ExperimentalFeatureItem {
            feature: Feature::ShellTool,
            name: "Shell tool".to_string(),
            description: "Allow the model to run shell commands.".to_string(),
            enabled: true,
        },
    ];
    let view = ExperimentalFeaturesView::new(features, chat.app_event_tx.clone());
    chat.bottom_pane.show_view(Box::new(view));

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("experimental_features_popup", popup);
}

#[tokio::test]
async fn experimental_features_toggle_saves_on_exit() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let expected_feature = Feature::GhostCommit;
    let view = ExperimentalFeaturesView::new(
        vec![ExperimentalFeatureItem {
            feature: expected_feature,
            name: "Ghost snapshots".to_string(),
            description: "Capture undo snapshots each turn.".to_string(),
            enabled: false,
        }],
        chat.app_event_tx.clone(),
    );
    chat.bottom_pane.show_view(Box::new(view));

    chat.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

    assert!(
        rx.try_recv().is_err(),
        "expected no updates until saving the popup"
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let mut updates = None;
    while let Ok(event) = rx.try_recv() {
        if let AppEvent::UpdateFeatureFlags {
            updates: event_updates,
        } = event
        {
            updates = Some(event_updates);
            break;
        }
    }

    let updates = updates.expect("expected UpdateFeatureFlags event");
    assert_eq!(updates, vec![(expected_feature, true)]);
}

#[tokio::test]
async fn experimental_popup_shows_js_repl_node_requirement() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let js_repl_description = FEATURES
        .iter()
        .find(|spec| spec.id == Feature::JsRepl)
        .and_then(|spec| spec.stage.experimental_menu_description())
        .expect("expected js_repl experimental description");
    let node_requirement = js_repl_description
        .split(". ")
        .find(|sentence| sentence.starts_with("Requires Node >= v"))
        .map(|sentence| sentence.trim_end_matches(" installed."))
        .expect("expected js_repl description to mention the Node requirement");

    chat.open_experimental_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        popup.contains(node_requirement),
        "expected js_repl feature description to mention the required Node version, got:\n{popup}"
    );
}

#[tokio::test]
async fn experimental_popup_includes_guardian_approval() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let guardian_stage = FEATURES
        .iter()
        .find(|spec| spec.id == Feature::GuardianApproval)
        .map(|spec| spec.stage)
        .expect("expected guardian approval feature metadata");
    let guardian_name = guardian_stage
        .experimental_menu_name()
        .expect("expected guardian approval experimental menu name");
    let guardian_description = guardian_stage
        .experimental_menu_description()
        .expect("expected guardian approval experimental description");

    chat.open_experimental_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 120);
    let normalized_popup = popup.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        popup.contains(guardian_name),
        "expected guardian approvals entry in experimental popup, got:\n{popup}"
    );
    assert!(
        normalized_popup.contains(guardian_description),
        "expected guardian approvals description in experimental popup, got:\n{popup}"
    );
}

#[tokio::test]
async fn multi_agent_enable_prompt_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_multi_agent_enable_prompt();

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("multi_agent_enable_prompt", popup);
}

#[tokio::test]
async fn multi_agent_enable_prompt_updates_feature_and_emits_notice() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_multi_agent_enable_prompt();
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::UpdateFeatureFlags { updates }) if updates == vec![(Feature::Collab, true)]
    );
    let cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected InsertHistoryCell event, got {other:?}"),
    };
    let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 120));
    assert!(rendered.contains("Subagents will be enabled in the next session."));
}

#[tokio::test]
async fn memories_enable_prompt_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::MemoryTool, /*enabled*/ false);

    chat.open_memories_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("memories_enable_prompt", popup);
}

#[tokio::test]
async fn memories_enable_prompt_updates_feature_without_notice() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::MemoryTool, /*enabled*/ false);

    chat.open_memories_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::UpdateFeatureFlags { updates }) if updates == vec![(Feature::MemoryTool, true)]
    );
    assert!(
        rx.try_recv().is_err(),
        "memory enable prompt should not emit the success notice before persistence succeeds"
    );
}

#[tokio::test]
async fn memories_settings_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::MemoryTool, /*enabled*/ true);
    chat.config.memories.use_memories = true;
    chat.config.memories.generate_memories = false;

    chat.open_memories_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("memories_settings_popup", popup);
}

#[tokio::test]
async fn memories_reset_confirmation_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::MemoryTool, /*enabled*/ true);
    chat.config.memories.use_memories = true;
    chat.config.memories.generate_memories = false;

    chat.open_memories_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("memories_reset_confirmation", popup);
}

#[tokio::test]
async fn memories_settings_toggle_saves_on_enter() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::MemoryTool, /*enabled*/ true);
    chat.config.memories.use_memories = true;
    chat.config.memories.generate_memories = false;

    chat.open_memories_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::UpdateMemorySettings {
            use_memories: true,
            generate_memories: true,
        })
    );
}

#[tokio::test]
async fn memories_reset_confirmation_sends_event_on_confirm() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::MemoryTool, /*enabled*/ true);
    chat.config.memories.use_memories = true;
    chat.config.memories.generate_memories = false;

    chat.open_memories_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    assert_matches!(rx.try_recv(), Ok(AppEvent::ResetMemories));
}

#[tokio::test]
async fn model_selection_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5-codex")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.open_model_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("model_selection_popup", popup);
}

#[tokio::test]
async fn personality_selection_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.2-codex")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.open_personality_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("personality_selection_popup", popup);
}

#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn realtime_audio_selection_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.2-codex")).await;
    chat.open_realtime_audio_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("realtime_audio_selection_popup", popup);
}

#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn realtime_audio_selection_popup_narrow_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.2-codex")).await;
    chat.open_realtime_audio_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 56);
    assert_chatwidget_snapshot!("realtime_audio_selection_popup_narrow", popup);
}

#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn realtime_microphone_picker_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.2-codex")).await;
    chat.config.realtime_audio.microphone = Some("Studio Mic".to_string());
    chat.open_realtime_audio_device_selection_with_names(
        RealtimeAudioDeviceKind::Microphone,
        vec!["Built-in Mic".to_string(), "USB Mic".to_string()],
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("realtime_microphone_picker_popup", popup);
}

#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn realtime_audio_picker_emits_persist_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.2-codex")).await;
    chat.open_realtime_audio_device_selection_with_names(
        RealtimeAudioDeviceKind::Speaker,
        vec!["Desk Speakers".to_string(), "Headphones".to_string()],
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::PersistRealtimeAudioDeviceSelection {
            kind: RealtimeAudioDeviceKind::Speaker,
            name: Some(name),
        }) if name == "Headphones"
    );
}

#[tokio::test]
async fn model_picker_hides_show_in_picker_false_models_from_cache() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("test-visible-model")).await;
    chat.thread_id = Some(ThreadId::new());
    let preset = |slug: &str, show_in_picker: bool| ModelPreset {
        id: slug.to_string(),
        model: slug.to_string(),
        display_name: slug.to_string(),
        description: format!("{slug} description"),
        default_reasoning_effort: ReasoningEffortConfig::Medium,
        supported_reasoning_efforts: vec![ReasoningEffortPreset {
            effort: ReasoningEffortConfig::Medium,
            description: "medium".to_string(),
        }],
        supports_personality: false,
        additional_speed_tiers: Vec::new(),
        is_default: false,
        upgrade: None,
        show_in_picker,
        availability_nux: None,
        supported_in_api: true,
        input_modalities: default_input_modalities(),
    };

    chat.open_model_popup_with_presets(vec![
        preset("test-visible-model", true),
        preset("test-hidden-model", false),
    ]);
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("model_picker_filters_hidden_models", popup);
    assert!(
        popup.contains("test-visible-model"),
        "expected visible model to appear in picker:\n{popup}"
    );
    assert!(
        !popup.contains("test-hidden-model"),
        "expected hidden model to be excluded from picker:\n{popup}"
    );
}

#[tokio::test]
async fn server_overloaded_error_does_not_switch_models() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.2-codex")).await;
    chat.set_model("gpt-5.2-codex");
    while rx.try_recv().is_ok() {}
    while op_rx.try_recv().is_ok() {}

    chat.handle_codex_event(Event {
        id: "err-1".to_string(),
        msg: EventMsg::Error(ErrorEvent {
            message: "server overloaded".to_string(),
            codex_error_info: Some(CodexErrorInfo::ServerOverloaded),
        }),
    });

    while let Ok(event) = rx.try_recv() {
        if let AppEvent::UpdateModel(model) = event {
            assert_eq!(
                model, "gpt-5.2-codex",
                "did not expect model switch on server-overloaded error"
            );
        }
    }

    while let Ok(event) = op_rx.try_recv() {
        if let Op::OverrideTurnContext { model, .. } = event {
            assert!(
                model.is_none(),
                "did not expect OverrideTurnContext model update on server-overloaded error"
            );
        }
    }
}

#[tokio::test]
async fn model_reasoning_selection_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;

    set_chatgpt_auth(&mut chat);
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::High));

    let preset = get_available_model(&chat, "gpt-5.1-codex-max");
    chat.open_reasoning_popup(preset);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("model_reasoning_selection_popup", popup);
}

#[tokio::test]
async fn model_reasoning_selection_popup_extra_high_warning_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;

    set_chatgpt_auth(&mut chat);
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::XHigh));

    let preset = get_available_model(&chat, "gpt-5.1-codex-max");
    chat.open_reasoning_popup(preset);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("model_reasoning_selection_popup_extra_high_warning", popup);
}

#[tokio::test]
async fn reasoning_popup_shows_extra_high_with_space() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;

    set_chatgpt_auth(&mut chat);

    let preset = get_available_model(&chat, "gpt-5.1-codex-max");
    chat.open_reasoning_popup(preset);

    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        popup.contains("Extra high"),
        "expected popup to include 'Extra high'; popup: {popup}"
    );
    assert!(
        !popup.contains("Extrahigh"),
        "expected popup not to include 'Extrahigh'; popup: {popup}"
    );
}

#[tokio::test]
async fn single_reasoning_option_skips_selection() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let single_effort = vec![ReasoningEffortPreset {
        effort: ReasoningEffortConfig::High,
        description: "Greater reasoning depth for complex or ambiguous problems".to_string(),
    }];
    let preset = ModelPreset {
        id: "model-with-single-reasoning".to_string(),
        model: "model-with-single-reasoning".to_string(),
        display_name: "model-with-single-reasoning".to_string(),
        description: "".to_string(),
        default_reasoning_effort: ReasoningEffortConfig::High,
        supported_reasoning_efforts: single_effort,
        supports_personality: false,
        additional_speed_tiers: Vec::new(),
        is_default: false,
        upgrade: None,
        show_in_picker: true,
        availability_nux: None,
        supported_in_api: true,
        input_modalities: default_input_modalities(),
    };
    chat.open_reasoning_popup(preset);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        !popup.contains("Select Reasoning Level"),
        "expected reasoning selection popup to be skipped"
    );

    let mut events = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        events.push(ev);
    }

    assert!(
        events
            .iter()
            .any(|ev| matches!(ev, AppEvent::UpdateReasoningEffort(Some(effort)) if *effort == ReasoningEffortConfig::High)),
        "expected reasoning effort to be applied automatically; events: {events:?}"
    );
}

#[tokio::test]
async fn feedback_selection_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Open the feedback category selection popup via slash command.
    chat.dispatch_command(SlashCommand::Feedback);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("feedback_selection_popup", popup);
}

#[tokio::test]
async fn feedback_upload_consent_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.show_selection_view(crate::bottom_pane::feedback_upload_consent_params(
        chat.app_event_tx.clone(),
        crate::app_event::FeedbackCategory::Bug,
        chat.current_rollout_path.clone(),
        &codex_feedback::FeedbackDiagnostics::new(vec![codex_feedback::FeedbackDiagnostic {
            headline: "Proxy environment variables are set and may affect connectivity."
                .to_string(),
            details: vec!["HTTPS_PROXY = hello".to_string()],
        }]),
    ));

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("feedback_upload_consent_popup", popup);
}

#[tokio::test]
async fn feedback_good_result_consent_popup_includes_connectivity_diagnostics_filename() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.show_selection_view(crate::bottom_pane::feedback_upload_consent_params(
        chat.app_event_tx.clone(),
        crate::app_event::FeedbackCategory::GoodResult,
        chat.current_rollout_path.clone(),
        &codex_feedback::FeedbackDiagnostics::new(vec![codex_feedback::FeedbackDiagnostic {
            headline: "Proxy environment variables are set and may affect connectivity."
                .to_string(),
            details: vec!["HTTPS_PROXY = hello".to_string()],
        }]),
    ));

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("feedback_good_result_consent_popup", popup);
}

#[tokio::test]
async fn reasoning_popup_escape_returns_to_model_popup() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.1-codex-max")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.open_model_popup();

    let preset = get_available_model(&chat, "gpt-5.1-codex-max");
    chat.open_reasoning_popup(preset);

    let before_escape = render_bottom_popup(&chat, /*width*/ 80);
    assert!(before_escape.contains("Select Reasoning Level"));

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    let after_escape = render_bottom_popup(&chat, /*width*/ 80);
    assert!(after_escape.contains("Select Model"));
    assert!(!after_escape.contains("Select Reasoning Level"));
}
