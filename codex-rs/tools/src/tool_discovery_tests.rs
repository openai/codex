use super::*;
use crate::JsonSchema;
use codex_app_server_protocol::AppInfo;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn create_tool_search_tool_deduplicates_and_renders_enabled_sources() {
    assert_eq!(
        create_tool_search_tool(
            &[
                ToolSearchSourceInfo {
                    name: "Google Drive".to_string(),
                    description: Some(
                        "Use Google Drive as the single entrypoint for Drive, Docs, Sheets, and Slides work."
                            .to_string(),
                    ),
                },
                ToolSearchSourceInfo {
                    name: "Google Drive".to_string(),
                    description: None,
                },
                ToolSearchSourceInfo {
                    name: "docs".to_string(),
                    description: None,
                },
            ],
            /*default_limit*/ 8,
        ),
        ToolSpec::ToolSearch {
            execution: "client".to_string(),
            description: "# Tool discovery\n\nSearches over deferred tool metadata with BM25 and exposes matching tools for the next model call.\n\nYou have access to tools from the following sources:\n- Google Drive: Use Google Drive as the single entrypoint for Drive, Docs, Sheets, and Slides work.\n- docs\nSome of the tools may not have been provided to you upfront, and you should use this tool (`tool_search`) to search for the required tools. For MCP tool discovery, always use `tool_search` instead of `list_mcp_resources` or `list_mcp_resource_templates`.".to_string(),
            parameters: JsonSchema::object(BTreeMap::from([
                    (
                        "limit".to_string(),
                        JsonSchema::number(Some(
                                "Maximum number of tools to return (defaults to 8)."
                                    .to_string(),
                            ),),
                    ),
                    (
                        "query".to_string(),
                        JsonSchema::string(Some("Search query for deferred tools.".to_string()),),
                    ),
                ]), Some(vec!["query".to_string()]), Some(false.into())),
        }
    );
}

#[test]
fn create_tool_suggest_tool_uses_plugin_summary_fallback() {
    let tool = create_tool_suggest_tool(&[
        ToolSuggestEntry {
            id: "slack@openai-curated".to_string(),
            name: "Slack".to_string(),
            description: None,
            tool_type: DiscoverableToolType::Connector,
            has_skills: false,
            mcp_server_names: Vec::new(),
            app_connector_ids: Vec::new(),
        },
        ToolSuggestEntry {
            id: "github".to_string(),
            name: "GitHub".to_string(),
            description: None,
            tool_type: DiscoverableToolType::Plugin,
            has_skills: true,
            mcp_server_names: vec!["github-mcp".to_string()],
            app_connector_ids: vec!["github-app".to_string()],
        },
    ]);
    let ToolSpec::Function(ResponsesApiTool {
        name,
        description,
        strict,
        defer_loading,
        parameters,
        output_schema,
    }) = tool
    else {
        panic!("expected function tool");
    };

    assert_eq!(name, "tool_suggest");
    assert!(!strict);
    assert_eq!(defer_loading, None);
    assert_eq!(output_schema, None);
    assert!(
        description.contains(
            "You've already tried to find a matching available tool for the user's request"
        )
    );
    assert!(description.contains("This includes `tool_search` (if available) and other means."));
    assert!(description.contains("There are two types of allowed suggestions:"));
    assert!(description.contains("tool_type = \"plugin\", action_type = \"install\""));
    assert!(description.contains("tool_type = \"connector\", action_type = \"install\""));
    assert!(
        description.contains("- GitHub (id: `github`, type: plugin, action: install): skills; MCP servers: github-mcp; app connectors: github-app")
    );
    assert!(
        description.contains("- Slack (id: `slack@openai-curated`, type: connector, action: install): No description provided.")
    );
    assert!(description.contains("placeholders like `placeholder`"));

    assert_eq!(
        parameters,
        JsonSchema::object(
            BTreeMap::from([
                (
                    "action_type".to_string(),
                    JsonSchema::string(Some(
                        "Suggested action for the tool. Use \"install\" or \"enable\"."
                            .to_string(),
                    )),
                ),
                (
                    "suggest_reason".to_string(),
                    JsonSchema::string(Some(
                        "Concise one-line user-facing reason why this tool can help with the current request, must not be empty and must not be a placeholder."
                            .to_string(),
                    )),
                ),
                (
                    "tool_id".to_string(),
                    JsonSchema::string(Some(
                        "Connector or plugin id to suggest. Must be one of the discoverable tool ids."
                            .to_string(),
                    )),
                ),
                (
                    "tool_type".to_string(),
                    JsonSchema::string(Some(
                        "Type of discoverable tool to suggest. Use \"connector\" or \"plugin\"."
                            .to_string(),
                    )),
                ),
            ]),
            Some(vec![
                "tool_type".to_string(),
                "action_type".to_string(),
                "tool_id".to_string(),
                "suggest_reason".to_string(),
            ]),
            Some(false.into()),
        )
    );
}

#[test]
fn discoverable_tool_enums_use_expected_wire_names() {
    assert_eq!(
        json!({
            "tool_type": DiscoverableToolType::Connector,
            "action_type": DiscoverableToolAction::Install,
        }),
        json!({
            "tool_type": "connector",
            "action_type": "install",
        })
    );
}

#[test]
fn filter_tool_suggest_discoverable_tools_for_codex_tui_omits_plugins() {
    let discoverable_tools = vec![
        DiscoverableTool::Connector(Box::new(AppInfo {
            id: "connector_google_calendar".to_string(),
            name: "Google Calendar".to_string(),
            description: Some("Plan events and schedules.".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some("https://example.test/google-calendar".to_string()),
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        })),
        DiscoverableTool::Plugin(Box::new(DiscoverablePluginInfo {
            id: "slack@openai-curated".to_string(),
            name: "Slack".to_string(),
            description: Some("Search Slack messages".to_string()),
            has_skills: true,
            mcp_server_names: vec!["slack".to_string()],
            app_connector_ids: vec!["connector_slack".to_string()],
        })),
    ];

    assert_eq!(
        filter_tool_suggest_discoverable_tools_for_client(discoverable_tools, Some("codex-tui"),),
        vec![DiscoverableTool::Connector(Box::new(AppInfo {
            id: "connector_google_calendar".to_string(),
            name: "Google Calendar".to_string(),
            description: Some("Plan events and schedules.".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some("https://example.test/google-calendar".to_string()),
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }))]
    );
}
