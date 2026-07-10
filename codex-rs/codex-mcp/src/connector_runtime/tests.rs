use std::sync::Arc;

use codex_protocol::mcp::McpServerInfo;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Tool;
use tempfile::tempdir;

use super::*;

const LEGACY_CACHE_KEY_SHA1: &str = "3ee2e36a9f9ee95357626b2c0ec2c72f069c2258";

#[test]
fn pre_move_v4_tool_info_cache_cold_loads_from_legacy_path() {
    let codex_home = tempdir().expect("tempdir");
    let tools_cache_dir = codex_home.path().join("cache/codex_apps_tools");
    let server_info_cache_dir = codex_home.path().join("cache/codex_apps_server_info");
    std::fs::create_dir_all(&tools_cache_dir).expect("create tools cache directory");
    std::fs::create_dir_all(&server_info_cache_dir).expect("create server info cache directory");
    std::fs::write(
        tools_cache_dir.join(format!("{LEGACY_CACHE_KEY_SHA1}.json")),
        r#"{
  "schema_version": 4,
  "tools": [
    {
      "server_name": "codex_apps",
      "supports_parallel_tool_calls": true,
      "server_origin": "https://chatgpt.com",
      "tool_name": "calendar_search",
      "tool_namespace": "codex_apps",
      "namespace_description": "Calendar",
      "tool": {
        "name": "calendar.search",
        "description": "Search the calendar",
        "inputSchema": {}
      },
      "connector_id": "connector-calendar",
      "connector_name": "Calendar",
      "plugin_display_names": ["Calendar plugin"]
    }
  ]
}"#,
    )
    .expect("write legacy tools cache");
    std::fs::write(
        server_info_cache_dir.join(format!("{LEGACY_CACHE_KEY_SHA1}.json")),
        r#"{
  "schema_version": 1,
  "server_info": {
    "name": "codex-apps",
    "title": "Codex Apps",
    "version": "1.0.0",
    "description": null,
    "icons": null,
    "websiteUrl": null
  }
}"#,
    )
    .expect("write legacy server info cache");

    let expected_server_info = McpServerInfo {
        name: "codex-apps".to_string(),
        title: Some("Codex Apps".to_string()),
        version: "1.0.0".to_string(),
        description: None,
        icons: None,
        website_url: None,
    };
    let expected_tool = ToolInfo {
        server_name: "codex_apps".to_string(),
        supports_parallel_tool_calls: true,
        server_origin: Some("https://chatgpt.com".to_string()),
        callable_name: "calendar_search".to_string(),
        callable_namespace: "codex_apps".to_string(),
        namespace_description: Some("Calendar".to_string()),
        tool: Tool::new(
            "calendar.search".to_string(),
            "Search the calendar".to_string(),
            Arc::new(JsonObject::default()),
        ),
        connector_id: Some("connector-calendar".to_string()),
        connector_name: Some("Calendar".to_string()),
        plugin_display_names: vec!["Calendar plugin".to_string()],
    };
    let reloaded = ConnectorRuntimeManager::default().context(
        codex_home.path().to_path_buf(),
        ConnectorRuntimeContextKey::personal(
            Some("account-one".to_string()),
            Some("user-one".to_string()),
        ),
    );

    assert_eq!(
        serde_json::to_value(reloaded.current_tools()).expect("serialize reloaded tools"),
        serde_json::to_value(Some(vec![expected_tool])).expect("serialize expected tools"),
    );
    assert_eq!(reloaded.cached_server_info(), Some(expected_server_info));
}
