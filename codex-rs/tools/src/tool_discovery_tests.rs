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
    assert_eq!(
        create_tool_suggest_tool(&[
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
        ]),
        ToolSpec::Function(ResponsesApiTool {
            name: "tool_suggest".to_string(),
            description: "# Tool suggestion discovery\n\nSuggests a missing connector in an installed plugin, or in narrower cases a not installed but discoverable plugin, when the user clearly wants a capability that is not currently available in the active `tools` list.\n\nUse this ONLY when:\n- You've already tried to find a matching available tool for the user's request but couldn't find a good match. This includes `tool_search` (if available) and other means.\n- For connectors/apps that are not installed but needed for an installed plugin, suggest to install them if the task requirements match precisely.\n- For plugins that are not installed but discoverable, only suggest discoverable and installable plugins when the user's intent very explicitly and unambiguously matches that plugin itself. Do not suggest a plugin just because one of its connectors or capabilities seems relevant.\n\nTool suggestions should only use the discoverable tools listed here. DO NOT explore or recommend tools that are not on this list.\n\nDiscoverable tools:\n- GitHub (id: `github`, type: plugin, action: install): skills; MCP servers: github-mcp; app connectors: github-app\n- Slack (id: `slack@openai-curated`, type: connector, action: install): No description provided.\n\nWorkflow:\n\n1. Ensure all possible means have been exhausted to find an existing available tool but none of them matches the request intent.\n2. Match the user's request against the discoverable tools list above. Apply the stricter explicit-and-unambiguous rule for *discoverable tools* like plugin install suggestions; *missing tools* like connector install suggestions continue to use the normal clear-fit standard.\n3. If one tool clearly fits, call `tool_suggest` with:\n   - `tool_type`: `connector` or `plugin`\n   - `action_type`: `install` or `enable`\n   - `tool_id`: exact id from the discoverable tools list above\n   - `suggest_reason`: concise one-line user-facing reason this tool can help with the current request\n4. After the suggestion flow completes:\n   - if the user finished the install or enable flow, continue by searching again or using the newly available tool\n   - if the user did not finish, continue without that tool, and don't suggest that tool again unless the user explicitly asks for it.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::from([
                    (
                        "action_type".to_string(),
                        JsonSchema::string(Some(
                                "Suggested action for the tool. Use \"install\" or \"enable\"."
                                    .to_string(),
                            ),),
                    ),
                    (
                        "suggest_reason".to_string(),
                        JsonSchema::string(Some(
                                "Concise one-line user-facing reason why this tool can help with the current request."
                                    .to_string(),
                            ),),
                    ),
                    (
                        "tool_id".to_string(),
                        JsonSchema::string(Some(
                                "Connector or plugin id to suggest. Must be one of: slack@openai-curated, github."
                                    .to_string(),
                            ),),
                    ),
                    (
                        "tool_type".to_string(),
                        JsonSchema::string(Some(
                                "Type of discoverable tool to suggest. Use \"connector\" or \"plugin\"."
                                    .to_string(),
                            ),),
                    ),
                ]), Some(vec![
                    "tool_type".to_string(),
                    "action_type".to_string(),
                    "tool_id".to_string(),
                    "suggest_reason".to_string(),
                ]), Some(false.into())),
            output_schema: None,
        })
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
