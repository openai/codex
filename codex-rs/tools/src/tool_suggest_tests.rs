use super::*;
use crate::DiscoverablePluginInfo;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn build_tool_suggestion_elicitation_request_uses_expected_shape() {
    let args = ToolSuggestArgs {
        tool_type: DiscoverableToolType::Connector,
        action_type: DiscoverableToolAction::Install,
        tool_id: "connector_2128aebfecb84f64a069897515042a44".to_string(),
        suggest_reason: "Plan and reference events from your calendar".to_string(),
    };
    let connector = DiscoverableTool::Connector(Box::new(AppInfo {
        id: "connector_2128aebfecb84f64a069897515042a44".to_string(),
        name: "Google Calendar".to_string(),
        description: Some("Plan events and schedules.".to_string()),
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: Some(
            "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44"
                .to_string(),
        ),
        is_accessible: false,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }));

    let request = build_tool_suggestion_elicitation_request(
        "codex-apps",
        "thread-1".to_string(),
        "turn-1".to_string(),
        &args,
        "Plan and reference events from your calendar",
        &connector,
    );

    assert_eq!(
        request,
        McpServerElicitationRequestParams {
            thread_id: "thread-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            server_name: "codex-apps".to_string(),
            request: McpServerElicitationRequest::Form {
                meta: Some(json!(ToolSuggestMeta {
                    codex_approval_kind: TOOL_SUGGEST_APPROVAL_KIND_VALUE,
                    tool_type: DiscoverableToolType::Connector,
                    suggest_type: DiscoverableToolAction::Install,
                    suggest_reason: "Plan and reference events from your calendar",
                    tool_id: "connector_2128aebfecb84f64a069897515042a44",
                    tool_name: "Google Calendar",
                    install_url: Some(
                        "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44"
                    ),
                })),
                message: "Plan and reference events from your calendar".to_string(),
                requested_schema: McpElicitationSchema {
                    schema_uri: None,
                    type_: McpElicitationObjectType::Object,
                    properties: BTreeMap::new(),
                    required: None,
                },
            },
        },
    );
}

#[test]
fn build_tool_suggestion_elicitation_request_for_plugin_omits_install_url() {
    let args = ToolSuggestArgs {
        tool_type: DiscoverableToolType::Plugin,
        action_type: DiscoverableToolAction::Install,
        tool_id: "sample@openai-curated".to_string(),
        suggest_reason: "Use the sample plugin's skills and MCP server".to_string(),
    };
    let plugin = DiscoverableTool::Plugin(Box::new(DiscoverablePluginInfo {
        id: "sample@openai-curated".to_string(),
        name: "Sample Plugin".to_string(),
        description: Some("Includes skills, MCP servers, and apps.".to_string()),
        has_skills: true,
        mcp_server_names: vec!["sample-docs".to_string()],
        app_connector_ids: vec!["connector_calendar".to_string()],
    }));

    let request = build_tool_suggestion_elicitation_request(
        "codex-apps",
        "thread-1".to_string(),
        "turn-1".to_string(),
        &args,
        "Use the sample plugin's skills and MCP server",
        &plugin,
    );

    assert_eq!(
        request,
        McpServerElicitationRequestParams {
            thread_id: "thread-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            server_name: "codex-apps".to_string(),
            request: McpServerElicitationRequest::Form {
                meta: Some(json!(ToolSuggestMeta {
                    codex_approval_kind: TOOL_SUGGEST_APPROVAL_KIND_VALUE,
                    tool_type: DiscoverableToolType::Plugin,
                    suggest_type: DiscoverableToolAction::Install,
                    suggest_reason: "Use the sample plugin's skills and MCP server",
                    tool_id: "sample@openai-curated",
                    tool_name: "Sample Plugin",
                    install_url: None,
                })),
                message: "Use the sample plugin's skills and MCP server".to_string(),
                requested_schema: McpElicitationSchema {
                    schema_uri: None,
                    type_: McpElicitationObjectType::Object,
                    properties: BTreeMap::new(),
                    required: None,
                },
            },
        },
    );
}

#[test]
fn build_tool_suggestion_meta_uses_expected_shape() {
    let meta = build_tool_suggestion_meta(
        DiscoverableToolType::Connector,
        DiscoverableToolAction::Install,
        "Find and reference emails from your inbox",
        "connector_68df038e0ba48191908c8434991bbac2",
        "Gmail",
        Some("https://chatgpt.com/apps/gmail/connector_68df038e0ba48191908c8434991bbac2"),
    );

    assert_eq!(
        meta,
        ToolSuggestMeta {
            codex_approval_kind: TOOL_SUGGEST_APPROVAL_KIND_VALUE,
            tool_type: DiscoverableToolType::Connector,
            suggest_type: DiscoverableToolAction::Install,
            suggest_reason: "Find and reference emails from your inbox",
            tool_id: "connector_68df038e0ba48191908c8434991bbac2",
            tool_name: "Gmail",
            install_url: Some(
                "https://chatgpt.com/apps/gmail/connector_68df038e0ba48191908c8434991bbac2"
            ),
        },
    );
}

#[test]
fn verified_connector_suggestion_completed_requires_accessible_connector() {
    let accessible_connectors = vec![AppInfo {
        id: "calendar".to_string(),
        name: "Google Calendar".to_string(),
        description: None,
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: None,
        is_accessible: true,
        is_enabled: false,
        plugin_display_names: Vec::new(),
    }];

    assert!(verified_connector_suggestion_completed(
        "calendar",
        &accessible_connectors,
    ));
    assert!(!verified_connector_suggestion_completed(
        "gmail",
        &accessible_connectors,
    ));
}

#[test]
fn all_suggested_connectors_picked_up_requires_every_expected_connector() {
    let accessible_connectors = vec![AppInfo {
        id: "calendar".to_string(),
        name: "Google Calendar".to_string(),
        description: None,
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: None,
        is_accessible: true,
        is_enabled: false,
        plugin_display_names: Vec::new(),
    }];

    assert!(all_suggested_connectors_picked_up(
        &["calendar".to_string()],
        &accessible_connectors,
    ));
    assert!(!all_suggested_connectors_picked_up(
        &["calendar".to_string(), "gmail".to_string()],
        &accessible_connectors,
    ));
}
