//! Metadata publication must preserve derived caches until runtime inputs change.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn display_refresh_preserves_loaded_skills_and_tool_suggestions() {
    let codex_home = TempDir::new().unwrap();
    let marketplace_root = codex_home
        .path()
        .join("plugins/cache/openai-curated-remote");
    write_plugin(&marketplace_root, "sample/local", "sample");
    let plugin_root = marketplace_root.join("sample/local");
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true

[plugins."sample@openai-curated-remote"]
enabled = true
"#,
    );
    let config = load_config(codex_home.path(), codex_home.path()).await;
    let manager = test_plugins_manager_with_options(
        codex_home.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::Chatgpt),
    );
    let mut remote = remote_installed_plugin("sample");
    remote.interface = Some(
        serde_json::from_value(serde_json::json!({
            "capabilities": [], "screenshots": [], "screenshotUrls": [],
            "logoUrl": "https://files.openai.com/logo.png?sig=old"
        }))
        .unwrap(),
    );
    assert!(manager.write_remote_installed_plugins_cache(vec![remote.clone()]));

    let loaded = manager.plugins_for_config(&config).await;
    let snapshots = manager.plugin_skill_snapshots_for_config(&config).unwrap();
    let roots = loaded.effective_plugin_skill_roots();
    assert_eq!(roots.len(), 1);
    assert_eq!(snapshots.get(&roots[0]).unwrap().skills.len(), 1);
    let loaded_generation = manager.loaded_plugins_cache_generation();
    let plugin = ConfiguredMarketplacePlugin {
        id: "sample@openai-curated-remote".to_string(),
        name: "sample".to_string(),
        local_version: None,
        installed_version: None,
        source: MarketplacePluginSource::Local {
            path: plugin_root.abs(),
        },
        policy: MarketplacePluginPolicy {
            installation: MarketplacePluginInstallPolicy::Available,
            authentication: MarketplacePluginAuthPolicy::OnUse,
            products: None,
        },
        interface: None,
        keywords: Vec::new(),
        manifest_fallback: None,
        installed: true,
        enabled: true,
    };
    let suggestions = manager
        .tool_suggest_metadata_cache
        .metadata_for_plugin(
            REMOTE_GLOBAL_MARKETPLACE_NAME,
            &plugin,
            manager.restriction_product,
            manager.skill_root_loader.as_ref(),
        )
        .await
        .unwrap();
    assert!(
        suggestions
            .project(&SkillConfigRules::default(), manager.auth_mode())
            .has_skills
    );

    let interface = remote.interface.as_mut().unwrap();
    interface.logo_url = Some("https://files.openai.com/logo.png?sig=new".to_string());
    interface.display_name = Some("Updated display name".to_string());
    interface.capabilities = vec!["Updated store badge".to_string()];
    assert!(!manager.write_remote_installed_plugins_cache(vec![remote.clone()]));
    assert_eq!(
        manager
            .remote_installed_plugins_cache
            .read()
            .unwrap()
            .plugins,
        Some(vec![remote.clone()])
    );
    assert_eq!(manager.loaded_plugins_cache_generation(), loaded_generation);
    assert_eq!(
        manager.plugin_skill_snapshots_for_config(&config),
        Some(snapshots.clone())
    );
    assert_eq!(manager.plugins_for_config(&config).await, loaded);
    let refreshed_suggestions = manager
        .tool_suggest_metadata_cache
        .metadata_for_plugin(
            REMOTE_GLOBAL_MARKETPLACE_NAME,
            &plugin,
            manager.restriction_product,
            manager.skill_root_loader.as_ref(),
        )
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&suggestions, &refreshed_suggestions));

    remote.version = Some("2.0.0".to_string());
    assert!(manager.write_remote_installed_plugins_cache(vec![remote]));
    assert_eq!(
        manager.loaded_plugins_cache_generation(),
        loaded_generation + 1
    );
    assert_eq!(manager.plugin_skill_snapshots_for_config(&config), None);
    manager.plugins_for_config(&config).await;
    assert_ne!(
        manager.plugin_skill_snapshots_for_config(&config),
        Some(snapshots)
    );
    let changed_suggestions = manager
        .tool_suggest_metadata_cache
        .metadata_for_plugin(
            REMOTE_GLOBAL_MARKETPLACE_NAME,
            &plugin,
            manager.restriction_product,
            manager.skill_root_loader.as_ref(),
        )
        .await
        .unwrap();
    assert!(!Arc::ptr_eq(&suggestions, &changed_suggestions));
}
