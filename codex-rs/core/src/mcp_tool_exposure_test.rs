use std::collections::HashMap;
use std::sync::Arc;

use codex_connectors::metadata::sanitize_name;
use codex_features::Feature;
use codex_features::Features;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo;
use codex_models_manager::test_support::construct_model_info_offline_for_tests;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_tools::ToolsConfig;
use codex_tools::ToolsConfigParams;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Tool;

use super::*;
use crate::config::test_config;
use crate::connectors::AppInfo;

fn make_connector(id: &str, name: &str) -> AppInfo {
    AppInfo {
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
        is_accessible: true,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }
}

fn make_mcp_tool(
    server_name: &str,
    tool_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
) -> ToolInfo {
    let tool_namespace = if server_name == CODEX_APPS_MCP_SERVER_NAME {
        connector_name
            .map(sanitize_name)
            .map(|connector_name| format!("mcp__{server_name}__{connector_name}"))
            .unwrap_or_else(|| server_name.to_string())
    } else {
        format!("mcp__{server_name}__")
    };

    ToolInfo {
        server_name: server_name.to_string(),
        callable_name: tool_name.to_string(),
        callable_namespace: tool_namespace,
        server_instructions: None,
        tool: Tool {
            name: tool_name.to_string().into(),
            title: None,
            description: Some(format!("Test tool: {tool_name}").into()),
            input_schema: Arc::new(JsonObject::default()),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        connector_id: connector_id.map(str::to_string),
        connector_name: connector_name.map(str::to_string),
        plugin_display_names: Vec::new(),
        connector_description: None,
    }
}

fn numbered_mcp_tools(count: usize) -> HashMap<String, ToolInfo> {
    (0..count)
        .map(|index| {
            let tool_name = format!("tool_{index}");
            (
                format!("mcp__rmcp__{tool_name}"),
                make_mcp_tool(
                    "rmcp", &tool_name, /*connector_id*/ None, /*connector_name*/ None,
                ),
            )
        })
        .collect()
}

async fn tools_config_for_mcp_tool_exposure(search_tool: bool) -> ToolsConfig {
    let config = test_config().await;
    let model_info =
        construct_model_info_offline_for_tests("gpt-5.4", &config.to_models_manager_config());
    let features = Features::with_defaults();
    let available_models = Vec::new();
    let mut tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    tools_config.search_tool = search_tool;
    tools_config
}

#[tokio::test]
async fn directly_exposes_small_effective_tool_sets() {
    let config = test_config().await;
    let tools_config = tools_config_for_mcp_tool_exposure(/*search_tool*/ true).await;
    let mcp_tools = numbered_mcp_tools(DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD - 1);

    let exposure = build_mcp_tool_exposure(
        &mcp_tools,
        /*connectors*/ None,
        &[],
        &config,
        &tools_config,
    );

    let mut direct_tool_names: Vec<_> = exposure.direct_tools.keys().cloned().collect();
    direct_tool_names.sort();
    let mut expected_tool_names: Vec<_> = mcp_tools.keys().cloned().collect();
    expected_tool_names.sort();
    assert_eq!(direct_tool_names, expected_tool_names);
    assert!(exposure.deferred_tools.is_none());
}

#[tokio::test]
async fn searches_large_effective_tool_sets() {
    let config = test_config().await;
    let tools_config = tools_config_for_mcp_tool_exposure(/*search_tool*/ true).await;
    let mcp_tools = numbered_mcp_tools(DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD);

    let exposure = build_mcp_tool_exposure(
        &mcp_tools,
        /*connectors*/ None,
        &[],
        &config,
        &tools_config,
    );

    assert!(exposure.direct_tools.is_empty());
    let deferred_tools = exposure
        .deferred_tools
        .as_ref()
        .expect("large tool sets should be discoverable through tool_search");
    let mut deferred_tool_names: Vec<_> = deferred_tools.keys().cloned().collect();
    deferred_tool_names.sort();
    let mut expected_tool_names: Vec<_> = mcp_tools.keys().cloned().collect();
    expected_tool_names.sort();
    assert_eq!(deferred_tool_names, expected_tool_names);
}

#[tokio::test]
async fn directly_exposes_explicit_apps_without_deferred_overlap() {
    let config = test_config().await;
    let tools_config = tools_config_for_mcp_tool_exposure(/*search_tool*/ true).await;
    let mut mcp_tools = numbered_mcp_tools(DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD - 1);
    mcp_tools.extend([(
        "mcp__codex_apps__calendar_create_event".to_string(),
        make_mcp_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "calendar_create_event",
            Some("calendar"),
            Some("Calendar"),
        ),
    )]);
    let connectors = vec![make_connector("calendar", "Calendar")];

    let exposure = build_mcp_tool_exposure(
        &mcp_tools,
        Some(connectors.as_slice()),
        connectors.as_slice(),
        &config,
        &tools_config,
    );

    let mut tool_names: Vec<String> = exposure.direct_tools.into_keys().collect();
    tool_names.sort();
    assert_eq!(
        tool_names,
        vec!["mcp__codex_apps__calendar_create_event".to_string()]
    );
    assert_eq!(
        exposure.deferred_tools.as_ref().map(HashMap::len),
        Some(DIRECT_MCP_TOOL_EXPOSURE_THRESHOLD - 1)
    );
    let deferred_tools = exposure
        .deferred_tools
        .as_ref()
        .expect("large tool sets should be discoverable through tool_search");
    assert!(
        tool_names
            .iter()
            .all(|direct_tool_name| !deferred_tools.contains_key(direct_tool_name)),
        "direct tools should not also be deferred: {tool_names:?}"
    );
    assert!(!deferred_tools.contains_key("mcp__codex_apps__calendar_create_event"));
    assert!(deferred_tools.contains_key("mcp__rmcp__tool_0"));
}

#[tokio::test]
async fn always_defer_feature_preserves_explicit_apps() {
    let mut config = test_config().await;
    config
        .features
        .enable(Feature::ToolSearchAlwaysDeferMcpTools)
        .expect("test config should allow feature update");
    let tools_config = tools_config_for_mcp_tool_exposure(/*search_tool*/ true).await;
    let mcp_tools = HashMap::from([
        (
            "mcp__rmcp__tool".to_string(),
            make_mcp_tool(
                "rmcp", "tool", /*connector_id*/ None, /*connector_name*/ None,
            ),
        ),
        (
            "mcp__codex_apps__calendar_create_event".to_string(),
            make_mcp_tool(
                CODEX_APPS_MCP_SERVER_NAME,
                "calendar_create_event",
                Some("calendar"),
                Some("Calendar"),
            ),
        ),
    ]);
    let connectors = vec![make_connector("calendar", "Calendar")];

    let exposure = build_mcp_tool_exposure(
        &mcp_tools,
        Some(connectors.as_slice()),
        connectors.as_slice(),
        &config,
        &tools_config,
    );

    let mut direct_tool_names: Vec<String> = exposure.direct_tools.into_keys().collect();
    direct_tool_names.sort();
    assert_eq!(
        direct_tool_names,
        vec!["mcp__codex_apps__calendar_create_event".to_string()]
    );
    let deferred_tools = exposure
        .deferred_tools
        .as_ref()
        .expect("MCP tools should be discoverable through tool_search");
    assert!(deferred_tools.contains_key("mcp__rmcp__tool"));
    assert!(!deferred_tools.contains_key("mcp__codex_apps__calendar_create_event"));
}
