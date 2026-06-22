use super::*;
use codex_app_server_protocol::PluginShareContext;
use codex_app_server_protocol::PluginShareDiscoverability;
use codex_app_server_protocol::PluginUninstallResponse;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn plugins_popup_workspace_remote_row_opens_remote_detail() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![plugins_test_remote_marketplace(
            "workspace-directory",
            "Workspace",
            vec![plugins_test_remote_summary(
                "plugins~Plugin_calendar",
                "calendar",
                Some("Calendar"),
                Some("Workspace schedules."),
                /*installed*/ false,
            )],
        )]),
    );
    let popup = select_plugins_tab_containing(&mut chat, /*width*/ 100, "[Workspace]");
    let remote_row = popup
        .lines()
        .find(|line| line.contains("Calendar"))
        .expect("expected remote plugin row");
    insta::assert_snapshot!(
        remote_row,
        @"› [-] Calendar   Available   Press Enter to install or view plugin details."
    );

    while rx.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    match rx.try_recv() {
        Ok(AppEvent::OpenPluginDetailLoading {
            plugin_display_name,
        }) => {
            assert_eq!(plugin_display_name, "Calendar");
        }
        other => panic!("expected OpenPluginDetailLoading event, got {other:?}"),
    }
    match rx.try_recv() {
        Ok(AppEvent::FetchPluginDetail { cwd: _, params }) => {
            assert_eq!(params.marketplace_path, None);
            assert_eq!(
                params.remote_marketplace_name,
                Some("workspace-directory".to_string())
            );
            assert_eq!(params.plugin_name, "plugins~Plugin_calendar");
        }
        other => panic!("expected FetchPluginDetail event, got {other:?}"),
    }
}

#[tokio::test]
async fn plugins_popup_workspace_and_shared_with_me_tabs_use_product_labels() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let response = plugins_test_response(vec![
        plugins_test_remote_marketplace(
            "workspace-directory",
            "Raw Workspace Directory",
            vec![plugins_test_remote_summary(
                "plugins~Plugin_buildkite",
                "buildkite",
                Some("Buildkite"),
                Some("Workspace CI."),
                /*installed*/ false,
            )],
        ),
        plugins_test_remote_marketplace(
            "workspace-shared-with-me-private",
            "Raw Shared Private",
            vec![plugins_test_remote_summary(
                "plugins~Plugin_docs",
                "docs",
                Some("Docs"),
                Some("Shared docs."),
                /*installed*/ false,
            )],
        ),
        plugins_test_remote_marketplace(
            "workspace-shared-with-me-unlisted",
            "Raw Shared Link",
            vec![plugins_test_remote_summary(
                "plugins~Plugin_link",
                "link",
                Some("Link Share"),
                Some("Shared by link."),
                /*installed*/ false,
            )],
        ),
    ]);
    render_loaded_plugins_popup(&mut chat, response);
    for _ in 0..3 {
        chat.handle_key_event(KeyEvent::from(KeyCode::Right));
    }
    let workspace_popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        workspace_popup.contains("Workspace.")
            && workspace_popup.contains("Buildkite")
            && !workspace_popup.contains("Raw Workspace Directory."),
        "expected workspace tab to use product label, got:\n{workspace_popup}"
    );
    let workspace_row = workspace_popup
        .lines()
        .find(|line| line.contains("Buildkite"))
        .expect("expected workspace plugin row");
    let workspace_row = workspace_row
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    insta::assert_snapshot!(
        workspace_row,
        @"› [-] Buildkite Available Press Enter to install or view plugin details."
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Right));
    let shared_popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        shared_popup.contains("Shared with me.")
            && shared_popup.contains("Docs")
            && !shared_popup.contains("Raw Shared Private."),
        "expected shared-with-me tab to use product label, got:\n{shared_popup}"
    );
    let shared_row = shared_popup
        .lines()
        .find(|line| line.contains("Docs"))
        .expect("expected shared-with-me plugin row");
    let shared_row = shared_row.split_whitespace().collect::<Vec<_>>().join(" ");
    insta::assert_snapshot!(
        shared_row,
        @"› [-] Docs Available Press Enter to install or view plugin details."
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Right));
    let shared_link_popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        shared_link_popup.contains("Shared with me (link).")
            && shared_link_popup.contains("Link Share")
            && !shared_link_popup.contains("Raw Shared Link."),
        "expected shared-with-me link tab to use product label, got:\n{shared_link_popup}"
    );
    let shared_link_row = shared_link_popup
        .lines()
        .find(|line| line.contains("Link Share"))
        .expect("expected shared-with-me link plugin row");
    let shared_link_row = shared_link_row
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    insta::assert_snapshot!(
        shared_link_row,
        @"› [-] Link Share Available Press Enter to install or view plugin details."
    );
}

#[tokio::test]
async fn plugins_popup_personal_marketplace_uses_local_product_label() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let response = plugins_test_response(vec![PluginMarketplaceEntry {
        name: "codex-curated".to_string(),
        path: Some(plugins_test_personal_marketplace_path()),
        interface: Some(MarketplaceInterface {
            display_name: Some("Personal".to_string()),
        }),
        plugins: vec![plugins_test_summary(
            "plugin-local-docs",
            "local-docs",
            Some("Local Docs"),
            Some("Local editable docs."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        )],
    }]);
    render_loaded_plugins_popup(&mut chat, response);
    let popup = select_plugins_tab_containing(&mut chat, /*width*/ 120, "Local.");
    assert!(
        popup.contains("Local.") && popup.contains("Local Docs") && !popup.contains("Personal."),
        "expected personal marketplace to use Local product label, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugins_popup_shared_with_me_disabled_by_feature_flag_shows_section_error() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);
    chat.set_feature_enabled(Feature::PluginSharing, /*enabled*/ false);

    chat.add_plugins_output();
    let cwd = chat.config.cwd.clone();
    chat.on_plugins_loaded(
        cwd.to_path_buf(),
        Ok(plugins_test_response(vec![
            plugins_test_curated_marketplace(Vec::new()),
        ])),
    );
    chat.on_plugin_remote_sections_loaded(
        cwd.to_path_buf(),
        Vec::new(),
        vec![crate::app_event::PluginRemoteSectionError {
            section_id: "shared-with-me".to_string(),
            label: "Shared with me".to_string(),
            message: "Plugin sharing is disabled for this Codex session. Enable plugin sharing to load shared plugins.".to_string(),
        }],
    );

    for _ in 0..4 {
        chat.handle_key_event(KeyEvent::from(KeyCode::Right));
    }
    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        popup.contains("Shared with me unavailable")
            && popup.contains("Plugin sharing is disabled for this Codex session."),
        "expected disabled sharing section error, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugins_popup_preserves_remote_section_tab_after_loading_finishes() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    chat.add_plugins_output();
    let cwd = chat.config.cwd.clone();
    chat.on_plugins_loaded(
        cwd.to_path_buf(),
        Ok(plugins_test_response(vec![
            plugins_test_curated_marketplace(Vec::new()),
        ])),
    );
    let loading_popup =
        select_plugins_tab_containing(&mut chat, /*width*/ 100, "Loading Workspace plugins.");
    assert!(
        loading_popup.contains("Loading Workspace plugins."),
        "expected Workspace loading tab before remote sections resolve, got:\n{loading_popup}"
    );

    chat.on_plugin_remote_sections_loaded(
        cwd.to_path_buf(),
        vec![plugins_test_remote_marketplace(
            "workspace-directory",
            "Raw Workspace Directory",
            vec![plugins_test_remote_summary(
                "plugins~Plugin_buildkite",
                "buildkite",
                Some("Buildkite"),
                Some("Buildkite pipelines."),
                /*installed*/ false,
            )],
        )],
        Vec::new(),
    );

    let workspace_popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        workspace_popup.contains("Workspace.")
            && workspace_popup.contains("Buildkite")
            && !workspace_popup.contains("Loading Workspace plugins."),
        "expected remote section refresh to keep the Workspace tab active, got:\n{workspace_popup}"
    );
}

#[tokio::test]
async fn open_plugins_list_preserves_saved_workspace_tab() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let response = plugins_test_response(vec![
        plugins_test_curated_marketplace(Vec::new()),
        plugins_test_remote_marketplace(
            "workspace-directory",
            "Raw Workspace Directory",
            vec![plugins_test_remote_summary(
                "plugins~Plugin_buildkite",
                "buildkite",
                Some("Buildkite"),
                Some("Buildkite pipelines."),
                /*installed*/ false,
            )],
        ),
    ]);
    render_loaded_plugins_popup(&mut chat, response.clone());
    for _ in 0..3 {
        chat.handle_key_event(KeyEvent::from(KeyCode::Right));
    }
    chat.open_plugin_detail_loading_popup("Buildkite");
    let cwd = chat.config.cwd.to_path_buf();
    chat.open_plugins_list(cwd, response);

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        popup.contains("Workspace.") && popup.contains("Buildkite"),
        "expected Back to plugins to preserve the Workspace tab, got:\n{popup}"
    );
}

#[tokio::test]
async fn plugins_popup_installed_remote_row_keeps_remote_detail() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let remote_plugin_id = "plugins~Plugin_docs";
    let local_summary = PluginSummary {
        share_context: Some(PluginShareContext {
            remote_plugin_id: remote_plugin_id.to_string(),
            remote_version: Some("3".to_string()),
            discoverability: Some(PluginShareDiscoverability::Private),
            share_url: Some("https://chatgpt.com/codex/plugins/share/docs".to_string()),
            creator_account_user_id: None,
            creator_name: Some("Test User".to_string()),
            share_principals: None,
        }),
        ..plugins_test_summary(
            "plugin-docs",
            "docs",
            Some("Docs"),
            Some("Local editable docs plugin."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        )
    };
    let popup = render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![
            plugins_test_curated_marketplace(vec![local_summary]),
            plugins_test_remote_marketplace(
                "workspace-shared-with-me-private",
                "Shared with me",
                vec![plugins_test_remote_summary(
                    remote_plugin_id,
                    "docs",
                    Some("Docs"),
                    Some("Shared docs plugin."),
                    /*installed*/ true,
                )],
            ),
        ]),
    );
    let all_plugins_row = popup
        .lines()
        .find(|line| line.contains("Docs"))
        .expect("expected all-plugins row");
    assert!(
        popup.contains("Installed 1 of 1 available plugins."),
        "expected header count to reflect deduped plugin rows, got:\n{popup}"
    );
    assert!(
        all_plugins_row.contains("Installed") && !all_plugins_row.contains("Available"),
        "expected installed remote duplicate to win over local mapped share, got:\n{all_plugins_row}"
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Right));
    let installed_popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        installed_popup.contains("Showing 1 installed plugins.")
            && installed_popup.contains("Docs"),
        "expected installed remote duplicate in the Installed tab, got:\n{installed_popup}"
    );

    while rx.try_recv().is_ok() {}
    select_plugins_tab_containing(&mut chat, /*width*/ 100, "[Shared with me]");
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    match rx.try_recv() {
        Ok(AppEvent::OpenPluginDetailLoading {
            plugin_display_name,
        }) => {
            assert_eq!(plugin_display_name, "Docs");
        }
        other => panic!("expected OpenPluginDetailLoading event, got {other:?}"),
    }
    match rx.try_recv() {
        Ok(AppEvent::FetchPluginDetail { params, .. }) => {
            assert_eq!(params.marketplace_path, None);
            assert_eq!(
                params.remote_marketplace_name,
                Some("workspace-shared-with-me-private".to_string())
            );
            assert_eq!(params.plugin_name, remote_plugin_id);
        }
        other => panic!("expected FetchPluginDetail event, got {other:?}"),
    }
}

#[tokio::test]
async fn plugins_popup_remote_local_dedupe_prefers_installed_remote_after_mapped_shares() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let remote_plugin_id = "plugins~Plugin_docs";
    let local_summary = PluginSummary {
        remote_plugin_id: Some(remote_plugin_id.to_string()),
        ..plugins_test_summary(
            "plugin-docs",
            "docs",
            Some("Docs"),
            Some("Local curated docs plugin."),
            /*installed*/ false,
            /*enabled*/ true,
            PluginInstallPolicy::Available,
        )
    };
    let cwd = chat.config.cwd.clone();
    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![plugins_test_curated_marketplace(vec![local_summary])]),
    );
    chat.on_plugin_remote_sections_loaded(
        cwd.to_path_buf(),
        vec![plugins_test_remote_marketplace(
            "openai-curated-remote",
            "Remote curated",
            vec![plugins_test_remote_summary(
                remote_plugin_id,
                "docs",
                Some("Docs"),
                Some("Remote installed docs plugin."),
                /*installed*/ true,
            )],
        )],
        Vec::new(),
    );
    let popup = render_bottom_popup(&chat, /*width*/ 100);
    let PluginsCacheState::Ready(response) = &chat.plugins_cache else {
        panic!("expected cached plugins after remote section refresh");
    };
    assert_eq!(
        response
            .marketplaces
            .iter()
            .map(|marketplace| marketplace.name.as_str())
            .collect::<Vec<_>>(),
        vec!["openai-curated-remote"]
    );
    let all_plugins_row = popup
        .lines()
        .find(|line| line.contains("Docs"))
        .expect("expected all-plugins row");
    assert!(
        popup.contains("Installed 1 of 1 available plugins."),
        "expected header count to reflect deduped plugin rows, got:\n{popup}"
    );
    assert!(
        all_plugins_row.contains("Installed")
            && !all_plugins_row.contains("Local curated docs plugin."),
        "expected installed remote duplicate to win when local row is not a mapped share, got:\n{all_plugins_row}"
    );

    while rx.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    match rx.try_recv() {
        Ok(AppEvent::OpenPluginDetailLoading {
            plugin_display_name,
        }) => {
            assert_eq!(plugin_display_name, "Docs");
        }
        other => panic!("expected OpenPluginDetailLoading event, got {other:?}"),
    }
    match rx.try_recv() {
        Ok(AppEvent::FetchPluginDetail { params, .. }) => {
            assert_eq!(params.marketplace_path, None);
            assert_eq!(
                params.remote_marketplace_name,
                Some("openai-curated-remote".to_string())
            );
            assert_eq!(params.plugin_name, "plugins~Plugin_docs");
        }
        other => panic!("expected FetchPluginDetail event, got {other:?}"),
    }
}

#[tokio::test]
async fn plugin_detail_remote_shared_plugin_uses_install_flow() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let summary = PluginSummary {
        share_context: Some(PluginShareContext {
            remote_plugin_id: "plugins~Plugin_linear".to_string(),
            remote_version: Some("5".to_string()),
            discoverability: Some(PluginShareDiscoverability::Private),
            share_url: Some("https://chatgpt.com/codex/plugins/share/linear".to_string()),
            creator_account_user_id: None,
            creator_name: Some("Test User".to_string()),
            share_principals: None,
        }),
        ..plugins_test_remote_summary(
            "plugins~Plugin_linear",
            "linear",
            Some("Linear"),
            Some("Issue tracking."),
            /*installed*/ false,
        )
    };
    let response = plugins_test_response(vec![plugins_test_remote_marketplace(
        "workspace-shared-with-me-private",
        "Shared with me",
        vec![summary.clone()],
    )]);
    let cwd = chat.config.cwd.clone();
    chat.on_plugins_loaded(cwd.to_path_buf(), Ok(response));
    chat.add_plugins_output();
    chat.on_plugin_detail_loaded(
        cwd.to_path_buf(),
        Ok(PluginReadResponse {
            plugin: plugins_test_remote_detail(
                "workspace-shared-with-me-private",
                summary,
                Some("Install shared Linear plugin."),
            ),
        }),
    );
    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert_chatwidget_snapshot!(
        "plugin_detail_popup_remote_shared_installable",
        strip_osc8_for_snapshot(&popup)
    );

    while rx.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    match rx.try_recv() {
        Ok(AppEvent::OpenPluginInstallLoading {
            plugin_display_name,
        }) => {
            assert_eq!(plugin_display_name, "Linear");
        }
        other => panic!("expected OpenPluginInstallLoading event, got {other:?}"),
    }
    match rx.try_recv() {
        Ok(AppEvent::FetchPluginInstall {
            cwd: _,
            location: crate::app_event::PluginLocation::Remote { marketplace_name },
            plugin_name,
            plugin_display_name,
        }) => {
            assert_eq!(marketplace_name, "workspace-shared-with-me-private");
            assert_eq!(plugin_name, "plugins~Plugin_linear");
            assert_eq!(plugin_display_name, "Linear");
        }
        other => panic!("expected remote FetchPluginInstall event, got {other:?}"),
    }
}

#[tokio::test]
async fn plugin_detail_not_installable_plugin_disables_install_action() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let summary = plugins_test_summary(
        "plugin-internal",
        "internal",
        Some("Internal"),
        Some("Internal only."),
        /*installed*/ false,
        /*enabled*/ true,
        PluginInstallPolicy::NotAvailable,
    );
    let cwd = chat.config.cwd.clone();
    chat.on_plugins_loaded(
        cwd.to_path_buf(),
        Ok(plugins_test_response(vec![
            plugins_test_curated_marketplace(vec![summary.clone()]),
        ])),
    );
    chat.add_plugins_output();
    chat.on_plugin_detail_loaded(
        cwd.to_path_buf(),
        Ok(PluginReadResponse {
            plugin: plugins_test_detail(summary, Some("Internal only."), &[], &[], &[], &[]),
        }),
    );

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    let install_row = popup
        .lines()
        .find(|line| line.contains("Install plugin"))
        .expect("expected install row");
    assert!(
        install_row.contains("This plugin is not installable from this marketplace."),
        "expected disabled not-installable row, got:\n{install_row}"
    );

    while rx.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    while let Ok(event) = rx.try_recv() {
        assert!(
            !matches!(
                event,
                AppEvent::OpenPluginInstallLoading { .. } | AppEvent::FetchPluginInstall { .. }
            ),
            "expected Enter on the disabled install row to emit no install action, got {event:?}"
        );
    }
    assert_eq!(render_bottom_popup(&chat, /*width*/ 100), popup);
}

#[tokio::test]
async fn plugin_uninstall_refresh_response_updates_cached_plugin_list() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let cwd = chat.config.cwd.clone();
    chat.on_plugins_loaded(
        cwd.to_path_buf(),
        Ok(plugins_test_response(vec![
            plugins_test_curated_marketplace(vec![plugins_test_summary(
                "plugin-docs",
                "docs",
                Some("Docs"),
                Some("Workspace docs."),
                /*installed*/ true,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            )]),
        ])),
    );
    chat.add_plugins_output();

    chat.open_plugin_uninstall_loading_popup("Docs");
    chat.on_plugin_uninstall_loaded(
        cwd.to_path_buf(),
        "Docs".to_string(),
        Ok(PluginUninstallResponse {}),
    );
    chat.on_plugins_loaded(
        cwd.to_path_buf(),
        Ok(plugins_test_response(vec![
            plugins_test_curated_marketplace(vec![plugins_test_summary(
                "plugin-docs",
                "docs",
                Some("Docs"),
                Some("Workspace docs."),
                /*installed*/ false,
                /*enabled*/ true,
                PluginInstallPolicy::Available,
            )]),
        ])),
    );
    let popup = render_bottom_popup(&chat, /*width*/ 120);
    let plugin_row = popup
        .lines()
        .find(|line| line.contains("Docs"))
        .expect("expected Docs plugin row");
    assert!(
        popup.contains("Installed 0 of 1 available plugins.")
            && plugin_row.contains("Available")
            && !plugin_row.contains("Installed"),
        "expected uninstall success to update the cached plugin list, got:\n{popup}"
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Right));
    let installed_popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        installed_popup.contains("Showing 0 installed plugins.")
            && installed_popup.contains("No installed plugins")
            && !installed_popup.contains("Docs"),
        "expected uninstalled plugin to be absent from the Installed tab, got:\n{installed_popup}"
    );
}
