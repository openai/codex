use super::ToolSuggestDiscoverablePlugin;
use super::ToolSuggestPluginDiscoveryInput;
use crate::OPENAI_BUNDLED_MARKETPLACE_NAME;
use crate::PluginInstallRequest;
use crate::PluginsConfigInput;
use crate::PluginsManager;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::remote::RemotePluginServiceConfig;
use crate::remote::fetch_and_cache_global_remote_plugin_catalog;
use crate::startup_sync::curated_plugins_repo_path;
use crate::test_support::TEST_CURATED_PLUGIN_SHA;
use crate::test_support::load_plugins_config;
use crate::test_support::write_curated_plugin;
use crate::test_support::write_curated_plugin_sha_with;
use crate::test_support::write_file;
use crate::test_support::write_openai_api_curated_marketplace;
use crate::test_support::write_openai_curated_marketplace;
use codex_config::CONFIG_TOML_FILE;
use codex_login::CodexAuth;
use codex_protocol::auth::AuthMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashSet;
use std::path::Path;
use tempfile::tempdir;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_test::internal::MockWriter;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

#[tokio::test]
async fn returns_fallback_plugins_when_remote_disabled_for_codex_auth() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample", "slack", "openai-developers"]);

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    plugins_manager.set_auth_mode(Some(AuthMode::Chatgpt));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        Some(&auth),
    )
    .await;

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec![
            "openai-developers@openai-curated".to_string(),
            "slack@openai-curated".to_string(),
        ]
    );
}

#[tokio::test]
async fn returns_api_curated_fallback_plugins_for_direct_provider_auth() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_api_curated_marketplace(&curated_root, &["sample", "slack", "openai-developers"]);

    let mut plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    plugins.remote_plugin_enabled = true;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    plugins_manager.set_auth_mode(Some(AuthMode::ApiKey));
    let auth = CodexAuth::from_api_key("test-api-key");
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        Some(&auth),
    )
    .await;

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec![
            "openai-developers@openai-api-curated".to_string(),
            "slack@openai-api-curated".to_string(),
        ]
    );
}

#[tokio::test]
async fn returns_microsoft_fallback_plugins() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(
        &curated_root,
        &["teams", "sharepoint", "outlook-email", "outlook-calendar"],
    );
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "teams").await;

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec![
            "outlook-calendar@openai-curated".to_string(),
            "outlook-email@openai-curated".to_string(),
            "sharepoint@openai-curated".to_string(),
        ]
    );
}

#[tokio::test]
async fn omits_openai_curated_but_keeps_configured_marketplaces_for_remote_codex_auth() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);

    let bundled_marketplace_name = OPENAI_BUNDLED_MARKETPLACE_NAME;
    let bundled_marketplace_root = codex_home
        .path()
        .join(format!(".tmp/marketplaces/{bundled_marketplace_name}"));
    write_file(
        &bundled_marketplace_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "{bundled_marketplace_name}",
  "plugins": [
    {{"name": "chrome", "source": {{"source": "local", "path": "./plugins/chrome"}}}}
  ]
}}
"#
        ),
    );
    write_curated_plugin(&bundled_marketplace_root, "chrome");
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        &format!(
            r#"[features]
plugins = true
remote_plugin = true

[marketplaces.{bundled_marketplace_name}]
source_type = "git"
source = "/tmp/{bundled_marketplace_name}"
"#
        ),
    );

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    plugins_manager.set_auth_mode(Some(AuthMode::Chatgpt));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        Some(&auth),
    )
    .await;

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec!["chrome@openai-bundled".to_string()]
    );
}

#[tokio::test]
async fn includes_openai_curated_when_remote_enabled_without_auth() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);

    let mut plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    plugins.remote_plugin_enabled = true;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec!["slack@openai-curated".to_string()]
    );
}

#[tokio::test]
async fn deduplicates_and_reprojects_cached_configured_marketplace_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let plugin_name = "sample";
    let marketplace_name = OPENAI_BUNDLED_MARKETPLACE_NAME;
    let plugin_id = format!("{plugin_name}@{marketplace_name}");
    let marketplace_root = codex_home
        .path()
        .join(format!(".tmp/marketplaces/{marketplace_name}"));
    write_file(
        &marketplace_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "{marketplace_name}",
  "plugins": [
    {{"name": "{plugin_name}", "source": {{"source": "local", "path": "./plugins/{plugin_name}"}}}}
  ]
}}
"#
        ),
    );
    write_curated_plugin(&marketplace_root, plugin_name);
    write_plugin_app(
        &marketplace_root,
        plugin_name,
        "sample-docs",
        "connector_sample",
    );
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        &format!(
            r#"[features]
plugins = true

[marketplaces.{marketplace_name}]
source_type = "git"
source = "/tmp/{marketplace_name}"
"#
        ),
    );
    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    assert!(plugins_manager.set_auth_mode(Some(AuthMode::Chatgpt)));
    let chatgpt_projection = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins.clone(), &[plugin_id.as_str()], &[], &[]),
        /*auth*/ None,
    )
    .await;
    let expected = ToolSuggestDiscoverablePlugin {
        id: plugin_id.clone(),
        remote_plugin_id: None,
        name: "sample".to_string(),
        description: Some(
            "Plugin that includes skills, MCP servers, and app connectors".to_string(),
        ),
        has_skills: true,
        mcp_server_names: Vec::new(),
        app_connector_ids: vec!["connector_sample".to_string()],
    };
    assert_eq!(chatgpt_projection, vec![expected.clone()]);

    assert!(plugins_manager.set_auth_mode(Some(AuthMode::ApiKey)));
    let api_key_projection = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[plugin_id.as_str()], &[], &[]),
        /*auth*/ None,
    )
    .await;
    assert_eq!(
        api_key_projection,
        vec![ToolSuggestDiscoverablePlugin {
            mcp_server_names: vec!["sample-docs".to_string()],
            app_connector_ids: Vec::new(),
            ..expected
        }]
    );
}

#[tokio::test]
async fn reprojects_cached_skill_availability_for_current_config() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let expected = ToolSuggestDiscoverablePlugin {
        id: "slack@openai-curated".to_string(),
        remote_plugin_id: None,
        name: "slack".to_string(),
        description: Some(
            "Plugin that includes skills, MCP servers, and app connectors".to_string(),
        ),
        has_skills: true,
        mcp_server_names: vec!["sample-docs".to_string()],
        app_connector_ids: vec!["connector_calendar".to_string()],
    };
    let initial = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;
    assert_eq!(initial, vec![expected.clone()]);

    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        r#"[[skills.config]]
name = "slack:sample"
enabled = false
"#,
    );
    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let after_skill_disabled = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;
    assert_eq!(
        after_skill_disabled,
        vec![ToolSuggestDiscoverablePlugin {
            has_skills: false,
            ..expected
        }]
    );
}

#[tokio::test]
async fn does_not_advertise_skills_when_skill_loading_fails() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_file(
        &curated_root.join("plugins/slack/skills/SKILL.md"),
        "---\nname: bad",
    );

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(
        discoverable_plugins,
        vec![ToolSuggestDiscoverablePlugin {
            id: "slack@openai-curated".to_string(),
            remote_plugin_id: None,
            name: "slack".to_string(),
            description: Some(
                "Plugin that includes skills, MCP servers, and app connectors".to_string(),
            ),
            has_skills: false,
            mcp_server_names: vec!["sample-docs".to_string()],
            app_connector_ids: vec!["connector_calendar".to_string()],
        }]
    );
}

#[tokio::test]
async fn clear_cache_invalidates_cached_tool_suggest_metadata() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    let plugin_manifest = curated_root.join("plugins/slack/.codex-plugin/plugin.json");
    write_file(
        &plugin_manifest,
        r#"{
  "name": "slack",
  "description": "Before reload"
}"#,
    );

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let input = discovery_input(plugins, &[], &[], &[]);
    let expected_cached = vec![ToolSuggestDiscoverablePlugin {
        id: "slack@openai-curated".to_string(),
        remote_plugin_id: None,
        name: "slack".to_string(),
        description: Some("Before reload".to_string()),
        has_skills: true,
        mcp_server_names: vec!["sample-docs".to_string()],
        app_connector_ids: vec!["connector_calendar".to_string()],
    }];
    let initial = list_discoverable_plugins(&plugins_manager, input.clone(), /*auth*/ None).await;
    assert_eq!(initial, expected_cached);

    write_file(
        &plugin_manifest,
        r#"{
  "name": "slack",
  "description": "After reload"
}"#,
    );
    let before_reload =
        list_discoverable_plugins(&plugins_manager, input.clone(), /*auth*/ None).await;
    assert_eq!(before_reload, expected_cached);

    plugins_manager.clear_cache();
    let after_reload = list_discoverable_plugins(&plugins_manager, input, /*auth*/ None).await;
    assert_eq!(
        after_reload,
        vec![ToolSuggestDiscoverablePlugin {
            description: Some("After reload".to_string()),
            ..expected_cached[0].clone()
        }]
    );
}

#[tokio::test]
async fn ignores_missing_marketplace_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["installed", "slack"]);
    let marketplace_name = OPENAI_BUNDLED_MARKETPLACE_NAME;
    let marketplace_root = codex_home
        .path()
        .join(format!(".tmp/marketplaces/{marketplace_name}"));
    write_file(
        &marketplace_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "{marketplace_name}",
  "plugins": [
    {{"name": "sample", "source": {{"source": "local", "path": "./plugins/sample"}}}}
  ]
}}
"#
        ),
    );
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        &format!(
            r#"[features]
plugins = true

[marketplaces.{marketplace_name}]
source_type = "git"
source = "/tmp/{marketplace_name}"
"#
        ),
    );
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "installed").await;

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(discoverable_plugins.len(), 1);
    assert_eq!(discoverable_plugins[0].id, "slack@openai-curated");
}

#[tokio::test]
async fn normalizes_description() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["installed", "slack"]);
    write_file(
        &curated_root.join("plugins/slack/.codex-plugin/plugin.json"),
        r#"{
  "name": "slack",
  "description": "  Plugin\n   with   extra   spacing  "
}"#,
    );
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "installed").await;

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(
        discoverable_plugins,
        vec![ToolSuggestDiscoverablePlugin {
            id: "slack@openai-curated".to_string(),
            remote_plugin_id: None,
            name: "slack".to_string(),
            description: Some("Plugin with extra spacing".to_string()),
            has_skills: true,
            mcp_server_names: vec!["sample-docs".to_string()],
            app_connector_ids: vec!["connector_calendar".to_string()],
        }]
    );
}

#[tokio::test]
async fn omits_installed_curated_plugins() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "slack").await;

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(discoverable_plugins, Vec::new());
}

#[tokio::test]
async fn omits_not_available_curated_plugins() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_file(
        &curated_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "openai-curated",
  "plugins": [
    {
      "name": "installed",
      "source": {
        "source": "local",
        "path": "./plugins/installed"
      }
    },
    {
      "name": "slack",
      "source": {
        "source": "local",
        "path": "./plugins/slack"
      }
    },
    {
      "name": "gmail",
      "source": {
        "source": "local",
        "path": "./plugins/gmail"
      },
      "policy": {
        "installation": "NOT_AVAILABLE"
      }
    }
  ]
}
"#,
    );
    write_curated_plugin(&curated_root, "installed");
    write_curated_plugin(&curated_root, "slack");
    write_curated_plugin(&curated_root, "gmail");
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "installed").await;

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec!["slack@openai-curated".to_string()]
    );
}

#[tokio::test]
async fn does_not_reload_marketplace_per_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack", "gmail", "openai-developers"]);
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "slack").await;

    let too_long_prompt = "x".repeat(129);
    for plugin_name in ["gmail", "openai-developers"] {
        write_file(
            &curated_root.join(format!("plugins/{plugin_name}/.codex-plugin/plugin.json")),
            &format!(
                r#"{{
  "name": "{plugin_name}",
  "description": "Plugin that includes skills, MCP servers, and app connectors",
  "interface": {{
    "defaultPrompt": "{too_long_prompt}"
  }}
}}"#
            ),
        );
    }

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let buffer: &'static std::sync::Mutex<Vec<u8>> =
        Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_level(true)
        .with_ansi(false)
        .with_max_level(Level::WARN)
        .with_span_events(FmtSpan::NONE)
        .with_writer(MockWriter::new(buffer))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(
        discoverable_plugins
            .iter()
            .map(|plugin| plugin.id.as_str())
            .collect::<Vec<_>>(),
        vec!["gmail@openai-curated", "openai-developers@openai-curated"]
    );

    let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
        .expect("utf8 logs")
        .replace('\\', "/");
    assert_eq!(logs.matches("ignoring interface.defaultPrompt").count(), 8);
    assert_eq!(logs.matches("gmail/.codex-plugin/plugin.json").count(), 4);
    assert_eq!(
        logs.matches("openai-developers/.codex-plugin/plugin.json")
            .count(),
        4
    );
}

#[tokio::test]
async fn does_not_expand_local_plugins_by_installed_apps() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample", "slack", "hubspot"]);
    write_plugin_app(&curated_root, "sample", "sample", "connector_sample");
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "slack").await;

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(discoverable_plugins, Vec::new());
}

#[tokio::test]
async fn does_not_read_local_plugins_for_loaded_apps() {
    let hubspot_app_id = "asdk_app_697acb8e53d88191bf7a79e62012ae14";
    let granola_app_id = "asdk_app_697761cab6f48191b5ed345919a3ce8b";
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["hubspot", "granola", "sample"]);
    write_plugin_app(&curated_root, "hubspot", "hubspot", hubspot_app_id);
    write_plugin_app(&curated_root, "granola", "granola", granola_app_id);
    write_file(
        &curated_root.join("plugins/sample/.app.json"),
        "invalid json",
    );

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let buffer: &'static std::sync::Mutex<Vec<u8>> =
        Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_level(true)
        .with_ansi(false)
        .with_max_level(Level::WARN)
        .with_span_events(FmtSpan::NONE)
        .with_writer(MockWriter::new(buffer))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[hubspot_app_id]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(discoverable_plugins, Vec::new());
    let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
        .expect("utf8 logs")
        .replace('\\', "/");
    assert_eq!(logs.matches("plugins/sample/.app.json").count(), 0);
}

#[tokio::test]
async fn does_not_expand_local_sales_apps() {
    let hubspot_app_id = "asdk_app_697acb8e53d88191bf7a79e62012ae14";
    let granola_app_id = "asdk_app_697761cab6f48191b5ed345919a3ce8b";
    let test_app_id = "asdk_app_test_source";
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["hubspot", "granola", "test-source"]);
    write_plugin_app(&curated_root, "hubspot", "hubspot", hubspot_app_id);
    write_plugin_app(&curated_root, "granola", "granola", granola_app_id);
    write_plugin_app(&curated_root, "test-source", "test_source", test_app_id);

    let sales_marketplace_name = "oai-maintained-plugins";
    let sales_marketplace_root = codex_home
        .path()
        .join(format!(".tmp/marketplaces/{sales_marketplace_name}"));
    write_file(
        &sales_marketplace_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "{sales_marketplace_name}",
  "plugins": [
    {{"name": "sales", "source": {{"source": "local", "path": "./plugins/sales"}}}}
  ]
}}
"#
        ),
    );
    write_curated_plugin(&sales_marketplace_root, "sales");
    write_file(
        &sales_marketplace_root.join("plugins/sales/.app.json"),
        &format!(
            r#"{{
  "apps": {{
    "hubspot": {{
      "id": "{hubspot_app_id}"
    }},
    "granola": {{
      "id": "{granola_app_id}"
    }}
  }}
}}
"#
        ),
    );
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        &format!(
            r#"[features]
plugins = true

[marketplaces.{sales_marketplace_name}]
source_type = "git"
source = "/tmp/{sales_marketplace_name}"
"#
        ),
    );
    install_marketplace_plugin(codex_home.path(), sales_marketplace_root.as_path(), "sales").await;

    let plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &[]),
        /*auth*/ None,
    )
    .await;

    assert_eq!(discoverable_plugins, Vec::new());
}

#[tokio::test]
async fn expands_cached_remote_plugins_by_loaded_apps() {
    let codex_home = tempdir().expect("tempdir should succeed");
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/list"))
        .and(query_param("scope", "GLOBAL"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [
                {
                    "id": "plugins~Plugin_remote_unlisted",
                    "name": "remote-unlisted",
                    "scope": "GLOBAL",
                    "installation_policy": "AVAILABLE",
                    "authentication_policy": "ON_USE",
                    "status": "AVAILABLE",
                    "release": {
                        "display_name": "Remote Unlisted",
                        "description": "Remote Unlisted long",
                        "app_ids": ["remote-unlisted-app"],
                        "interface": {
                            "short_description": "Remote Unlisted short",
                            "long_description": null,
                            "developer_name": null,
                            "category": null,
                            "capabilities": [],
                            "website_url": null,
                            "privacy_policy_url": null,
                            "terms_of_service_url": null,
                            "brand_color": null,
                            "default_prompt": null,
                            "composer_icon_url": null,
                            "logo_url": null,
                            "screenshot_urls": []
                        },
                        "skills": [
                            {
                                "name": "remote-unlisted",
                                "description": "Use unlisted remote plugin",
                                "interface": null
                            }
                        ]
                    }
                }
            ],
            "pagination": {
                "next_page_token": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let mut plugins = load_plugins_config(codex_home.path(), codex_home.path()).await;
    plugins.chatgpt_base_url = format!("{}/backend-api", server.uri());
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());
    fetch_and_cache_global_remote_plugin_catalog(
        codex_home.path(),
        &RemotePluginServiceConfig {
            chatgpt_base_url: plugins.chatgpt_base_url.clone(),
        },
        Some(&auth),
    )
    .await
    .expect("remote plugin catalog cache should write");

    for scope in ["GLOBAL", "USER", "WORKSPACE"] {
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/installed"))
            .and(query_param("scope", scope))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [],
                "pagination": {
                    "next_page_token": null
                }
            })))
            .expect(1)
            .mount(&server)
            .await;
    }
    plugins_manager
        .build_and_cache_remote_installed_plugin_marketplaces(
            &plugins,
            Some(&auth),
            &[REMOTE_GLOBAL_MARKETPLACE_NAME],
            /*on_effective_plugins_changed*/ None,
        )
        .await
        .expect("remote installed plugin cache should write");

    let discoverable_plugins = list_discoverable_plugins(
        &plugins_manager,
        discovery_input(plugins, &[], &[], &["remote-unlisted-app"]),
        Some(&auth),
    )
    .await;

    assert_eq!(
        discoverable_plugins,
        vec![ToolSuggestDiscoverablePlugin {
            id: "remote-unlisted@openai-curated-remote".to_string(),
            remote_plugin_id: Some("plugins~Plugin_remote_unlisted".to_string()),
            name: "Remote Unlisted".to_string(),
            description: Some("Remote Unlisted short".to_string()),
            has_skills: true,
            mcp_server_names: Vec::new(),
            app_connector_ids: vec!["remote-unlisted-app".to_string()],
        }]
    );
}

fn discovery_input(
    plugins: PluginsConfigInput,
    configured_plugin_ids: &[&str],
    disabled_plugin_ids: &[&str],
    loaded_plugin_app_connector_ids: &[&str],
) -> ToolSuggestPluginDiscoveryInput {
    ToolSuggestPluginDiscoveryInput {
        plugins,
        configured_plugin_ids: string_set(configured_plugin_ids),
        disabled_plugin_ids: string_set(disabled_plugin_ids),
        loaded_plugin_app_connector_ids: string_set(loaded_plugin_app_connector_ids),
    }
}

async fn list_discoverable_plugins(
    plugins_manager: &PluginsManager,
    input: ToolSuggestPluginDiscoveryInput,
    auth: Option<&CodexAuth>,
) -> Vec<ToolSuggestDiscoverablePlugin> {
    plugins_manager
        .list_tool_suggest_discoverable_plugins(&input, auth)
        .await
        .expect("discoverable plugins should load")
}

fn string_set(values: &[&str]) -> HashSet<String> {
    values.iter().map(ToString::to_string).collect()
}

async fn install_marketplace_plugin(codex_home: &Path, marketplace_root: &Path, plugin_name: &str) {
    write_curated_plugin_sha_with(codex_home, TEST_CURATED_PLUGIN_SHA);
    let config = load_plugins_config(codex_home, marketplace_root).await;
    PluginsManager::new(codex_home.to_path_buf())
        .install_plugin(
            &config.config_layer_stack,
            PluginInstallRequest {
                plugin_name: plugin_name.to_string(),
                marketplace_path: AbsolutePathBuf::try_from(
                    marketplace_root.join(".agents/plugins/marketplace.json"),
                )
                .expect("marketplace path"),
            },
        )
        .await
        .expect("plugin should install");
}

fn write_plugin_app(root: &Path, plugin_name: &str, app_name: &str, app_id: &str) {
    write_file(
        &root.join(format!("plugins/{plugin_name}/.app.json")),
        &format!(
            r#"{{
  "apps": {{
    "{app_name}": {{
      "id": "{app_id}"
    }}
  }}
}}
"#
        ),
    );
}
