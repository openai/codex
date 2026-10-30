#![cfg(test)]

use super::AgentMcpMetadataRecipient;
use codex_config::McpServerConfig;
use codex_mcp::McpPluginAttribution;
use codex_mcp::McpServerRegistration;
use codex_mcp::ResolvedMcpCatalog;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test_case::test_case("exact", true)]
#[test_case::test_case("plugin_id", false)]
#[test_case::test_case("plugin_root", false)]
#[test_case::test_case("server_name", false)]
#[test_case::test_case("endpoint", false)]
#[test_case::test_case("environment", false)]
#[test_case::test_case("config_source", false)]
fn agent_context_requires_the_exact_captured_recipient(mismatch: &str, expected: bool) {
    let mut config: McpServerConfig = serde_json::from_value(json!({
        "url": "https://coordinator.example/mcp"
    }))
    .expect("server config");
    let recipient = AgentMcpMetadataRecipient {
        plugin_id: "agents@local".into(),
        plugin_root: PathUri::parse("file:///plugins/agents").unwrap(),
        server_name: "agents".into(),
        transport: config.transport.clone(),
    };
    let mut plugin_id = recipient.plugin_id.clone();
    let mut root = recipient.plugin_root.clone();
    let mut name = recipient.server_name.clone();
    match mismatch {
        "plugin_id" => plugin_id = "unrelated@local".into(),
        "plugin_root" => root = PathUri::parse("file:///plugins/replaced").unwrap(),
        "server_name" => name = "another-server".into(),
        "endpoint" => {
            config = serde_json::from_value(json!({
                "url": "https://unrelated.example/mcp"
            }))
            .expect("replacement server config")
        }
        "environment" => config.environment_id = "remote".into(),
        "exact" | "config_source" => {}
        _ => panic!("unknown mismatch"),
    }
    let registration = if mismatch == "config_source" {
        McpServerRegistration::from_config(name.clone(), config)
    } else {
        McpServerRegistration::from_plugin(
            name.clone(),
            McpPluginAttribution::new(plugin_id, "Agents".into()).with_host_root(root),
            /*plugin_order*/ 0,
            config,
        )
    };
    let mut builder = ResolvedMcpCatalog::builder();
    builder.register(registration);
    assert_eq!(recipient.matches(&name, &builder.build()), expected);
}
