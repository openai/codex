use super::*;
use crate::plugins::PluginInstallRequest;
use crate::plugins::PluginsManager;
use crate::plugins::test_support::load_plugins_config;
use crate::plugins::test_support::write_curated_plugin_sha;
use crate::plugins::test_support::write_openai_curated_marketplace;
use crate::plugins::test_support::write_plugins_feature_config;
use crate::tools::discoverable::DiscoverablePluginInfo;
use crate::tools::discoverable::filter_tool_suggest_discoverable_tools_for_client;
use codex_app_server_protocol::AppInfo;
use codex_tools::DiscoverableToolAction;
use codex_tools::DiscoverableToolType;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use tempfile::tempdir;

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
        }
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
            server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
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
        }
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

#[tokio::test]
async fn verified_plugin_suggestion_completed_requires_installed_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = crate::plugins::curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample"]);
    write_curated_plugin_sha(codex_home.path());
    write_plugins_feature_config(codex_home.path());

    let config = load_plugins_config(codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());

    assert!(!verified_plugin_suggestion_completed(
        "sample@openai-curated",
        &config,
        &plugins_manager,
    ));

    plugins_manager
        .install_plugin(PluginInstallRequest {
            plugin_name: "sample".to_string(),
            marketplace_path: AbsolutePathBuf::try_from(
                curated_root.join(".agents/plugins/marketplace.json"),
            )
            .expect("marketplace path"),
        })
        .await
        .expect("plugin should install");

    let refreshed_config = load_plugins_config(codex_home.path()).await;
    assert!(verified_plugin_suggestion_completed(
        "sample@openai-curated",
        &refreshed_config,
        &plugins_manager,
    ));
}
