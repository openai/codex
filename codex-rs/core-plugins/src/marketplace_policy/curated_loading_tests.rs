//! Exercises curated Git requirements across discovery, installation, cached loading, and sync.

use super::*;
use pretty_assertions::assert_eq;

#[path = "curated_skill_tests.rs"]
mod skills;

#[tokio::test]
async fn curated_git_policy_controls_catalog_install_and_cached_activation() {
    for (name, auth) in [
        (OPENAI_CURATED_MARKETPLACE_NAME, AuthMode::Chatgpt),
        (OPENAI_API_CURATED_MARKETPLACE_NAME, AuthMode::ApiKey),
    ] {
        let home = TempDir::new().unwrap();
        let root = curated_plugins_repo_path(home.path());
        write_openai_curated_marketplace(&root, &["sample"]);
        write_openai_api_curated_marketplace(&root, &["sample"]);
        write_curated_plugin_sha(home.path(), TEST_CURATED_PLUGIN_SHA);
        write_cached_plugin(home.path(), name, "sample");
        let user_config = format!("[plugins.\"sample@{name}\"]\nenabled = true\n");
        let allowed = plugins_config_input_with_requirements(
            home.path(),
            &user_config,
            r#"
[marketplaces]
restrict_to_allowed_sources = true
[marketplaces.allowed_sources.curated]
source = "git"
url = "https://github.com/openai/plugins.git"
"#,
        );
        let blocked = plugins_config_input_with_requirements(
            home.path(),
            &user_config,
            "[marketplaces]\nrestrict_to_allowed_sources = true\n",
        );
        let host_allowed = plugins_config_input_with_requirements(
            home.path(),
            &user_config,
            r#"
[marketplaces]
restrict_to_allowed_sources = true
[marketplaces.allowed_sources.github]
source = "host_pattern"
host_pattern = '^github[.]com$'
"#,
        );
        let manager = Arc::new(test_plugins_manager_with_options(
            home.path().to_path_buf(),
            Some(Product::Codex),
            Some(auth),
        ));
        let manifest = if name == OPENAI_CURATED_MARKETPLACE_NAME {
            root.join(".agents/plugins/marketplace.json")
        } else {
            curated_plugins_api_marketplace_path(home.path())
        };
        for (config, permitted) in [(&allowed, true), (&blocked, false), (&host_allowed, true)] {
            let expected = if permitted {
                vec![format!("sample@{name}")]
            } else {
                Vec::new()
            };
            assert_eq!(loaded_plugin_names(&manager, config).await, expected);
            let catalogs = manager
                .list_marketplaces_for_config(config, &[], /*include_openai_curated*/ true)
                .unwrap();
            assert_eq!(
                catalogs
                    .marketplaces
                    .iter()
                    .map(|catalog| catalog.name.as_str())
                    .collect::<Vec<_>>(),
                if permitted { vec![name] } else { Vec::new() }
            );
            assert_eq!(
                manager
                    .install_plugin(
                        &config.config_layer_stack,
                        PluginInstallRequest {
                            plugin_name: "sample".to_string(),
                            marketplace_path: AbsolutePathBuf::try_from(manifest.clone()).unwrap(),
                        }
                    )
                    .await
                    .is_ok(),
                permitted
            );
        }
        manager.maybe_start_curated_repo_sync_for_config(
            &blocked, /*on_effective_plugins_changed*/ None,
        );
        assert!(!CURATED_REPO_SYNC_STARTED.load(std::sync::atomic::Ordering::SeqCst));
    }
}

#[tokio::test]
async fn blocking_curated_git_does_not_block_remote_installed_plugins() {
    let home = TempDir::new().unwrap();
    write_cached_plugin(home.path(), REMOTE_GLOBAL_MARKETPLACE_NAME, "linear");
    let manager = test_plugins_manager_with_options(
        home.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::Chatgpt),
    );
    manager.write_remote_installed_plugins_cache(vec![remote_installed_plugin("linear")]);
    let mut config = plugins_config_input_with_requirements(
        home.path(),
        "",
        "[marketplaces]\nrestrict_to_allowed_sources = true\n",
    );
    config.remote_plugin_enabled = true;
    assert_eq!(
        loaded_plugin_names(&manager, &config).await,
        vec!["linear@openai-curated-remote".to_string()]
    );
}
