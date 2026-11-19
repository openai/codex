//! Regression coverage for recommendation invalidation when installed snapshots are published.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn installed_snapshot_changes_invalidate_only_the_affected_recommendations() {
    let codex_home = TempDir::new().unwrap();
    write_cached_plugin(
        codex_home.path(),
        REMOTE_WORKSPACE_MARKETPLACE_NAME,
        "linear",
    );
    let server = MockServer::start().await;
    let mut config = load_config(codex_home.path(), codex_home.path()).await;
    config.chatgpt_base_url = server.uri();
    let auth_manager = test_auth_manager(Some(AuthMode::Chatgpt));
    let auth = auth_manager.auth_cached().expect("test ChatGPT auth");
    let other_auth = CodexAuth::from_external_chatgpt_tokens(
        "header.e30.other",
        "other-account",
        /*chatgpt_plan_type*/ None,
    )
    .expect("build second ChatGPT auth");
    let manager = test_plugins_manager_with_auth_manager(
        codex_home.path().to_path_buf(),
        Some(Product::Codex),
        auth_manager,
    );
    let installed_state = Arc::new(std::sync::Mutex::new(Some(true)));
    let response_state = Arc::clone(&installed_state);
    Mock::given(method("GET"))
        .and(path("/ps/plugins/installed"))
        .respond_with(move |_: &wiremock::Request| {
            let plugins = (*response_state.lock().unwrap())
                .into_iter()
                .map(|enabled| {
                    serde_json::json!({
                        "id": "plugins~Plugin_linear",
                        "name": "linear",
                        "scope": "WORKSPACE",
                        "discoverability": "LISTED",
                        "installation_policy": "AVAILABLE",
                        "authentication_policy": "ON_USE",
                        "release": {
                            "version": "local",
                            "display_name": "Linear",
                            "description": "Test plugin",
                            "interface": {},
                        },
                        "enabled": enabled,
                    })
                })
                .collect::<Vec<_>>();
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "plugins": plugins,
                "pagination": {"next_page_token": null},
            }))
        })
        .expect(8)
        .mount(&server)
        .await;
    let suggested_response = ResponseTemplate::new(200)
        .set_body_json(serde_json::json!({"enabled": true, "plugins": []}));
    let suggested = Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested/codex"))
        .and(header("chatgpt-account-id", auth.get_account_id().unwrap()))
        .respond_with(suggested_response.clone())
        .expect(5)
        .mount_as_scoped(&server)
        .await;
    let other_suggested = Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested/codex"))
        .and(header(
            "chatgpt-account-id",
            other_auth.get_account_id().unwrap(),
        ))
        .respond_with(suggested_response)
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let expected = RecommendedPluginsMode::Endpoint {
        plugins: Vec::new(),
    };
    let read_recommendations = async || {
        assert_eq!(
            manager
                .recommended_plugins_mode_for_config(&config, Some(&auth))
                .await,
            expected
        );
    };
    read_recommendations().await;
    assert_eq!(
        manager
            .recommended_plugins_mode_for_config(&config, Some(&other_auth))
            .await,
        expected
    );

    for (state, expected_reads) in [
        (Some(true), 2),
        (Some(true), 2),
        (Some(false), 3),
        (Some(false), 3),
    ] {
        *installed_state.lock().unwrap() = state;
        manager
            .reconcile_remote_installed_plugins(&config, Some(&auth))
            .await
            .expect("reconcile installed plugins");
        read_recommendations().await;
        assert_eq!(suggested.received_requests().await.len(), expected_reads);
    }

    *installed_state.lock().unwrap() = Some(true);
    manager
        .build_and_cache_remote_installed_plugin_marketplaces_with_display_assets(
            &config,
            Some(&auth),
            &[],
            /*on_effective_plugins_changed*/ None,
            InstalledPluginDisplayAssets::Skip,
        )
        .await
        .expect("refresh installed metadata before reconciliation");
    read_recommendations().await;
    assert_eq!(suggested.received_requests().await.len(), 4);
    manager
        .reconcile_remote_installed_plugins(&config, Some(&auth))
        .await
        .expect("reconcile the already published state");
    read_recommendations().await;
    assert_eq!(suggested.received_requests().await.len(), 4);

    for expected_reads in [5, 5] {
        *installed_state.lock().unwrap() = None;
        manager
            .reconcile_remote_installed_plugins(&config, Some(&auth))
            .await
            .expect("reconcile removed plugin");
        read_recommendations().await;
        assert_eq!(suggested.received_requests().await.len(), expected_reads);
    }
    assert_eq!(
        manager
            .recommended_plugins_mode_for_config(&config, Some(&other_auth))
            .await,
        expected
    );
    assert_eq!(other_suggested.received_requests().await.len(), 1);
    server.verify().await;
}

#[tokio::test]
async fn installed_snapshot_backend_switch_invalidates_only_the_new_backend() {
    let codex_home = TempDir::new().unwrap();
    let first_server = MockServer::start().await;
    let second_server = MockServer::start().await;
    let mut first_config = load_config(codex_home.path(), codex_home.path()).await;
    first_config.chatgpt_base_url = first_server.uri();
    let mut second_config = first_config.clone();
    second_config.chatgpt_base_url = second_server.uri();
    let auth_manager = test_auth_manager(Some(AuthMode::Chatgpt));
    let auth = auth_manager.auth_cached().expect("test ChatGPT auth");
    let manager = test_plugins_manager_with_auth_manager(
        codex_home.path().to_path_buf(),
        Some(Product::Codex),
        auth_manager,
    );
    let installed_response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "plugins": [],
        "pagination": {"next_page_token": null},
    }));
    for (server, calls) in [(&first_server, 1), (&second_server, 2)] {
        Mock::given(method("GET"))
            .and(path("/ps/plugins/installed"))
            .respond_with(installed_response.clone())
            .expect(calls)
            .mount(server)
            .await;
    }
    let suggested_response = ResponseTemplate::new(200)
        .set_body_json(serde_json::json!({"enabled": true, "plugins": []}));
    let mut suggested = Vec::new();
    for (server, calls) in [(&first_server, 1), (&second_server, 2)] {
        suggested.push(
            Mock::given(method("GET"))
                .and(path("/ps/plugins/suggested/codex"))
                .respond_with(suggested_response.clone())
                .expect(calls)
                .mount_as_scoped(server)
                .await,
        );
    }
    let expected = RecommendedPluginsMode::Endpoint {
        plugins: Vec::new(),
    };
    assert_eq!(
        manager
            .recommended_plugins_mode_for_config(&second_config, Some(&auth))
            .await,
        expected
    );
    let publish = async |config: &PluginsConfigInput| {
        manager
            .build_and_cache_remote_installed_plugin_marketplaces_with_display_assets(
                config,
                Some(&auth),
                &[],
                /*on_effective_plugins_changed*/ None,
                InstalledPluginDisplayAssets::Skip,
            )
            .await
            .expect("publish installed snapshot");
    };
    publish(&first_config).await;
    assert_eq!(
        manager
            .recommended_plugins_mode_for_config(&first_config, Some(&auth))
            .await,
        expected
    );
    publish(&second_config).await;
    for unchanged in [false, true] {
        if unchanged {
            publish(&second_config).await;
        }
        assert_eq!(
            manager
                .recommended_plugins_mode_for_config(&second_config, Some(&auth))
                .await,
            expected
        );
        assert_eq!(
            manager
                .recommended_plugins_mode_for_config(&first_config, Some(&auth))
                .await,
            expected
        );
        assert_eq!(suggested[0].received_requests().await.len(), 1);
        assert_eq!(suggested[1].received_requests().await.len(), 2);
    }
    first_server.verify().await;
    second_server.verify().await;
}
