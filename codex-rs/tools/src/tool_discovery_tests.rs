use super::*;
use codex_app_server_protocol::AppInfo;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Tool;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

fn mcp_tool(name: &str, description: &str) -> Tool {
    Tool {
        name: name.to_string().into(),
        title: None,
        description: Some(description.to_string().into()),
        input_schema: Arc::new(JsonObject::from_iter([(
            "type".to_string(),
            json!("object"),
        )])),
        output_schema: None,
        annotations: None,
        execution: None,
        icons: None,
        meta: None,
    }
}

#[test]
fn create_tool_search_tool_deduplicates_and_renders_enabled_apps() {
    assert_eq!(
        create_tool_search_tool(
            &[
                ToolSearchAppInfo {
                    name: "Google Drive".to_string(),
                    description: Some(
                        "Use Google Drive as the single entrypoint for Drive, Docs, Sheets, and Slides work."
                            .to_string(),
                    ),
                },
                ToolSearchAppInfo {
                    name: "Google Drive".to_string(),
                    description: None,
                },
                ToolSearchAppInfo {
                    name: "Slack".to_string(),
                    description: None,
                },
            ],
            /*default_limit*/ 8,
        ),
        ToolSpec::ToolSearch {
            execution: "client".to_string(),
            description: "# Apps (Connectors) tool discovery\n\nSearches over apps/connectors tool metadata with BM25 and exposes matching tools for the next model call.\n\nYou have access to all the tools of the following apps/connectors:\n- Google Drive: Use Google Drive as the single entrypoint for Drive, Docs, Sheets, and Slides work.\n- Slack\nSome of the tools may not have been provided to you upfront, and you should use this tool (`tool_search`) to search for the required tools and load them for the apps mentioned above. For the apps mentioned above, always use `tool_search` instead of `list_mcp_resources` or `list_mcp_resource_templates` for tool discovery.".to_string(),
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([
                    (
                        "limit".to_string(),
                        JsonSchema::Number {
                            description: Some(
                                "Maximum number of tools to return (defaults to 8)."
                                    .to_string(),
                            ),
                        },
                    ),
                    (
                        "query".to_string(),
                        JsonSchema::String {
                            description: Some("Search query for apps tools.".to_string()),
                        },
                    ),
                ]),
                required: Some(vec!["query".to_string()]),
                additional_properties: Some(false.into()),
            },
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
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([
                    (
                        "action_type".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "Suggested action for the tool. Use \"install\" or \"enable\"."
                                    .to_string(),
                            ),
                        },
                    ),
                    (
                        "suggest_reason".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "Concise one-line user-facing reason why this tool can help with the current request."
                                    .to_string(),
                            ),
                        },
                    ),
                    (
                        "tool_id".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "Connector or plugin id to suggest. Must be one of: slack@openai-curated, github."
                                    .to_string(),
                            ),
                        },
                    ),
                    (
                        "tool_type".to_string(),
                        JsonSchema::String {
                            description: Some(
                                "Type of discoverable tool to suggest. Use \"connector\" or \"plugin\"."
                                    .to_string(),
                            ),
                        },
                    ),
                ]),
                required: Some(vec![
                    "tool_type".to_string(),
                    "action_type".to_string(),
                    "tool_id".to_string(),
                    "suggest_reason".to_string(),
                ]),
                additional_properties: Some(false.into()),
            },
            output_schema: None,
        })
    );
}

#[test]
fn collect_tool_search_output_tools_groups_results_by_namespace() {
    let calendar_create_event = mcp_tool("calendar-create-event", "Create a calendar event.");
    let gmail_read_email = mcp_tool("gmail-read-email", "Read an email.");
    let calendar_list_events = mcp_tool("calendar-list-events", "List calendar events.");

    let tools = collect_tool_search_output_tools([
        ToolSearchResultSource {
            tool_namespace: "mcp__codex_apps__calendar",
            tool_name: "_create_event",
            tool: &calendar_create_event,
            connector_name: Some("Calendar"),
            connector_description: Some("Plan events"),
        },
        ToolSearchResultSource {
            tool_namespace: "mcp__codex_apps__gmail",
            tool_name: "_read_email",
            tool: &gmail_read_email,
            connector_name: Some("Gmail"),
            connector_description: Some("Read mail"),
        },
        ToolSearchResultSource {
            tool_namespace: "mcp__codex_apps__calendar",
            tool_name: "_list_events",
            tool: &calendar_list_events,
            connector_name: Some("Calendar"),
            connector_description: Some("Plan events"),
        },
    ])
    .expect("collect tool search output tools");

    assert_eq!(
        tools,
        vec![
            ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
                name: "mcp__codex_apps__calendar".to_string(),
                description: "Plan events".to_string(),
                tools: vec![
                    ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                        name: "_create_event".to_string(),
                        description: "Create a calendar event.".to_string(),
                        strict: false,
                        defer_loading: Some(true),
                        parameters: JsonSchema::Object {
                            properties: Default::default(),
                            required: None,
                            additional_properties: None,
                        },
                        output_schema: None,
                    }),
                    ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                        name: "_list_events".to_string(),
                        description: "List calendar events.".to_string(),
                        strict: false,
                        defer_loading: Some(true),
                        parameters: JsonSchema::Object {
                            properties: Default::default(),
                            required: None,
                            additional_properties: None,
                        },
                        output_schema: None,
                    }),
                ],
            }),
            ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
                name: "mcp__codex_apps__gmail".to_string(),
                description: "Read mail".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "_read_email".to_string(),
                    description: "Read an email.".to_string(),
                    strict: false,
                    defer_loading: Some(true),
                    parameters: JsonSchema::Object {
                        properties: Default::default(),
                        required: None,
                        additional_properties: None,
                    },
                    output_schema: None,
                })],
            }),
        ],
    );
}

#[test]
fn collect_tool_search_output_tools_falls_back_to_connector_name_description() {
    let gmail_batch_read_email = mcp_tool("gmail-batch-read-email", "Read multiple emails.");

    let tools = collect_tool_search_output_tools([ToolSearchResultSource {
        tool_namespace: "mcp__codex_apps__gmail",
        tool_name: "_batch_read_email",
        tool: &gmail_batch_read_email,
        connector_name: Some("Gmail"),
        connector_description: None,
    }])
    .expect("collect tool search output tools");

    assert_eq!(
        tools,
        vec![ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
            name: "mcp__codex_apps__gmail".to_string(),
            description: "Tools for working with Gmail.".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "_batch_read_email".to_string(),
                description: "Read multiple emails.".to_string(),
                strict: false,
                defer_loading: Some(true),
                parameters: JsonSchema::Object {
                    properties: Default::default(),
                    required: None,
                    additional_properties: None,
                },
                output_schema: None,
            })],
        })],
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
            tools: Vec::new(),
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
            tools: Vec::new(),
        }))]
    );
}
