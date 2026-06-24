use super::*;
use crate::config::CONFIG_TOML_FILE;
use crate::config::ConfigBuilder;
use codex_config::AppRequirementToml;
use codex_config::AppsRequirementsToml;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_config::types::ApprovalsReviewer;
use codex_connectors::merge::plugin_connector_to_app_info;
use codex_connectors::metadata::connector_install_url;
use codex_connectors::metadata::sanitize_name;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Meta;
use rmcp::model::Tool;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::Arc;
use tempfile::tempdir;

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

fn plugin_names(names: &[&str]) -> Vec<String> {
    names.iter().map(ToString::to_string).collect()
}

fn test_tool_definition(tool_name: &str) -> Tool {
    Tool::new_with_raw(tool_name.to_string(), None, Arc::new(JsonObject::default()))
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
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: tool_name.to_string(),
        callable_namespace: tool_namespace,
        namespace_description: None,
        tool: test_tool_definition(tool_name),
        connector_id: Some(connector_id.to_string()),
        connector_name: connector_name.map(ToOwned::to_owned),
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
fn accessible_connectors_from_mcp_tools_carries_plugin_display_names() {
    let tools = vec![
        codex_app_tool(
            "calendar_list_events",
            "calendar",
            /*connector_name*/ None,
            &["sample", "sample"],
        ),
        codex_app_tool(
            "calendar_create_event",
            "calendar",
            Some("Google Calendar"),
            &["beta", "sample"],
        ),
        ToolInfo {
            server_name: "sample".to_string(),
            supports_parallel_tool_calls: false,
            server_origin: None,
            callable_name: "echo".to_string(),
            callable_namespace: "sample".to_string(),
            namespace_description: None,
            tool: test_tool_definition("echo"),
            connector_id: None,
            connector_name: None,
            plugin_display_names: plugin_names(&["ignored"]),
        },
    ];

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

#[test]
fn synthetic_links_are_exposed_to_the_agent_but_not_accessible_in_app_list() {
    let mut synthetic_tool = codex_app_tool("gmail_batch_read_email", "gmail", Some("Gmail"), &[]);
    synthetic_tool.tool.meta = Some(Meta(
        serde_json::json!({
            "resource_name": "gmail.batch_read_email",
            "_codex_apps": {
                "resource_uri": "/connector/gmail/batch_read_email",
                "contains_mcp_source": false,
                "synthetic_link": true
            }
        })
        .as_object()
        .expect("meta should be an object")
        .clone(),
    ));
    let tools = vec![
        synthetic_tool,
        codex_app_tool("calendar_list_events", "calendar", Some("Calendar"), &[]),
    ];

    let calendar = AppInfo {
        id: "calendar".to_string(),
        name: "Calendar".to_string(),
        description: None,
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        install_url: Some(connector_install_url("Calendar", "calendar")),
        branding: None,
        app_metadata: None,
        labels: None,
        is_accessible: true,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    };
    assert_eq!(
        accessible_connectors_for_app_list_from_mcp_tools(&tools),
        vec![calendar.clone()]
    );
    assert_eq!(
        accessible_connectors_from_mcp_tools(&tools),
        vec![
            calendar,
            AppInfo {
                id: "gmail".to_string(),
                name: "Gmail".to_string(),
                description: None,
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                install_url: Some(connector_install_url("Gmail", "gmail")),
                branding: None,
                app_metadata: None,
                labels: None,
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }
        ]
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
    let tools = vec![
        codex_app_tool(
            "calendar_list_events",
            "calendar",
            Some("Google Calendar"),
            &["calendar-plugin"],
        ),
        codex_app_tool(
            "openai_hidden",
            "connector_openai_hidden",
            Some("Hidden"),
            &[],
        ),
    ];

    let cached = with_accessible_connectors_cache_cleared(|| {
        refresh_accessible_connectors_cache_from_mcp_tools(&config, /*auth*/ None, &tools);
        read_cached_accessible_connectors(&cache_key).expect("cache should be populated")
    });

    assert_eq!(
        cached,
        vec![
            AppInfo {
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
            },
            AppInfo {
                id: "connector_openai_hidden".to_string(),
                name: "Hidden".to_string(),
                description: None,
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                install_url: Some(connector_install_url("Hidden", "connector_openai_hidden")),
                branding: None,
                app_metadata: None,
                labels: None,
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }
        ]
    );
}

#[test]
fn accessible_connectors_from_mcp_tools_preserves_description() {
    let mcp_tools = vec![ToolInfo {
        server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: "calendar_create_event".to_string(),
        callable_namespace: "mcp__codex_apps__calendar".to_string(),
        namespace_description: Some("Plan events".to_string()),
        tool: Tool::new(
            "calendar_create_event",
            "Create a calendar event",
            Arc::new(JsonObject::default()),
        ),
        connector_id: Some("calendar".to_string()),
        connector_name: Some("Calendar".to_string()),
        plugin_display_names: Vec::new(),
    }];

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

#[tokio::test]
async fn app_approvals_reviewer_uses_app_then_default_then_global() {
    for (global, app_default, app, expected_global, expected_default, expected_app) in [
        (
            "user",
            "auto_review",
            "user",
            ApprovalsReviewer::User,
            ApprovalsReviewer::AutoReview,
            ApprovalsReviewer::User,
        ),
        (
            "auto_review",
            "user",
            "auto_review",
            ApprovalsReviewer::AutoReview,
            ApprovalsReviewer::User,
            ApprovalsReviewer::AutoReview,
        ),
    ] {
        let codex_home = tempdir().expect("tempdir should succeed");
        std::fs::write(
            codex_home.path().join(CONFIG_TOML_FILE),
            format!(
                r#"
approvals_reviewer = "{global}"

[apps._default]
approvals_reviewer = "{app_default}"

[apps.calendar]
approvals_reviewer = "{app}"
"#
            ),
        )
        .expect("write config");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config should build");

        assert_eq!(
            mcp_approvals_reviewer(&config, CODEX_APPS_MCP_SERVER_NAME, Some("calendar")),
            expected_app
        );
        assert_eq!(
            mcp_approvals_reviewer(&config, CODEX_APPS_MCP_SERVER_NAME, Some("drive")),
            expected_default
        );
        assert_eq!(
            mcp_approvals_reviewer(
                &config,
                CODEX_APPS_MCP_SERVER_NAME,
                /*connector_id*/ None
            ),
            expected_default
        );
        assert_eq!(
            mcp_approvals_reviewer(&config, "custom_server", Some("calendar")),
            expected_global
        );
    }
}

#[tokio::test]
async fn default_app_approvals_reviewer_respects_global_reviewer_requirements() {
    let codex_home = tempdir().expect("tempdir should succeed");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"
approvals_reviewer = "auto_review"

[apps._default]
approvals_reviewer = "user"
"#,
    )
    .expect("write config");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                r#"allowed_approvals_reviewers = ["auto_review"]"#,
            ),
        )
        .build()
        .await
        .expect("config should build");

    assert_eq!(
        mcp_approvals_reviewer(&config, CODEX_APPS_MCP_SERVER_NAME, Some("calendar")),
        ApprovalsReviewer::AutoReview
    );
}

#[tokio::test]
async fn app_approvals_reviewer_respects_global_reviewer_requirements() {
    let codex_home = tempdir().expect("tempdir should succeed");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"
approvals_reviewer = "auto_review"

[apps.calendar]
approvals_reviewer = "user"
"#,
    )
    .expect("write config");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                r#"allowed_approvals_reviewers = ["auto_review"]"#,
            ),
        )
        .build()
        .await
        .expect("config should build");

    assert_eq!(
        mcp_approvals_reviewer(&config, CODEX_APPS_MCP_SERVER_NAME, Some("calendar")),
        ApprovalsReviewer::AutoReview
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
                    tools: None,
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
        tool_suggest_connector_ids(&config, &[]),
        HashSet::from(["connector_2128aebfecb84f64a069897515042a44".to_string()])
    );
}

#[tokio::test]
async fn tool_suggest_connector_ids_exclude_disabled_tool_suggestions() {
    let codex_home = tempdir().expect("tempdir should succeed");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"
[tool_suggest]
discoverables = [
  { type = "connector", id = "connector_calendar" },
  { type = "connector", id = "connector_gmail" }
]
disabled_tools = [
  { type = "connector", id = "connector_calendar" }
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
        tool_suggest_connector_ids(&config, &[]),
        HashSet::from(["connector_gmail".to_string()])
    );
}

#[tokio::test]
async fn tool_suggest_uses_connector_id_fallback_when_directory_cache_is_empty() {
    let codex_home = tempdir().expect("tempdir should succeed");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"
[features]
apps = true

[tool_suggest]
discoverables = [
  { type = "connector", id = "connector_gmail" }
]
"#,
    )
    .expect("write config");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config should load");
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let plugins_manager = PluginsManager::new(config.codex_home.to_path_buf());

    let discoverable_tools = list_tool_suggest_discoverable_tools_with_auth(
        &config,
        &plugins_manager,
        Some(&auth),
        &[],
        &[],
    )
    .await
    .expect("discoverable tools should load");

    assert_eq!(
        discoverable_tools,
        vec![DiscoverableTool::from(plugin_connector_to_app_info(
            "connector_gmail".to_string(),
        ))]
    );
}

#[tokio::test]
async fn tool_suggest_includes_connectors_from_loaded_plugin_apps() {
    let codex_home = tempdir().expect("tempdir should succeed");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"
[features]
apps = true
"#,
    )
    .expect("write config");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config should load");
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let loaded_plugin_app_connector_ids = vec!["asdk_app_databricks_workspace".to_string()];
    let plugins_manager = PluginsManager::new(config.codex_home.to_path_buf());

    let discoverable_tools = list_tool_suggest_discoverable_tools_with_auth(
        &config,
        &plugins_manager,
        Some(&auth),
        &[],
        &loaded_plugin_app_connector_ids,
    )
    .await
    .expect("discoverable tools should load");

    assert_eq!(
        discoverable_tools,
        vec![DiscoverableTool::from(plugin_connector_to_app_info(
            "asdk_app_databricks_workspace".to_string(),
        ))]
    );
}
