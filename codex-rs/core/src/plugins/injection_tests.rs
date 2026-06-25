use std::sync::Arc;

use pretty_assertions::assert_eq;
use rmcp::model::Tool;

use super::*;

#[test]
fn plugin_injection_exposes_callable_namespace() {
    let plugin = PluginCapabilitySummary {
        config_name: "sample@test".to_string(),
        display_name: "sample".to_string(),
        has_skills: true,
        ..PluginCapabilitySummary::default()
    };
    let server_name = "sample-server";
    let callable_namespace = "sample_tools";
    let tool = ToolInfo {
        server_name: server_name.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: "search".to_string(),
        callable_namespace: callable_namespace.to_string(),
        namespace_description: None,
        namespace_title: None,
        search_aliases: Vec::new(),
        tool: Tool::new(
            "search",
            "Search",
            Arc::new(rmcp::model::JsonObject::default()),
        ),
        plugin_display_names: vec![plugin.display_name.clone()],
    };

    let serialized = serde_json::to_string(&build_plugin_injections(&[plugin], &[tool]))
        .expect("plugin injections should serialize");
    assert!(serialized.contains(callable_namespace));
    assert!(!serialized.contains(server_name));
    assert_eq!(serialized.matches(callable_namespace).count(), 1);
}
