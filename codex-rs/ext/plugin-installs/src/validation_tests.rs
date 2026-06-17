use super::*;

use codex_app_server_protocol::AppInfo;
use codex_tools::DiscoverableToolAction;
use pretty_assertions::assert_eq;

use crate::RequestPluginInstallPickerCategory;
use crate::RequestPluginInstallPickerEntry;
use crate::RequestPluginInstallsArgs;

#[test]
fn validate_request_plugin_install_picker_args_supports_categories() {
    let args = RequestPluginInstallsArgs {
        action_type: DiscoverableToolAction::Install,
        entries: None,
        categories: Some(vec![RequestPluginInstallPickerCategory {
            title: "Calendar".to_string(),
            entries: vec![RequestPluginInstallPickerEntry {
                tool_id: "connector_calendar".to_string(),
                tool_type: DiscoverableToolType::Connector,
            }],
        }]),
    };
    let discoverable_tools = vec![connector_tool("connector_calendar", "Google Calendar")];

    let resolved_entries = validate_request_plugin_install_picker_args(
        &args,
        &discoverable_tools,
        /*app_server_client_name*/ None,
        ToolSuggestPresentation::ListTool,
    )
    .expect("categorized picker args");

    assert_eq!(resolved_entries.len(), 1);
    assert_eq!(resolved_entries[0].category_index, Some(0));
    assert_eq!(resolved_entries[0].tool.id(), "connector_calendar");
}

#[test]
fn validate_request_plugin_install_picker_args_rejects_mixed_sources() {
    let entry = RequestPluginInstallPickerEntry {
        tool_id: "connector_calendar".to_string(),
        tool_type: DiscoverableToolType::Connector,
    };
    let args = RequestPluginInstallsArgs {
        action_type: DiscoverableToolAction::Install,
        entries: Some(vec![entry]),
        categories: Some(vec![RequestPluginInstallPickerCategory {
            title: "Calendar".to_string(),
            entries: vec![RequestPluginInstallPickerEntry {
                tool_id: "connector_calendar".to_string(),
                tool_type: DiscoverableToolType::Connector,
            }],
        }]),
    };
    let discoverable_tools = vec![connector_tool("connector_calendar", "Google Calendar")];

    assert_eq!(
        validate_request_plugin_install_picker_args(
            &args,
            &discoverable_tools,
            /*app_server_client_name*/ None,
            ToolSuggestPresentation::ListTool,
        )
        .expect_err("mixed picker args"),
        FunctionCallError::RespondToModel(
            "picker install requests must include exactly one of entries or categories".to_string(),
        ),
    );
}

#[test]
fn validate_request_plugin_install_picker_args_rejects_duplicate_tools() {
    let entry = RequestPluginInstallPickerEntry {
        tool_id: "connector_calendar".to_string(),
        tool_type: DiscoverableToolType::Connector,
    };
    let args = RequestPluginInstallsArgs {
        action_type: DiscoverableToolAction::Install,
        entries: None,
        categories: Some(vec![
            RequestPluginInstallPickerCategory {
                title: "Calendar".to_string(),
                entries: vec![entry],
            },
            RequestPluginInstallPickerCategory {
                title: "Meetings".to_string(),
                entries: vec![RequestPluginInstallPickerEntry {
                    tool_id: "connector_calendar".to_string(),
                    tool_type: DiscoverableToolType::Connector,
                }],
            },
        ]),
    };
    let discoverable_tools = vec![connector_tool("connector_calendar", "Google Calendar")];

    assert_eq!(
        validate_request_plugin_install_picker_args(
            &args,
            &discoverable_tools,
            /*app_server_client_name*/ None,
            ToolSuggestPresentation::ListTool,
        )
        .expect_err("duplicate picker tool"),
        FunctionCallError::RespondToModel(
            "picker install requests must not repeat a tool_type/tool_id pair".to_string(),
        ),
    );
}

#[test]
fn validate_request_plugin_install_picker_args_rejects_multi_tool_tui_requests() {
    let args = RequestPluginInstallsArgs {
        action_type: DiscoverableToolAction::Install,
        entries: Some(vec![
            RequestPluginInstallPickerEntry {
                tool_id: "connector_calendar".to_string(),
                tool_type: DiscoverableToolType::Connector,
            },
            RequestPluginInstallPickerEntry {
                tool_id: "connector_gmail".to_string(),
                tool_type: DiscoverableToolType::Connector,
            },
        ]),
        categories: None,
    };
    let discoverable_tools = vec![
        connector_tool("connector_calendar", "Google Calendar"),
        connector_tool("connector_gmail", "Gmail"),
    ];

    assert_eq!(
        validate_request_plugin_install_picker_args(
            &args,
            &discoverable_tools,
            Some("codex-tui"),
            ToolSuggestPresentation::ListTool,
        )
        .expect_err("multi-tool TUI request"),
        FunctionCallError::RespondToModel(
            "multi-tool install requests are not available in codex-tui yet".to_string(),
        ),
    );
}

#[test]
fn validate_request_plugin_install_picker_args_caps_entries() {
    let entries = || {
        (0..=MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES)
            .map(|index| RequestPluginInstallPickerEntry {
                tool_id: format!("connector_{index}"),
                tool_type: DiscoverableToolType::Connector,
            })
            .collect()
    };

    for args in [
        RequestPluginInstallsArgs {
            action_type: DiscoverableToolAction::Install,
            entries: Some(entries()),
            categories: None,
        },
        RequestPluginInstallsArgs {
            action_type: DiscoverableToolAction::Install,
            entries: None,
            categories: Some(vec![RequestPluginInstallPickerCategory {
                title: "Connectors".to_string(),
                entries: entries(),
            }]),
        },
    ] {
        assert_eq!(
            validate_request_plugin_install_picker_args(
                &args,
                &[],
                /*app_server_client_name*/ None,
                ToolSuggestPresentation::ListTool,
            )
            .expect_err("oversized picker args"),
            FunctionCallError::RespondToModel(format!(
                "picker install requests support at most {MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES} entries"
            )),
        );
    }
}

#[test]
fn picker_completion_only_requires_one_completed_entry() {
    let entries = vec![
        RequestPluginInstallEntryResult {
            tool_type: DiscoverableToolType::Connector,
            tool_id: "connector_salesforce".to_string(),
            tool_name: "Salesforce".to_string(),
            completed: true,
        },
        RequestPluginInstallEntryResult {
            tool_type: DiscoverableToolType::Connector,
            tool_id: "connector_hubspot".to_string(),
            tool_name: "HubSpot".to_string(),
            completed: false,
        },
    ];

    assert!(request_plugin_install_picker_completed(&entries));
}

fn connector_tool(id: &str, name: &str) -> DiscoverableTool {
    DiscoverableTool::Connector(Box::new(AppInfo {
        id: id.to_string(),
        name: name.to_string(),
        description: None,
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: None,
        is_accessible: false,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }))
}
