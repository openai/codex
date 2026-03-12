use super::*;
use pretty_assertions::assert_eq;

#[test]
fn build_tool_suggestion_elicitation_request_uses_expected_shape() {
    let args = ToolSuggestArgs {
        tool_type: DiscoverableToolType::Connector,
        action_type: DiscoverableToolAction::Install,
        tool_id: "connector_2128aebfecb84f64a069897515042a44".to_string(),
        suggest_reason: "Plan and reference events from your calendar".to_string(),
    };
    let connector = AppInfo {
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
    };

    let request = build_tool_suggestion_elicitation_request(
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
                server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                request: McpServerElicitationRequest::Form {
                    meta: Some(json!(ToolSuggestMeta {
                        codex_approval_kind: TOOL_SUGGEST_APPROVAL_KIND_VALUE,
                        tool_type: DiscoverableToolType::Connector,
                        suggest_type: DiscoverableToolAction::Install,
                        suggest_reason: "Plan and reference events from your calendar",
                        tool_id: "connector_2128aebfecb84f64a069897515042a44",
                        tool_name: "Google Calendar",
                        install_url: "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44",
                    })),
                    message: "Google Calendar could help with this request.\n\nPlan and reference events from your calendar\n\nOpen ChatGPT to install it, then confirm here if you finish.".to_string(),
                    requested_schema: McpElicitationSchema {
                        schema_uri: None,
                        type_: McpElicitationObjectType::Object,
                        properties: BTreeMap::new(),
                        required: None,
                    },
                },
            }
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
        "https://chatgpt.com/apps/gmail/connector_68df038e0ba48191908c8434991bbac2",
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
            install_url: "https://chatgpt.com/apps/gmail/connector_68df038e0ba48191908c8434991bbac2",
        }
    );
}

#[test]
fn verified_connector_suggestion_completed_requires_installed_connector() {
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
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }];

    assert!(verified_connector_suggestion_completed(
        DiscoverableToolAction::Install,
        "calendar",
        &accessible_connectors,
    ));
    assert!(!verified_connector_suggestion_completed(
        DiscoverableToolAction::Install,
        "gmail",
        &accessible_connectors,
    ));
}

#[test]
fn verified_connector_suggestion_completed_requires_enabled_connector_for_enable() {
    let accessible_connectors = vec![
        AppInfo {
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
        },
        AppInfo {
            id: "gmail".to_string(),
            name: "Gmail".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        },
    ];

    assert!(!verified_connector_suggestion_completed(
        DiscoverableToolAction::Enable,
        "calendar",
        &accessible_connectors,
    ));
    assert!(verified_connector_suggestion_completed(
        DiscoverableToolAction::Enable,
        "gmail",
        &accessible_connectors,
    ));
}
