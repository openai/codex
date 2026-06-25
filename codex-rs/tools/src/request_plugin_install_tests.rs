use super::*;
use crate::DiscoverablePluginInfo;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn build_request_plugin_install_elicitation_request_uses_expected_shape() {
    let plugin = DiscoverablePluginInfo {
        id: "sample@openai-curated-remote".to_string(),
        remote_plugin_id: Some("plugins~Plugin_sample".to_string()),
        name: "Sample Plugin".to_string(),
        description: Some("Includes skills, MCP servers, and apps.".to_string()),
        has_skills: true,
        mcp_server_names: vec!["sample-docs".to_string()],
        app_connector_ids: vec!["connector_calendar".to_string()],
    };

    let request = build_request_plugin_install_elicitation_request(
        "Use the sample plugin's skills and MCP server",
        &plugin,
    );

    assert_eq!(
        request,
        ElicitationRequest::Form {
            meta: Some(json!(RequestPluginInstallMeta {
                codex_approval_kind: REQUEST_PLUGIN_INSTALL_APPROVAL_KIND_VALUE,
                persist: REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE,
                tool_type: DiscoverableToolType::Plugin,
                suggest_type: DiscoverableToolAction::Install,
                suggest_reason: "Use the sample plugin's skills and MCP server",
                tool_id: "sample@openai-curated-remote",
                tool_name: "Sample Plugin",
                remote_plugin_id: Some("plugins~Plugin_sample"),
                app_connector_ids: &["connector_calendar".to_string()],
            })),
            message: "Use the sample plugin's skills and MCP server".to_string(),
            requested_schema: json!({
                "type": "object",
                "properties": {},
            }),
        },
    );
}
