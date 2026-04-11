use super::*;
use crate::config::CONFIG_TOML_FILE;
use crate::config::ConfigBuilder;
use crate::config_loader::AppRequirementToml;
use crate::config_loader::AppsRequirementsToml;
use crate::config_loader::CloudRequirementsLoader;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigRequirements;
use crate::config_loader::ConfigRequirementsToml;
use codex_config::types::AppConfig;
use codex_config::types::AppToolConfig;
use codex_config::types::AppToolsConfig;
use codex_config::types::AppsDefaultConfig;
use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Tool;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
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
        tools: Vec::new(),
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

fn app_tool_info(tool_name: &str, description: Option<&str>) -> AppToolInfo {
    AppToolInfo {
        name: tool_name.to_string(),
        description: description.map(ToOwned::to_owned),
        input_schema: serde_json::Value::Object(JsonObject::default()),
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
        tools: Vec::new(),
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
        callable_name: tool_name.to_string(),
        callable_namespace: tool_namespace,
        server_instructions: None,
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
            tools: Vec::new(),
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
                /*connector_name*/ None,
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
                callable_name: "echo".to_string(),
                callable_namespace: "sample".to_string(),
                server_instructions: None,
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
            tools: vec![
                app_tool_info("calendar_create_event", None),
                app_tool_info("calendar_list_events", None),
            ],
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
    let _ = config.features.set_enabled(Feature::Apps, /*enabled*/ true);
    let cache_key = accessible_connectors_cache_key(&config, /*auth*/ None);
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
        refresh_accessible_connectors_cache_from_mcp_tools(&config, /*auth*/ None, &tools);
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
            tools: vec![app_tool_info("calendar_list_events", None)],
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
            tools: Vec::new(),
        }]
    );
}

#[test]
fn accessible_connectors_from_mcp_tools_preserves_description() {
    let mcp_tools = HashMap::from([(
        "mcp__codex_apps__calendar_create_event".to_string(),
        ToolInfo {
            server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
            callable_name: "calendar_create_event".to_string(),
            callable_namespace: "mcp__codex_apps__calendar".to_string(),
            server_instructions: None,
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
            tools: vec![app_tool_info(
                "calendar_create_event",
                Some("Create a calendar event"),
            )],
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
        /*tool_title*/ None,
        Some(&annotations(Some(true), /*open_world_hint*/ None)),
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
    assert!(!app_is_enabled(&apps_config, /*connector_id*/ None));
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
fn requirements_disabled_connector_overrides_enabled_connector() {
    let mut effective_apps = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "connector_123123".to_string(),
            AppConfig {
                enabled: true,
                ..Default::default()
            },
        )]),
    };
    let requirements_apps = AppsRequirementsToml {
        apps: BTreeMap::from([(
            "connector_123123".to_string(),
            AppRequirementToml {
                enabled: Some(false),
            },
        )]),
    };

    apply_requirements_apps_constraints(&mut effective_apps, Some(&requirements_apps));

    assert_eq!(
        effective_apps
            .apps
            .get("connector_123123")
            .map(|app| app.enabled),
        Some(false)
    );
}

#[test]
fn requirements_enabled_does_not_override_disabled_connector() {
    let mut effective_apps = AppsConfigToml {
        default: None,
        apps: HashMap::from([(
            "connector_123123".to_string(),
            AppConfig {
                enabled: false,
                ..Default::default()
            },
        )]),
    };
    let requirements_apps = AppsRequirementsToml {
        apps: BTreeMap::from([(
            "connector_123123".to_string(),
            AppRequirementToml {
                enabled: Some(true),
            },
        )]),
    };

    apply_requirements_apps_constraints(&mut effective_apps, Some(&requirements_apps));

    assert_eq!(
        effective_apps
            .apps
            .get("connector_123123")
            .map(|app| app.enabled),
        Some(false)
    );
}

#[tokio::test]
async fn cloud_requirements_disable_connector_overrides_user_apps_config() {
    let codex_home = tempdir().expect("tempdir should succeed");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"
[apps.connector_123123]
enabled = true
"#,
    )
    .expect("write config");

    let requirements = ConfigRequirementsToml {
        apps: Some(AppsRequirementsToml {
            apps: BTreeMap::from([(
                "connector_123123".to_string(),
                AppRequirementToml {
                    enabled: Some(false),
                },
            )]),
        }),
        ..Default::default()
    };

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cloud_requirements(CloudRequirementsLoader::new(async move {
            Ok(Some(requirements))
        }))
        .build()
        .await
        .expect("config should build");

    let policy = app_tool_policy(
        &config,
        Some("connector_123123"),
        "events.list",
        /*tool_title*/ None,
        /*annotations*/ None,
    );
    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[tokio::test]
async fn cloud_requirements_disable_connector_applies_without_user_apps_table() {
    let codex_home = tempdir().expect("tempdir should succeed");
    std::fs::write(codex_home.path().join(CONFIG_TOML_FILE), "").expect("write config");

    let requirements = ConfigRequirementsToml {
        apps: Some(AppsRequirementsToml {
            apps: BTreeMap::from([(
                "connector_123123".to_string(),
                AppRequirementToml {
                    enabled: Some(false),
                },
            )]),
        }),
        ..Default::default()
    };

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cloud_requirements(CloudRequirementsLoader::new(async move {
            Ok(Some(requirements))
        }))
        .build()
        .await
        .expect("config should build");

    let policy = app_tool_policy(
        &config,
        Some("connector_123123"),
        "events.list",
        /*tool_title*/ None,
        /*annotations*/ None,
    );
    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[tokio::test]
async fn local_requirements_disable_connector_overrides_user_apps_config() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let config_toml_path =
        AbsolutePathBuf::try_from(codex_home.path().join(CONFIG_TOML_FILE)).expect("abs path");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("config should build");

    let requirements = ConfigRequirementsToml {
        apps: Some(AppsRequirementsToml {
            apps: BTreeMap::from([(
                "connector_123123".to_string(),
                AppRequirementToml {
                    enabled: Some(false),
                },
            )]),
        }),
        ..Default::default()
    };
    config.config_layer_stack =
        ConfigLayerStack::new(Vec::new(), ConfigRequirements::default(), requirements)
            .expect("requirements stack")
            .with_user_config(
                &config_toml_path,
                toml::from_str::<toml::Value>(
                    r#"
[apps.connector_123123]
enabled = true
"#,
                )
                .expect("apps config"),
            );

    let policy = app_tool_policy(
        &config,
        Some("connector_123123"),
        "events.list",
        /*tool_title*/ None,
        /*annotations*/ None,
    );
    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[tokio::test]
async fn local_requirements_disable_connector_applies_without_user_apps_table() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("config should build");

    let requirements = ConfigRequirementsToml {
        apps: Some(AppsRequirementsToml {
            apps: BTreeMap::from([(
                "connector_123123".to_string(),
                AppRequirementToml {
                    enabled: Some(false),
                },
            )]),
        }),
        ..Default::default()
    };
    config.config_layer_stack =
        ConfigLayerStack::new(Vec::new(), ConfigRequirements::default(), requirements)
            .expect("requirements stack");

    let policy = app_tool_policy(
        &config,
        Some("connector_123123"),
        "events.list",
        /*tool_title*/ None,
        /*annotations*/ None,
    );
    assert_eq!(
        policy,
        AppToolPolicy {
            enabled: false,
            approval: AppToolApproval::Auto,
        }
    );
}

#[tokio::test]
async fn with_app_enabled_state_preserves_unrelated_disabled_connector() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("config should build");

    let requirements = ConfigRequirementsToml {
        apps: Some(AppsRequirementsToml {
            apps: BTreeMap::from([(
                "connector_drive".to_string(),
                AppRequirementToml {
                    enabled: Some(false),
                },
            )]),
        }),
        ..Default::default()
    };
    config.config_layer_stack =
        ConfigLayerStack::new(Vec::new(), ConfigRequirements::default(), requirements)
            .expect("requirements stack");

    let mut slack = app("connector_slack");
    slack.is_enabled = false;

    let mut drive = app("connector_drive");
    drive.is_enabled = false;

    assert_eq!(
        with_app_enabled_state(vec![slack.clone(), app("connector_drive")], &config),
        vec![slack, drive]
    );
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
        /*tool_title*/ None,
        Some(&annotations(
            /*destructive_hint*/ None, /*open_world_hint*/ None,
        )),
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
        /*tool_title*/ None,
        Some(&annotations(
            /*destructive_hint*/ None, /*open_world_hint*/ None,
        )),
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
        /*tool_title*/ None,
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
        /*tool_title*/ None,
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
        /*tool_title*/ None,
        Some(&annotations(
            /*destructive_hint*/ None, /*open_world_hint*/ None,
        )),
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
        /*tool_title*/ None,
        Some(&annotations(
            /*destructive_hint*/ None, /*open_world_hint*/ None,
        )),
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
        app("connector_3f8d1a79f27c4c7ba1a897ab13bf37dc"),
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

#[tokio::test]
async fn tool_suggest_connector_ids_include_configured_tool_suggest_discoverables() {
    let codex_home = tempdir().expect("tempdir should succeed");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"
[tool_suggest]
discoverables = [
  { type = "connector", id = "connector_2128aebfecb84f64a069897515042a44" },
  { type = "plugin", id = "slack@openai-curated" },
  { type = "connector", id = "   " }
]
"#,
    )
    .expect("write config");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config should load");

    assert_eq!(
        tool_suggest_connector_ids(&config),
        HashSet::from(["connector_2128aebfecb84f64a069897515042a44".to_string()])
    );
}

#[test]
fn filter_tool_suggest_discoverable_connectors_keeps_only_plugin_backed_uninstalled_apps() {
    let filtered = filter_tool_suggest_discoverable_connectors(
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
        &HashSet::from([
            "connector_2128aebfecb84f64a069897515042a44".to_string(),
            "connector_68df038e0ba48191908c8434991bbac2".to_string(),
        ]),
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
fn filter_tool_suggest_discoverable_connectors_excludes_accessible_apps_even_when_disabled() {
    let filtered = filter_tool_suggest_discoverable_connectors(
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
        &HashSet::from([
            "connector_2128aebfecb84f64a069897515042a44".to_string(),
            "connector_68df038e0ba48191908c8434991bbac2".to_string(),
        ]),
    );

    assert_eq!(filtered, Vec::<AppInfo>::new());
}
