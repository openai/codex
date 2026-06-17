use super::*;
use codex_tools::DiscoverablePluginInfo;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn request_plugin_installs_result_keeps_model_response_minimal() {
    assert_eq!(
        json!(RequestPluginInstallsResult {
            completed: false,
            user_confirmed: false,
            action_type: DiscoverableToolAction::Install,
            entries: vec![RequestPluginInstallEntryResult {
                tool_type: DiscoverableToolType::Plugin,
                tool_id: "apollo@openai-curated-remote".to_string(),
                tool_name: "Apollo".to_string(),
                completed: false,
            }],
        }),
        json!({
            "completed": false,
            "user_confirmed": false,
            "action_type": "install",
            "entries": [
                {
                    "tool_type": "plugin",
                    "tool_id": "apollo@openai-curated-remote",
                    "tool_name": "Apollo",
                    "completed": false,
                },
            ],
        }),
    );
}

#[test]
fn build_request_plugin_installs_elicitation_request_uses_flat_entries_shape() {
    let args = RequestPluginInstallsArgs {
        action_type: DiscoverableToolAction::Install,
        entries: Some(vec![RequestPluginInstallPickerEntry {
            tool_id: "connector_2128aebfecb84f64a069897515042a44".to_string(),
            tool_type: DiscoverableToolType::Connector,
        }]),
        categories: None,
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
    let resolved_entries = [RequestPluginInstallResolvedPickerEntry {
        category_index: None,
        tool: connector,
    }];

    let request = build_request_plugin_installs_elicitation_request(
        "codex-apps",
        "thread-1".to_string(),
        "turn-1".to_string(),
        &args,
        &resolved_entries,
    );

    assert_eq!(
        request,
        McpServerElicitationRequestParams {
            thread_id: "thread-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            server_name: "codex-apps".to_string(),
            request: McpServerElicitationRequest::Form {
                meta: Some(json!(RequestPluginInstallsMeta {
                    codex_approval_kind: REQUEST_PLUGIN_INSTALL_APPROVAL_KIND_VALUE,
                    persist: Some(REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE),
                    suggest_type: DiscoverableToolAction::Install,
                    entries: Some(vec![RequestPluginInstallEntryMeta {
                        tool_id: "connector_2128aebfecb84f64a069897515042a44",
                        tool_name: "Google Calendar",
                        tool_type: DiscoverableToolType::Connector,
                        description: Some("Plan events and schedules."),
                        install_url: Some(
                            "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44"
                        ),
                        remote_plugin_id: None,
                        app_connector_ids: None,
                    }]),
                    categories: None,
                    legacy_single_entry: Some(RequestPluginInstallLegacyMeta {
                        tool_type: DiscoverableToolType::Connector,
                        suggest_reason: "Plan events and schedules.",
                        tool_id: "connector_2128aebfecb84f64a069897515042a44",
                        tool_name: "Google Calendar",
                        install_url: Some(
                            "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44"
                        ),
                        remote_plugin_id: None,
                        app_connector_ids: None,
                    }),
                })),
                message: "Choose integrations".to_string(),
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
fn build_request_plugin_installs_elicitation_request_injects_plugin_metadata() {
    let args = RequestPluginInstallsArgs {
        action_type: DiscoverableToolAction::Install,
        entries: Some(vec![RequestPluginInstallPickerEntry {
            tool_id: "sample@openai-curated-remote".to_string(),
            tool_type: DiscoverableToolType::Plugin,
        }]),
        categories: None,
    };
    let plugin = DiscoverableTool::Plugin(Box::new(DiscoverablePluginInfo {
        id: "sample@openai-curated-remote".to_string(),
        remote_plugin_id: Some("plugins~Plugin_sample".to_string()),
        name: "Sample Plugin".to_string(),
        description: Some("Includes skills, MCP servers, and apps.".to_string()),
        has_skills: true,
        mcp_server_names: vec!["sample-docs".to_string()],
        app_connector_ids: vec!["connector_calendar".to_string()],
    }));
    let resolved_entries = [RequestPluginInstallResolvedPickerEntry {
        category_index: None,
        tool: plugin,
    }];

    let request = build_request_plugin_installs_elicitation_request(
        "codex-apps",
        "thread-1".to_string(),
        "turn-1".to_string(),
        &args,
        &resolved_entries,
    );

    assert_eq!(
        request,
        McpServerElicitationRequestParams {
            thread_id: "thread-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            server_name: "codex-apps".to_string(),
            request: McpServerElicitationRequest::Form {
                meta: Some(json!(RequestPluginInstallsMeta {
                    codex_approval_kind: REQUEST_PLUGIN_INSTALL_APPROVAL_KIND_VALUE,
                    persist: Some(REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE),
                    suggest_type: DiscoverableToolAction::Install,
                    entries: Some(vec![RequestPluginInstallEntryMeta {
                        tool_id: "sample@openai-curated-remote",
                        tool_name: "Sample Plugin",
                        tool_type: DiscoverableToolType::Plugin,
                        description: Some("Includes skills, MCP servers, and apps."),
                        install_url: None,
                        remote_plugin_id: Some("plugins~Plugin_sample"),
                        app_connector_ids: Some(&["connector_calendar".to_string()]),
                    }]),
                    categories: None,
                    legacy_single_entry: Some(RequestPluginInstallLegacyMeta {
                        tool_type: DiscoverableToolType::Plugin,
                        suggest_reason: "Includes skills, MCP servers, and apps.",
                        tool_id: "sample@openai-curated-remote",
                        tool_name: "Sample Plugin",
                        install_url: None,
                        remote_plugin_id: Some("plugins~Plugin_sample"),
                        app_connector_ids: Some(&["connector_calendar".to_string()]),
                    }),
                })),
                message: "Choose integrations".to_string(),
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
fn build_request_plugin_installs_elicitation_request_uses_categories_shape() {
    let args = RequestPluginInstallsArgs {
        action_type: DiscoverableToolAction::Install,
        entries: None,
        categories: Some(vec![RequestPluginInstallPickerCategory {
            title: "Calendar".to_string(),
            entries: vec![RequestPluginInstallPickerEntry {
                tool_id: "connector_2128aebfecb84f64a069897515042a44".to_string(),
                tool_type: DiscoverableToolType::Connector,
            }],
        }]),
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
    let resolved_entries = [RequestPluginInstallResolvedPickerEntry {
        category_index: Some(0),
        tool: connector,
    }];

    let request = build_request_plugin_installs_elicitation_request(
        "codex-apps",
        "thread-1".to_string(),
        "turn-1".to_string(),
        &args,
        &resolved_entries,
    );

    assert_eq!(
        request,
        McpServerElicitationRequestParams {
            thread_id: "thread-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            server_name: "codex-apps".to_string(),
            request: McpServerElicitationRequest::Form {
                meta: Some(json!(RequestPluginInstallsMeta {
                    codex_approval_kind: REQUEST_PLUGIN_INSTALL_APPROVAL_KIND_VALUE,
                    persist: Some(REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE),
                    suggest_type: DiscoverableToolAction::Install,
                    entries: None,
                    categories: Some(vec![RequestPluginInstallCategoryMeta {
                        title: "Calendar",
                        entries: vec![RequestPluginInstallEntryMeta {
                            tool_id: "connector_2128aebfecb84f64a069897515042a44",
                            tool_name: "Google Calendar",
                            tool_type: DiscoverableToolType::Connector,
                            description: Some("Plan events and schedules."),
                            install_url: Some(
                                "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44"
                            ),
                            remote_plugin_id: None,
                            app_connector_ids: None,
                        }],
                    }]),
                    legacy_single_entry: Some(RequestPluginInstallLegacyMeta {
                        tool_type: DiscoverableToolType::Connector,
                        suggest_reason: "Plan events and schedules.",
                        tool_id: "connector_2128aebfecb84f64a069897515042a44",
                        tool_name: "Google Calendar",
                        install_url: Some(
                            "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44"
                        ),
                        remote_plugin_id: None,
                        app_connector_ids: None,
                    }),
                })),
                message: "Choose integrations".to_string(),
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
fn verified_connector_install_completed_requires_accessible_connector() {
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

    assert!(verified_connector_install_completed(
        "calendar",
        &accessible_connectors,
    ));
    assert!(!verified_connector_install_completed(
        "gmail",
        &accessible_connectors,
    ));
}
#[test]
fn all_requested_connectors_picked_up_requires_every_expected_connector() {
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

    assert!(all_requested_connectors_picked_up(
        &["calendar".to_string()],
        &accessible_connectors,
    ));
    assert!(!all_requested_connectors_picked_up(
        &["calendar".to_string(), "gmail".to_string()],
        &accessible_connectors,
    ));
}
