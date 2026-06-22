use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn plugins_popup_uses_product_labels_for_remote_and_personal_tabs() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    render_loaded_plugins_popup(
        &mut chat,
        plugins_test_response(vec![
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
            PluginMarketplaceEntry {
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
            },
        ]),
    );

    let rows = [
        (
            "[Workspace]",
            "Workspace.",
            "Buildkite",
            "Raw Workspace Directory.",
        ),
        (
            "[Shared with me]",
            "Shared with me.",
            "Docs",
            "Raw Shared Private.",
        ),
        (
            "[Shared with me (link)]",
            "Shared with me (link).",
            "Link Share",
            "Raw Shared Link.",
        ),
        ("[Local]", "Local.", "Local Docs", "Personal."),
    ]
    .into_iter()
    .map(|(selected_tab, product_label, plugin_name, raw_label)| {
        let popup = select_plugins_tab_containing(&mut chat, /*width*/ 120, selected_tab);
        assert!(
            popup.contains(product_label)
                && popup.contains(plugin_name)
                && !popup.contains(raw_label),
            "expected {selected_tab} to use its product label, got:\n{popup}"
        );
        popup
            .lines()
            .find(|line| line.contains(plugin_name))
            .expect("expected plugin row")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    })
    .collect::<Vec<_>>()
    .join("\n");

    insta::assert_snapshot!(
        rows,
        @r###"
        › [-] Buildkite Available Press Enter to install or view plugin details.
        › [-] Docs Available Press Enter to install or view plugin details.
        › [-] Link Share Available Press Enter to install or view plugin details.
        › [-] Local Docs Available Press Enter to install or view plugin details.
        "###
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
async fn plugins_popup_preserves_workspace_tab_across_load_and_detail_navigation() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Plugins, /*enabled*/ true);

    let workspace_marketplace = plugins_test_remote_marketplace(
        "workspace-directory",
        "Raw Workspace Directory",
        vec![plugins_test_remote_summary(
            "plugins~Plugin_buildkite",
            "buildkite",
            Some("Buildkite"),
            Some("Buildkite pipelines."),
            /*installed*/ false,
        )],
    );
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
        vec![workspace_marketplace.clone()],
        Vec::new(),
    );
    let workspace_popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        workspace_popup.contains("Workspace.")
            && workspace_popup.contains("Buildkite")
            && !workspace_popup.contains("Loading Workspace plugins."),
        "expected remote section refresh to keep the Workspace tab active, got:\n{workspace_popup}"
    );

    chat.open_plugin_detail_loading_popup("Buildkite");
    chat.open_plugins_list(
        cwd.to_path_buf(),
        plugins_test_response(vec![
            plugins_test_curated_marketplace(Vec::new()),
            workspace_marketplace,
        ]),
    );
    let reopened_popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        reopened_popup.contains("Workspace.") && reopened_popup.contains("Buildkite"),
        "expected Back to plugins to preserve the Workspace tab, got:\n{reopened_popup}"
    );
}

#[tokio::test]
async fn plugins_popup_remote_local_dedupe_prefers_installed_remote_after_mapped_shares() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
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
}

#[tokio::test]
async fn plugin_detail_not_installable_plugin_disables_install_action() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
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

    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    assert_eq!(
        render_bottom_popup(&chat, /*width*/ 100),
        popup,
        "expected navigation to skip the disabled install row"
    );
}
