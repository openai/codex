use super::*;
use pretty_assertions::assert_eq;

#[test]
fn build_remote_marketplace_preserves_directory_order_and_appends_installed_only_plugins() {
    let directory_plugins = vec![
        directory_plugin("plugin-z", "zulu"),
        directory_plugin("plugin-m", "mike"),
    ];
    let installed_plugins = vec![RemotePluginInstalledItem {
        plugin: directory_plugin("plugin-a", "alpha"),
        enabled: true,
        disabled_skill_names: Vec::new(),
    }];

    let marketplace = build_remote_marketplace(
        "marketplace",
        "Marketplace",
        directory_plugins,
        installed_plugins,
        /*include_installed_only*/ true,
    )
    .expect("marketplace should be valid")
    .expect("marketplace should not be empty");

    assert_eq!(
        marketplace
            .plugins
            .into_iter()
            .map(|plugin| plugin.remote_plugin_id)
            .collect::<Vec<_>>(),
        vec!["plugin-z", "plugin-m", "plugin-a"]
    );
}

fn directory_plugin(id: &str, name: &str) -> RemotePluginDirectoryItem {
    RemotePluginDirectoryItem {
        id: id.to_string(),
        name: name.to_string(),
        scope: RemotePluginScope::Global,
        discoverability: None,
        creator_account_user_id: None,
        creator_name: None,
        share_url: None,
        share_principals: None,
        installation_policy: PluginInstallPolicy::Available,
        authentication_policy: PluginAuthPolicy::OnUse,
        availability: PluginAvailability::Available,
        release: RemotePluginReleaseResponse {
            version: None,
            display_name: name.to_string(),
            description: String::new(),
            bundle_download_url: None,
            app_ids: Vec::new(),
            app_manifest: None,
            app_templates: Vec::new(),
            keywords: Vec::new(),
            interface: RemotePluginReleaseInterfaceResponse {
                short_description: None,
                long_description: None,
                developer_name: None,
                category: None,
                capabilities: Vec::new(),
                website_url: None,
                privacy_policy_url: None,
                terms_of_service_url: None,
                brand_color: None,
                default_prompt: None,
                default_prompts: None,
                composer_icon_url: None,
                logo_url: None,
                screenshot_urls: Vec::new(),
            },
            skills: Vec::new(),
            mcp_servers: Vec::new(),
        },
    }
}
fn item(name: &str, display_name: &str) -> RecommendedPluginItem {
    RecommendedPluginItem {
        id: format!("plugin_{name}"),
        name: name.to_string(),
        status: None,
        installation_policy: None,
        release: RecommendedPluginRelease {
            display_name: display_name.to_string(),
            app_ids: Vec::new(),
        },
    }
}

#[test]
fn recommended_plugins_enabled_flag_selects_endpoint_or_legacy_mode() {
    let disabled: RecommendedPluginsResponse = serde_json::from_value(serde_json::json!({
        "enabled": false,
        "plugins": [{"id": "plugin_github", "name": "github", "release": {"display_name": "GitHub"}}]
    }))
    .expect("response should deserialize");
    assert_eq!(
        recommended_plugins_mode(disabled),
        RecommendedPluginsMode::Legacy
    );

    for response in [
        serde_json::json!({"plugins": []}),
        serde_json::json!({"enabled": null, "plugins": []}),
    ] {
        let response: RecommendedPluginsResponse =
            serde_json::from_value(response).expect("response should deserialize");
        assert_eq!(
            recommended_plugins_mode(response),
            RecommendedPluginsMode::Legacy
        );
    }

    let enabled: RecommendedPluginsResponse = serde_json::from_value(serde_json::json!({
        "enabled": true,
        "plugins": []
    }))
    .expect("response should deserialize");
    assert_eq!(
        recommended_plugins_mode(enabled),
        RecommendedPluginsMode::Endpoint {
            plugins: Vec::new()
        }
    );
}

#[test]
fn recommended_plugins_require_remote_install_identity() {
    let response = serde_json::from_value::<RecommendedPluginsResponse>(serde_json::json!({
        "enabled": true,
        "plugins": [{
            "name": "github",
            "release": {"display_name": "GitHub"}
        }]
    }));

    assert!(response.is_err());
}

#[test]
fn recommended_plugins_are_validated_deduplicated_sorted_and_capped() {
    let mut plugins = (0..=52)
        .rev()
        .map(|index| item(&format!("plugin-{index:02}"), &format!("Plugin {index:02}")))
        .collect::<Vec<_>>();
    plugins.push(item("plugin-00", "Duplicate"));
    plugins.push(item("not/a/plugin", "Invalid"));
    plugins.push(RecommendedPluginItem {
        id: "plugin_disabled".to_string(),
        name: "disabled".to_string(),
        status: Some(PluginAvailability::DisabledByAdmin),
        installation_policy: Some(PluginInstallPolicy::Available),
        release: RecommendedPluginRelease {
            display_name: "Disabled".to_string(),
            app_ids: Vec::new(),
        },
    });
    plugins.push(RecommendedPluginItem {
        id: "plugin_not_available".to_string(),
        name: "not-available".to_string(),
        status: Some(PluginAvailability::Available),
        installation_policy: Some(PluginInstallPolicy::NotAvailable),
        release: RecommendedPluginRelease {
            display_name: "Not Available".to_string(),
            app_ids: Vec::new(),
        },
    });

    let mode = recommended_plugins_mode(RecommendedPluginsResponse {
        enabled: Some(true),
        plugins,
    });
    let RecommendedPluginsMode::Endpoint { plugins } = mode else {
        panic!("expected endpoint mode");
    };

    assert_eq!(plugins.len(), MAX_RECOMMENDED_PLUGINS);
    assert_eq!(
        plugins.first(),
        Some(&RecommendedPlugin {
            config_id: "plugin-00@openai-curated-remote".to_string(),
            remote_plugin_id: "plugin_plugin-00".to_string(),
            display_name: "Plugin 00".to_string(),
            app_connector_ids: Vec::new(),
        })
    );
    assert_eq!(
        plugins.last(),
        Some(&RecommendedPlugin {
            config_id: "plugin-49@openai-curated-remote".to_string(),
            remote_plugin_id: "plugin_plugin-49".to_string(),
            display_name: "Plugin 49".to_string(),
            app_connector_ids: Vec::new(),
        })
    );
}

#[test]
fn recommended_plugins_preserve_install_identity_and_normalize_app_ids() {
    let mode = recommended_plugins_mode(RecommendedPluginsResponse {
        enabled: Some(true),
        plugins: vec![RecommendedPluginItem {
            id: "plugin_connector_sample".to_string(),
            name: "sample".to_string(),
            status: Some(PluginAvailability::Available),
            installation_policy: Some(PluginInstallPolicy::Available),
            release: RecommendedPluginRelease {
                display_name: "Sample".to_string(),
                app_ids: vec![
                    "connector_one".to_string(),
                    String::new(),
                    "connector_two".to_string(),
                    "connector_one".to_string(),
                ],
            },
        }],
    });

    assert_eq!(
        mode,
        RecommendedPluginsMode::Endpoint {
            plugins: vec![RecommendedPlugin {
                config_id: "sample@openai-curated-remote".to_string(),
                remote_plugin_id: "plugin_connector_sample".to_string(),
                display_name: "Sample".to_string(),
                app_connector_ids: vec!["connector_one".to_string(), "connector_two".to_string(),],
            }],
        }
    );
}

#[test]
fn recommended_plugins_ignore_invalid_remote_plugin_ids() {
    let mode = recommended_plugins_mode(RecommendedPluginsResponse {
        enabled: Some(true),
        plugins: vec![RecommendedPluginItem {
            id: "not/a/plugin".to_string(),
            name: "sample".to_string(),
            status: None,
            installation_policy: None,
            release: RecommendedPluginRelease {
                display_name: "Sample".to_string(),
                app_ids: Vec::new(),
            },
        }],
    });

    assert_eq!(
        mode,
        RecommendedPluginsMode::Endpoint {
            plugins: Vec::new(),
        }
    );
}
