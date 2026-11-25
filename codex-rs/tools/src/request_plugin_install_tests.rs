use super::*;
use crate::DiscoverableConnectorInfo;
use crate::DiscoverablePluginInfo;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn build_request_plugin_install_elicitation_request_uses_expected_shape() {
    let connector = DiscoverableTool::from(DiscoverableConnectorInfo {
        id: "connector_2128aebfecb84f64a069897515042a44".to_string(),
        name: "Google Calendar".to_string(),
        description: Some("Plan events and schedules.".to_string()),
        install_url: Some(
            "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44"
                .to_string(),
        ),
    });

    let request = build_request_plugin_install_elicitation_request(
        "Plan and reference events from your calendar",
        &connector,
        "request_plugin_install_connector-calendar",
    );

    assert_eq!(
        request,
        ElicitationRequest::Form {
            meta: Some(json!(RequestPluginInstallMeta {
                codex_approval_kind: REQUEST_PLUGIN_INSTALL_APPROVAL_KIND_VALUE,
                persist: REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE,
                tool_type: DiscoverableToolType::Connector,
                suggest_type: DiscoverableToolAction::Install,
                suggest_reason: "Plan and reference events from your calendar",
                tool_id: "connector_2128aebfecb84f64a069897515042a44",
                tool_name: "Google Calendar",
                suggestion_id: None,
                install_url: Some(
                    "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44"
                ),
                remote_plugin_id: None,
                app_connector_ids: None,
            })),
            message: "Plan and reference events from your calendar".to_string(),
            requested_schema: json!({
                "type": "object",
                "properties": {},
            }),
        },
    );
}

#[test]
fn build_request_plugin_install_elicitation_request_injects_plugin_metadata() {
    let plugin = DiscoverableTool::Plugin(Box::new(DiscoverablePluginInfo {
        id: "sample@openai-curated-remote".to_string(),
        remote_plugin_id: Some("plugins~Plugin_sample".to_string()),
        name: "Sample Plugin".to_string(),
        description: Some("Includes skills, MCP servers, and apps.".to_string()),
        has_skills: true,
        mcp_server_names: vec!["sample-docs".to_string()],
        app_connector_ids: vec!["connector_calendar".to_string()],
    }));

    let request = build_request_plugin_install_elicitation_request(
        "Use the sample plugin's skills and MCP server",
        &plugin,
        "request_plugin_install_install-sample",
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
                suggestion_id: Some("request_plugin_install_install-sample"),
                install_url: None,
                remote_plugin_id: Some("plugins~Plugin_sample"),
                app_connector_ids: Some(&["connector_calendar".to_string()]),
            })),
            message: "Use the sample plugin's skills and MCP server".to_string(),
            requested_schema: json!({
                "type": "object",
                "properties": {},
            }),
        },
    );
}

#[test]
fn build_request_plugin_install_meta_uses_expected_shape() {
    let connector = DiscoverableTool::from(DiscoverableConnectorInfo {
        id: "connector_68df038e0ba48191908c8434991bbac2".to_string(),
        name: "Gmail".to_string(),
        description: None,
        install_url: Some(
            "https://chatgpt.com/apps/gmail/connector_68df038e0ba48191908c8434991bbac2".to_string(),
        ),
    });
    let meta = build_request_plugin_install_meta(
        "Find and reference emails from your inbox",
        &connector,
        "request_plugin_install_connector-gmail",
    );

    assert_eq!(
        meta,
        RequestPluginInstallMeta {
            codex_approval_kind: REQUEST_PLUGIN_INSTALL_APPROVAL_KIND_VALUE,
            persist: REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE,
            tool_type: DiscoverableToolType::Connector,
            suggest_type: DiscoverableToolAction::Install,
            suggest_reason: "Find and reference emails from your inbox",
            tool_id: "connector_68df038e0ba48191908c8434991bbac2",
            tool_name: "Gmail",
            suggestion_id: None,
            install_url: Some(
                "https://chatgpt.com/apps/gmail/connector_68df038e0ba48191908c8434991bbac2"
            ),
            remote_plugin_id: None,
            app_connector_ids: None,
        },
    );
}
