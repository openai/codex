use super::*;
use crate::config::ConfigBuilder;
use crate::config::types::AppConfig;
use crate::config::types::AppToolConfig;
use crate::config::types::AppToolsConfig;
use crate::config::types::AppsDefaultConfig;
use crate::features::Feature;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Tool;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::tempdir;

fn annotations(destructive_hint: Option<bool>, open_world_hint: Option<bool>) -> ToolAnnotations {
    ToolAnnotations {
        destructive_hint,
        idempotent_hint: None,
        open_world_hint,
        read_only_hint: None,
        title: None,
    }
}

fn app(id: &str) -> AppInfo {
    AppInfo {
        id: id.to_string(),
        name: id.to_string(),
        description: None,
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        install_url: None,
        branding: None,
        app_metadata: None,
        labels: None,
        is_accessible: false,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }
}

fn named_app(id: &str, name: &str) -> AppInfo {
    AppInfo {
        id: id.to_string(),
        name: name.to_string(),
        install_url: Some(connector_install_url(name, id)),
        ..app(id)
    }
}

fn plugin_names(names: &[&str]) -> Vec<String> {
    names.iter().map(ToString::to_string).collect()
}

fn test_tool_definition(tool_name: &str) -> Tool {
    Tool {
        name: tool_name.to_string().into(),
        title: None,
        description: None,
        input_schema: Arc::new(JsonObject::default()),
        output_schema: None,
        annotations: None,
        execution: None,
        icons: None,
        meta: None,
    }
}

fn google_calendar_accessible_connector(plugin_display_names: &[&str]) -> AppInfo {
    AppInfo {
        id: "calendar".to_string(),
        name: "Google Calendar".to_string(),
        description: Some("Plan events".to_string()),
        logo_url: Some("https://example.com/logo.png".to_string()),
        logo_url_dark: Some("https://example.com/logo-dark.png".to_string()),
        distribution_channel: Some("workspace".to_string()),
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: None,
        is_accessible: true,
        is_enabled: true,
        plugin_display_names: plugin_names(plugin_display_names),
    }
}

fn codex_app_tool(
    tool_name: &str,
    connector_id: &str,
    connector_name: Option<&str>,
    plugin_display_names: &[&str],
) -> ToolInfo {
    let tool_namespace = connector_name
        .map(sanitize_name)
        .map(|connector_name| format!("mcp__{CODEX_APPS_MCP_SERVER_NAME}__{connector_name}"))
        .unwrap_or_else(|| CODEX_APPS_MCP_SERVER_NAME.to_string());

    ToolInfo {
        server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        tool_name: tool_name.to_string(),
        tool_namespace,
        tool: test_tool_definition(tool_name),
        connector_id: Some(connector_id.to_string()),
        connector_name: connector_name.map(ToOwned::to_owned),
        connector_description: None,
        plugin_display_names: plugin_names(plugin_display_names),
    }
}

fn with_accessible_connectors_cache_cleared<R>(f: impl FnOnce() -> R) -> R {
    let previous = {
        let mut cache_guard = ACCESSIBLE_CONNECTORS_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache_guard.take()
    };
    let result = f();
    let mut cache_guard = ACCESSIBLE_CONNECTORS_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *cache_guard = previous;
    result
}

#[test]
fn merge_connectors_replaces_plugin_placeholder_name_with_accessible_name() {
    let plugin = plugin_app_to_app_info(AppConnectorId("calendar".to_string()));
    let accessible = google_calendar_accessible_connector(&[]);

    let merged = merge_connectors(vec![plugin], vec![accessible]);

    assert_eq!(
        merged,
        vec![AppInfo {
            id: "calendar".to_string(),
            name: "Google Calendar".to_string(),
            description: Some("Plan events".to_string()),
            logo_url: Some("https://example.com/logo.png".to_string()),
            logo_url_dark: Some("https://example.com/logo-dark.png".to_string()),
            distribution_channel: Some("workspace".to_string()),
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some(connector_install_url("calendar", "calendar")),
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }]
    );
    assert_eq!(connector_mention_slug(&merged[0]), "google-calendar");
}

#[test]
fn accessible_connectors_from_mcp_tools_carries_plugin_display_names() {
    let tools = HashMap::from([
        (
            "mcp__codex_apps__calendar_list_events".to_string(),
            codex_app_tool(
                "calendar_list_events",
                "calendar",
                None,
                &["sample", "sample"],
            ),
        ),
        (
            "mcp__codex_apps__calendar_create_event".to_string(),
            codex_app_tool(
                "calendar_create_event",
                "calendar",
                Some("Google Calendar"),
                &["beta", "sample"],
            ),
        ),
        (
            "mcp__sample__echo".to_string(),
            ToolInfo {
                server_name: "sample".to_string(),
                tool_name: "echo".to_string(),
                tool_namespace: "sample".to_string(),
                tool: test_tool_definition("echo"),
                connector_id: None,
                connector_name: None,
                connector_description: None,
                plugin_display_names: plugin_names(&["ignored"]),
            },
        ),
    ]);

    let connectors = accessible_connectors_from_mcp_tools(&tools);

    assert_eq!(
        connectors,
        vec![AppInfo {
            id: "calendar".to_string(),
            name: "Google Calendar".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: Some(connector_install_url("Google Calendar", "calendar")),
            branding: None,
            app_metadata: None,
            labels: None,
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: plugin_names(&["beta", "sample"]),
        }]
    );
}

#[tokio::test]
async fn refresh_accessible_connectors_cache_from_mcp_tools_writes_latest_installed_apps() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config should load");
    let _ = config.features.set_enabled(Feature::Apps, true);
    let cache_key = accessible_connectors_cache_key(&config, None);
    let tools = HashMap::from([
        (
            "mcp__codex_apps__calendar_list_events".to_string(),
            codex_app_tool(
                "calendar_list_events",
                "calendar",
                Some("Google Calendar"),
                &["calendar-plugin"],
            ),
        ),
        (
            "mcp__codex_apps__openai_hidden".to_string(),
            codex_app_tool(
                "openai_hidden",
                "connector_openai_hidden",
                Some("Hidden"),
                &[],
            ),
        ),
    ]);

    let cached = with_accessible_connectors_cache_cleared(|| {
        refresh_accessible_connectors_cache_from_mcp_tools(&config, None, &tools);
        read_cached_accessible_connectors(&cache_key).expect("cache should be populated")
    });

    assert_eq!(
        cached,
        vec![AppInfo {
            id: "calendar".to_string(),
            name: "Google Calendar".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: Some(connector_install_url("Google Calendar", "calendar")),
            branding: None,
            app_metadata: None,
            labels: None,
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: plugin_names(&["calendar-plugin"]),
        }]
    );
}

#[test]
fn merge_connectors_unions_and_dedupes_plugin_display_names() {
    let mut plugin = plugin_app_to_app_info(AppConnectorId("calendar".to_string()));
    plugin.plugin_display_names = plugin_names(&["sample", "alpha", "sample"]);

    let accessible = google_calendar_accessible_connector(&["beta", "alpha"]);

    let merged = merge_connectors(vec![plugin], vec![accessible]);

    assert_eq!(
        merged,
        vec![AppInfo {
            id: "calendar".to_string(),
            name: "Google Calendar".to_string(),
            description: Some("Plan events".to_string()),
            logo_url: Some("https://example.com/logo.png".to_string()),
            logo_url_dark: Some("https://example.com/logo-dark.png".to_string()),
            distribution_channel: Some("workspace".to_string()),
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some(connector_install_url("calendar", "calendar")),
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: plugin_names(&["alpha", "beta", "sample"]),
        }]
    );
}

#[test]
fn accessible_connectors_from_mcp_tools_preserves_description() {
    let mcp_tools = HashMap::from([(
        "mcp__codex_apps__calendar_create_event".to_string(),
        ToolInfo {
            server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
            tool_name: "calendar_create_event".to_string(),
            tool_namespace: "mcp__codex_apps__calendar".to_string(),
            tool: Tool {
                name: "calendar_create_event".to_string().into(),
                title: None,
                description: Some("Create a calendar event".into()),
                input_schema: Arc::new(JsonObject::default()),
                output_schema: None,
                annotations: None,
                execution: None,
                icons: None,
                meta: None,
            },
            connector_id: Some("calendar".to_string()),
            connector_name: Some("Calendar".to_string()),
            connector_description: Some("Plan events".to_string()),
            plugin_display_names: Vec::new(),
        },
    )]);

    assert_eq!(
        accessible_connectors_from_mcp_tools(&mcp_tools),
        vec![AppInfo {
            id: "calendar".to_string(),
            name: "Calendar".to_string(),
            description: Some("Plan events".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some(connector_install_url("Calendar", "calendar")),
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }]
    );
}

#[test]
fn app_tool_policy_uses_global_defaults_for_destructive_hints() {
    let apps_config = AppsConfigToml {
        default: Some(AppsDefaultConfig {
            enabled: true,
            destructive_enabled: false,
            open_world_enabled: true,
        }),
        apps: HashMap::new(),
    };

    let policy = app_tool_policy_from_apps_config(
        Some(&apps_config),
        Some("calendar"),
        "events/create",
        None,
        Some(&annotations(Some(true), None)),
    );

    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn app_is_enabled_uses_default_for_unconfigured_apps() {
    let apps_config = AppsConfigToml {
        default: Some(AppsDefaultConfig {
            enabled: false,
            destructive_enabled: true,
            open_world_enabled: true,
        }),
        apps: HashMap::new(),
    };

    assert!(!app_is_enabled(&apps_config, Some("calendar")));
    assert!(!app_is_enabled(&apps_config, None));
}

#[test]
fn app_is_enabled_prefers_per_app_override_over_default() {
    let apps_config = AppsConfigToml {
        default: Some(AppsDefaultConfig {
            enabled: false,
            destructive_enabled: true,
            open_world_enabled: true,
        }),
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                destructive_enabled: None,
                open_world_enabled: None,
                default_tools_approval_mode: None,
                default_tools_enabled: None,
                tools: None,
            },
        )]),
    };

    assert!(app_is_enabled(&apps_config, Some("calendar")));
    assert!(!app_is_enabled(&apps_config, Some("drive")));
}

#[test]
fn app_tool_policy_honors_default_app_enabled_false() {
    let apps_config = AppsConfigToml {
        default: Some(AppsDefaultConfig {
            enabled: false,
            destructive_enabled: true,
            open_world_enabled: true,
        }),
        apps: HashMap::new(),
    };

    let policy = app_tool_policy_from_apps_config(
        Some(&apps_config),
        Some("calendar"),
        "events/list",
        None,
        Some(&annotations(None, None)),
    );

    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn app_tool_policy_allows_per_app_enable_when_default_is_disabled() {
    let apps_config = AppsConfigToml {
        default: Some(AppsDefaultConfig {
            enabled: false,
            destructive_enabled: true,
            open_world_enabled: true,
        }),
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                destructive_enabled: None,
                open_world_enabled: None,
                default_tools_approval_mode: None,
                default_tools_enabled: None,
                tools: None,
            },
        )]),
    };

    let policy = app_tool_policy_from_apps_config(
        Some(&apps_config),
        Some("calendar"),
        "events/list",
        None,
        Some(&annotations(None, None)),
    );

    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: true,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn app_tool_policy_per_tool_enabled_true_overrides_app_level_disable_flags() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                destructive_enabled: Some(false),
                open_world_enabled: Some(false),
                default_tools_approval_mode: None,
                default_tools_enabled: None,
                tools: Some(AppToolsConfig {
                    tools: HashMap::from([(
                        "events/create".to_string(),
                        AppToolConfig {
                            enabled: Some(true),
                            approval_mode: None,
                        },
                    )]),
                }),
            },
        )]),
    };

    let policy = app_tool_policy_from_apps_config(
        Some(&apps_config),
        Some("calendar"),
        "events/create",
        None,
        Some(&annotations(Some(true), Some(true))),
    );

    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: true,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn app_tool_policy_default_tools_enabled_true_overrides_app_level_tool_hints() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                destructive_enabled: Some(false),
                open_world_enabled: Some(false),
                default_tools_approval_mode: None,
                default_tools_enabled: Some(true),
                tools: None,
            },
        )]),
    };

    let policy = app_tool_policy_from_apps_config(
        Some(&apps_config),
        Some("calendar"),
        "events/create",
        None,
        Some(&annotations(Some(true), Some(true))),
    );

    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: true,
            approval: AppToolApproval::Auto,
        }
    );
}

#[test]
fn app_tool_policy_default_tools_enabled_false_overrides_app_level_tool_hints() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                destructive_enabled: Some(true),
                open_world_enabled: Some(true),
                default_tools_approval_mode: Some(AppToolApproval::Approve),
                default_tools_enabled: Some(false),
                tools: None,
            },
        )]),
    };

    let policy = app_tool_policy_from_apps_config(
        Some(&apps_config),
        Some("calendar"),
        "events/list",
        None,
        Some(&annotations(None, None)),
    );

    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Approve,
        }
    );
}

#[test]
fn app_tool_policy_uses_default_tools_approval_mode() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                destructive_enabled: None,
                open_world_enabled: None,
                default_tools_approval_mode: Some(AppToolApproval::Prompt),
                default_tools_enabled: None,
                tools: Some(AppToolsConfig {
                    tools: HashMap::new(),
                }),
            },
        )]),
    };

    let policy = app_tool_policy_from_apps_config(
        Some(&apps_config),
        Some("calendar"),
        "events/list",
        None,
        Some(&annotations(None, None)),
    );

    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: true,
            approval: AppToolApproval::Prompt,
        }
    );
}

#[test]
fn app_tool_policy_matches_prefix_stripped_tool_name_for_tool_config() {
    let apps_config = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "calendar".to_string(),
            AppConfig {
                enabled: true,
                destructive_enabled: Some(false),
                open_world_enabled: Some(false),
                default_tools_approval_mode: Some(AppToolApproval::Auto),
                default_tools_enabled: Some(false),
                tools: Some(AppToolsConfig {
                    tools: HashMap::from([(
                        "events/create".to_string(),
                        AppToolConfig {
                            enabled: Some(true),
                            approval_mode: Some(AppToolApproval::Approve),
                        },
                    )]),
                }),
            },
        )]),
    };

    let policy = app_tool_policy_from_apps_config(
        Some(&apps_config),
        Some("calendar"),
        "calendar_events/create",
        Some("events/create"),
        Some(&annotations(Some(true), Some(true))),
    );

    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: true,
            approval: AppToolApproval::Approve,
        }
    );
}

#[test]
fn filter_disallowed_connectors_allows_non_disallowed_connectors() {
    let filtered = filter_disallowed_connectors(vec![app("asdk_app_hidden"), app("alpha")]);
    assert_eq!(filtered, vec![app("asdk_app_hidden"), app("alpha")]);
}

#[test]
fn filter_disallowed_connectors_filters_openai_prefix() {
    let filtered = filter_disallowed_connectors(vec![
        app("connector_openai_foo"),
        app("connector_openai_bar"),
        app("gamma"),
    ]);
    assert_eq!(filtered, vec![app("gamma")]);
}

#[test]
fn filter_disallowed_connectors_filters_disallowed_connector_ids() {
    let filtered = filter_disallowed_connectors(vec![
        app("asdk_app_6938a94a61d881918ef32cb999ff937c"),
        app("delta"),
    ]);
    assert_eq!(filtered, vec![app("delta")]);
}

#[test]
fn first_party_chat_originator_filters_target_and_openai_prefixed_connectors() {
    let filtered = filter_disallowed_connectors_for_originator(
        vec![
            app("connector_openai_foo"),
            app("asdk_app_6938a94a61d881918ef32cb999ff937c"),
            app("connector_0f9c9d4592e54d0a9a12b3f44a1e2010"),
        ],
        "codex_atlas",
    );
    assert_eq!(
        filtered,
        vec![app("asdk_app_6938a94a61d881918ef32cb999ff937c")]
    );
}

#[test]
fn filter_tool_suggest_discoverable_tools_keeps_only_allowlisted_uninstalled_apps() {
    let filtered = filter_tool_suggest_discoverable_tools(
        vec![
            named_app(
                "connector_2128aebfecb84f64a069897515042a44",
                "Google Calendar",
            ),
            named_app("connector_68df038e0ba48191908c8434991bbac2", "Gmail"),
            named_app("connector_other", "Other"),
        ],
        &[AppInfo {
            is_accessible: true,
            ..named_app(
                "connector_2128aebfecb84f64a069897515042a44",
                "Google Calendar",
            )
        }],
    );

    assert_eq!(
        filtered,
        vec![named_app(
            "connector_68df038e0ba48191908c8434991bbac2",
            "Gmail",
        )]
    );
}

#[test]
fn filter_tool_suggest_discoverable_tools_keeps_disabled_accessible_apps() {
    let filtered = filter_tool_suggest_discoverable_tools(
        vec![
            named_app(
                "connector_2128aebfecb84f64a069897515042a44",
                "Google Calendar",
            ),
            named_app("connector_68df038e0ba48191908c8434991bbac2", "Gmail"),
        ],
        &[
            AppInfo {
                is_accessible: true,
                ..named_app(
                    "connector_2128aebfecb84f64a069897515042a44",
                    "Google Calendar",
                )
            },
            AppInfo {
                is_accessible: true,
                is_enabled: false,
                ..named_app("connector_68df038e0ba48191908c8434991bbac2", "Gmail")
            },
        ],
    );

    assert_eq!(
        filtered,
        vec![named_app(
            "connector_68df038e0ba48191908c8434991bbac2",
            "Gmail"
        )]
    );
}
