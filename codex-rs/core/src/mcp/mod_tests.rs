use super::*;
use crate::config::CONFIG_TOML_FILE;
use crate::config::ConfigBuilder;
use crate::plugins::AppConnectorId;
use crate::plugins::PluginCapabilitySummary;
use codex_features::Feature;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use toml::Value;

fn write_file(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("file should have a parent")).unwrap();
    fs::write(path, contents).unwrap();
}

fn plugin_config_toml() -> String {
    let mut root = toml::map::Map::new();

    let mut features = toml::map::Map::new();
    features.insert("plugins".to_string(), Value::Boolean(true));
    root.insert("features".to_string(), Value::Table(features));

    let mut plugin = toml::map::Map::new();
    plugin.insert("enabled".to_string(), Value::Boolean(true));

    let mut plugins = toml::map::Map::new();
    plugins.insert("sample@test".to_string(), Value::Table(plugin));
    root.insert("plugins".to_string(), Value::Table(plugins));

    toml::to_string(&Value::Table(root)).expect("plugin test config should serialize")
}

fn make_tool_info(server_name: &str, tool_name: &str, tool_namespace: &str) -> ToolInfo {
    ToolInfo {
        server_name: server_name.to_string(),
        tool_name: tool_name.to_string(),
        tool_namespace: tool_namespace.to_string(),
        tool: rmcp::model::Tool {
            name: tool_name.to_string().into(),
            title: None,
            description: None,
            input_schema: Arc::new(JsonObject::default()),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
        connector_description: None,
    }
}

#[test]
fn qualified_mcp_tool_name_prefix_sanitizes_server_names_without_lowercasing() {
    assert_eq!(
        qualified_mcp_tool_name_prefix("Some-Server"),
        "mcp__Some_Server__".to_string()
    );
}

#[test]
fn mcp_server_status_tool_name_preserves_hyphenated_mcp_tool_names() {
    let tool_info = make_tool_info(
        "music-studio",
        "play-live-pattern",
        "music-studio",
    );

    assert_eq!(
        mcp_server_status_tool_name(&tool_info),
        "play-live-pattern".to_string()
    );
}

#[test]
fn mcp_server_status_tool_name_includes_codex_apps_connector_namespace() {
    let tool_info = make_tool_info(
        CODEX_APPS_MCP_SERVER_NAME,
        "_property_search",
        "mcp__codex_apps__zillow",
    );

    assert_eq!(
        mcp_server_status_tool_name(&tool_info),
        "zillow_property_search".to_string()
    );
}

#[test]
fn tool_plugin_provenance_collects_app_and_mcp_sources() {
    let provenance = ToolPluginProvenance::from_capability_summaries(&[
        PluginCapabilitySummary {
            display_name: "alpha-plugin".to_string(),
            app_connector_ids: vec![AppConnectorId("connector_example".to_string())],
            mcp_server_names: vec!["alpha".to_string()],
            ..PluginCapabilitySummary::default()
        },
        PluginCapabilitySummary {
            display_name: "beta-plugin".to_string(),
            app_connector_ids: vec![
                AppConnectorId("connector_example".to_string()),
                AppConnectorId("connector_gmail".to_string()),
            ],
            mcp_server_names: vec!["beta".to_string()],
            ..PluginCapabilitySummary::default()
        },
    ]);

    assert_eq!(
        provenance,
        ToolPluginProvenance {
            plugin_display_names_by_connector_id: HashMap::from([
                (
                    "connector_example".to_string(),
                    vec!["alpha-plugin".to_string(), "beta-plugin".to_string()],
                ),
                (
                    "connector_gmail".to_string(),
                    vec!["beta-plugin".to_string()],
                ),
            ]),
            plugin_display_names_by_mcp_server_name: HashMap::from([
                ("alpha".to_string(), vec!["alpha-plugin".to_string()]),
                ("beta".to_string(), vec!["beta-plugin".to_string()]),
            ]),
        }
    );
}

#[test]
fn codex_apps_mcp_url_for_base_url_keeps_existing_paths() {
    assert_eq!(
        codex_apps_mcp_url_for_base_url("https://chatgpt.com/backend-api"),
        "https://chatgpt.com/backend-api/wham/apps"
    );
    assert_eq!(
        codex_apps_mcp_url_for_base_url("https://chat.openai.com"),
        "https://chat.openai.com/backend-api/wham/apps"
    );
    assert_eq!(
        codex_apps_mcp_url_for_base_url("http://localhost:8080/api/codex"),
        "http://localhost:8080/api/codex/apps"
    );
    assert_eq!(
        codex_apps_mcp_url_for_base_url("http://localhost:8080"),
        "http://localhost:8080/api/codex/apps"
    );
}

#[test]
fn codex_apps_mcp_url_uses_legacy_codex_apps_path() {
    let mut config = crate::config::test_config();
    config.chatgpt_base_url = "https://chatgpt.com".to_string();

    assert_eq!(
        codex_apps_mcp_url(&config),
        "https://chatgpt.com/backend-api/wham/apps"
    );
}

#[test]
fn codex_apps_server_config_uses_legacy_codex_apps_path() {
    let mut config = crate::config::test_config();
    config.chatgpt_base_url = "https://chatgpt.com".to_string();

    let mut servers = with_codex_apps_mcp(
        HashMap::new(),
        /*connectors_enabled*/ false,
        /*auth*/ None,
        &config,
    );
    assert!(!servers.contains_key(CODEX_APPS_MCP_SERVER_NAME));

    config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow apps");

    servers = with_codex_apps_mcp(
        servers, /*connectors_enabled*/ true, /*auth*/ None, &config,
    );
    let server = servers
        .get(CODEX_APPS_MCP_SERVER_NAME)
        .expect("codex apps should be present when apps is enabled");
    let url = match &server.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => url,
        _ => panic!("expected streamable http transport for codex apps"),
    };

    assert_eq!(url, "https://chatgpt.com/backend-api/wham/apps");
}

#[tokio::test]
async fn effective_mcp_servers_include_plugins_without_overriding_user_config() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");
    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "sample": {
      "type": "http",
      "url": "https://plugin.example/mcp"
    },
    "docs": {
      "type": "http",
      "url": "https://docs.example/mcp"
    }
  }
}"#,
    );
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        &plugin_config_toml(),
    );

    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config should load");

    let mut configured_servers = config.mcp_servers.get().clone();
    configured_servers.insert(
        "sample".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://user.example/mcp".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );
    config
        .mcp_servers
        .set(configured_servers)
        .expect("test config should accept MCP servers");

    let mcp_manager = McpManager::new(Arc::new(PluginsManager::new(config.codex_home.clone())));
    let effective = mcp_manager.effective_servers(&config, /*auth*/ None);

    let sample = effective.get("sample").expect("user server should exist");
    let docs = effective.get("docs").expect("plugin server should exist");

    match &sample.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            assert_eq!(url, "https://user.example/mcp");
        }
        other => panic!("expected streamable http transport, got {other:?}"),
    }
    match &docs.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            assert_eq!(url, "https://docs.example/mcp");
        }
        other => panic!("expected streamable http transport, got {other:?}"),
    }
}
