use crate::config::test_config;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::test_support::construct_model_info_offline;
use crate::tools::ToolRouter;
use crate::tools::registry::tool_handler_key;
use crate::tools::router::ToolRouterParams;
use codex_app_server_protocol::AppInfo;
use codex_features::Feature;
use codex_features::Features;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_models_manager::bundled_models_response;
use codex_models_manager::model_info::with_config_overrides;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_tools::ConfiguredToolSpec;
use codex_tools::DiscoverableTool;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ShellCommandBackendConfig;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::TOOL_SUGGEST_TOOL_NAME;
use codex_tools::ToolSpec;
use codex_tools::ToolsConfig;
use codex_tools::ToolsConfigParams;
use codex_tools::UnifiedExecShellMode;
use codex_tools::ZshForkConfig;
use codex_tools::mcp_call_tool_result_output_schema;
use codex_tools::mcp_tool_to_deferred_responses_api_tool;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;

fn mcp_tool(name: &str, description: &str, input_schema: serde_json::Value) -> rmcp::model::Tool {
    rmcp::model::Tool {
        name: name.to_string().into(),
        title: None,
        description: Some(description.to_string().into()),
        input_schema: std::sync::Arc::new(rmcp::model::object(input_schema)),
        output_schema: None,
        annotations: None,
        execution: None,
        icons: None,
        meta: None,
    }
}

fn discoverable_connector(id: &str, name: &str, description: &str) -> DiscoverableTool {
    let slug = name.replace(' ', "-").to_lowercase();
    DiscoverableTool::Connector(Box::new(AppInfo {
        id: id.to_string(),
        name: name.to_string(),
        description: Some(description.to_string()),
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: Some(format!("https://chatgpt.com/apps/{slug}/{id}")),
        is_accessible: false,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }))
}

async fn search_capable_model_info() -> ModelInfo {
    let config = test_config().await;
    let mut model_info = construct_model_info_offline("gpt-5-codex", &config);
    model_info.supports_search_tool = true;
    model_info
}

#[test]
fn deferred_responses_api_tool_serializes_with_defer_loading() {
    let tool = mcp_tool(
        "lookup_order",
        "Look up an order",
        serde_json::json!({
            "type": "object",
            "properties": {
                "order_id": {"type": "string"}
            },
            "required": ["order_id"],
            "additionalProperties": false,
        }),
    );

    let serialized = serde_json::to_value(ToolSpec::Function(
        mcp_tool_to_deferred_responses_api_tool("mcp__codex_apps__lookup_order".to_string(), &tool)
            .expect("convert deferred tool"),
    ))
    .expect("serialize deferred tool");

    assert_eq!(
        serialized,
        serde_json::json!({
            "type": "function",
            "name": "mcp__codex_apps__lookup_order",
            "description": "Look up an order",
            "strict": false,
            "defer_loading": true,
            "parameters": {
                "type": "object",
                "properties": {
                    "order_id": {"type": "string"}
                },
                "required": ["order_id"],
                "additionalProperties": false,
            }
        })
    );
}

// Avoid order-based assertions; compare via set containment instead.
fn assert_contains_tool_names(tools: &[ConfiguredToolSpec], expected_subset: &[&str]) {
    use std::collections::HashSet;
    let mut names = HashSet::new();
    let mut duplicates = Vec::new();
    for name in tools.iter().map(ConfiguredToolSpec::name) {
        if !names.insert(name) {
            duplicates.push(name);
        }
    }
    assert!(
        duplicates.is_empty(),
        "duplicate tool entries detected: {duplicates:?}"
    );
    for expected in expected_subset {
        assert!(
            names.contains(expected),
            "expected tool {expected} to be present; had: {names:?}"
        );
    }
}

fn shell_tool_name(config: &ToolsConfig) -> Option<&'static str> {
    match config.shell_type {
        ConfigShellToolType::Default => Some("shell"),
        ConfigShellToolType::Local => Some("local_shell"),
        ConfigShellToolType::UnifiedExec => None,
        ConfigShellToolType::Disabled => None,
        ConfigShellToolType::ShellCommand => Some("shell_command"),
    }
}

fn find_tool<'a>(tools: &'a [ConfiguredToolSpec], expected_name: &str) -> &'a ConfiguredToolSpec {
    tools
        .iter()
        .find(|tool| tool.name() == expected_name)
        .unwrap_or_else(|| panic!("expected tool {expected_name}"))
}

async fn model_info_from_models_json(slug: &str) -> ModelInfo {
    let config = test_config().await;
    let response = bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    let model = response
        .models
        .into_iter()
        .find(|candidate| candidate.slug == slug)
        .unwrap_or_else(|| panic!("model slug {slug} is missing from models.json"));
    with_config_overrides(model, &config.to_models_manager_config())
}

/// Builds the tool registry builder while collecting tool specs for later serialization.
fn build_specs(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    app_tools: Option<HashMap<String, ToolInfo>>,
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    build_specs_with_discoverable_tools(
        config,
        mcp_tools,
        app_tools,
        /*discoverable_tools*/ None,
        dynamic_tools,
    )
}

#[tokio::test]
async fn model_provided_unified_exec_is_blocked_for_windows_sandboxed_policies() {
    let mut model_info = model_info_from_models_json("gpt-5-codex").await;
    model_info.shell_type = ConfigShellToolType::UnifiedExec;
    let features = Features::with_defaults();
    let available_models = Vec::new();
    let config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::new_workspace_write_policy(),
        windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
    });

    let expected_shell_type = if cfg!(target_os = "windows") {
        ConfigShellToolType::ShellCommand
    } else {
        ConfigShellToolType::UnifiedExec
    };
    assert_eq!(config.shell_type, expected_shell_type);
}

#[tokio::test]
async fn get_memory_requires_feature_flag() {
    let config = test_config().await;
    let model_info = construct_model_info_offline("gpt-5-codex", &config);
    let mut features = Features::with_defaults();
    features.disable(Feature::MemoryTool);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*app_tools*/ None,
        &[],
    )
    .build();
    assert!(
        !tools.iter().any(|t| t.spec.name() == "get_memory"),
        "get_memory should be disabled when memory_tool feature is off"
    );
}

async fn assert_model_tools(
    model_slug: &str,
    features: &Features,
    web_search_mode: Option<WebSearchMode>,
    expected_tools: &[&str],
) {
    let _config = test_config().await;
    let model_info = model_info_from_models_json(model_slug).await;
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features,
        web_search_mode,
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let router = ToolRouter::from_config(
        &tools_config,
        ToolRouterParams {
            mcp_tools: None,
            app_tools: None,
            discoverable_tools: None,
            dynamic_tools: &[],
        },
    );
    let model_visible_specs = router.model_visible_specs();
    let tool_names = model_visible_specs
        .iter()
        .map(ToolSpec::name)
        .collect::<Vec<_>>();
    assert_eq!(&tool_names, &expected_tools,);
}

async fn assert_default_model_tools(
    model_slug: &str,
    features: &Features,
    web_search_mode: Option<WebSearchMode>,
    shell_tool: &'static str,
    expected_tail: &[&str],
) {
    let mut expected = if features.enabled(Feature::UnifiedExec) {
        vec!["exec_command", "write_stdin"]
    } else {
        vec![shell_tool]
    };
    expected.extend(expected_tail);
    assert_model_tools(model_slug, features, web_search_mode, &expected).await;
}

#[tokio::test]
async fn test_build_specs_gpt5_codex_default() {
    let features = Features::with_defaults();
    assert_default_model_tools(
        "gpt-5-codex",
        &features,
        Some(WebSearchMode::Cached),
        "shell_command",
        &[
            "update_plan",
            "request_user_input",
            "apply_patch",
            "web_search",
            "view_image",
            "spawn_agent",
            "send_input",
            "resume_agent",
            "wait_agent",
            "close_agent",
        ],
    )
    .await;
}

#[tokio::test]
async fn test_build_specs_gpt51_codex_default() {
    let features = Features::with_defaults();
    assert_default_model_tools(
        "gpt-5.1-codex",
        &features,
        Some(WebSearchMode::Cached),
        "shell_command",
        &[
            "update_plan",
            "request_user_input",
            "apply_patch",
            "web_search",
            "view_image",
            "spawn_agent",
            "send_input",
            "resume_agent",
            "wait_agent",
            "close_agent",
        ],
    )
    .await;
}

#[tokio::test]
async fn test_build_specs_gpt5_codex_unified_exec_web_search() {
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    assert_model_tools(
        "gpt-5-codex",
        &features,
        Some(WebSearchMode::Live),
        &[
            "exec_command",
            "write_stdin",
            "update_plan",
            "request_user_input",
            "apply_patch",
            "web_search",
            "view_image",
            "spawn_agent",
            "send_input",
            "resume_agent",
            "wait_agent",
            "close_agent",
        ],
    )
    .await;
}

#[tokio::test]
async fn test_build_specs_gpt51_codex_unified_exec_web_search() {
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    assert_model_tools(
        "gpt-5.1-codex",
        &features,
        Some(WebSearchMode::Live),
        &[
            "exec_command",
            "write_stdin",
            "update_plan",
            "request_user_input",
            "apply_patch",
            "web_search",
            "view_image",
            "spawn_agent",
            "send_input",
            "resume_agent",
            "wait_agent",
            "close_agent",
        ],
    )
    .await;
}

#[tokio::test]
async fn test_gpt_5_1_codex_max_defaults() {
    let features = Features::with_defaults();
    assert_default_model_tools(
        "gpt-5.1-codex-max",
        &features,
        Some(WebSearchMode::Cached),
        "shell_command",
        &[
            "update_plan",
            "request_user_input",
            "apply_patch",
            "web_search",
            "view_image",
            "spawn_agent",
            "send_input",
            "resume_agent",
            "wait_agent",
            "close_agent",
        ],
    )
    .await;
}

#[tokio::test]
async fn test_codex_5_1_mini_defaults() {
    let features = Features::with_defaults();
    assert_default_model_tools(
        "gpt-5.1-codex-mini",
        &features,
        Some(WebSearchMode::Cached),
        "shell_command",
        &[
            "update_plan",
            "request_user_input",
            "apply_patch",
            "web_search",
            "view_image",
            "spawn_agent",
            "send_input",
            "resume_agent",
            "wait_agent",
            "close_agent",
        ],
    )
    .await;
}

#[tokio::test]
async fn test_gpt_5_defaults() {
    let features = Features::with_defaults();
    assert_default_model_tools(
        "gpt-5",
        &features,
        Some(WebSearchMode::Cached),
        "shell",
        &[
            "update_plan",
            "request_user_input",
            "web_search",
            "view_image",
            "spawn_agent",
            "send_input",
            "resume_agent",
            "wait_agent",
            "close_agent",
        ],
    )
    .await;
}

#[tokio::test]
async fn test_gpt_5_1_defaults() {
    let features = Features::with_defaults();
    assert_default_model_tools(
        "gpt-5.1",
        &features,
        Some(WebSearchMode::Cached),
        "shell_command",
        &[
            "update_plan",
            "request_user_input",
            "apply_patch",
            "web_search",
            "view_image",
            "spawn_agent",
            "send_input",
            "resume_agent",
            "wait_agent",
            "close_agent",
        ],
    )
    .await;
}

#[tokio::test]
async fn test_gpt_5_1_codex_max_unified_exec_web_search() {
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    assert_model_tools(
        "gpt-5.1-codex-max",
        &features,
        Some(WebSearchMode::Live),
        &[
            "exec_command",
            "write_stdin",
            "update_plan",
            "request_user_input",
            "apply_patch",
            "web_search",
            "view_image",
            "spawn_agent",
            "send_input",
            "resume_agent",
            "wait_agent",
            "close_agent",
        ],
    )
    .await;
}

#[tokio::test]
async fn test_build_specs_default_shell_present() {
    let config = test_config().await;
    let model_info = construct_model_info_offline("o3", &config);
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Live),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        Some(HashMap::new()),
        /*app_tools*/ None,
        &[],
    )
    .build();

    // Only check the shell variant and a couple of core tools.
    let mut subset = vec!["exec_command", "write_stdin", "update_plan"];
    if let Some(shell_tool) = shell_tool_name(&tools_config) {
        subset.push(shell_tool);
    }
    assert_contains_tool_names(&tools, &subset);
}

#[tokio::test]
async fn shell_zsh_fork_prefers_shell_command_over_unified_exec() {
    let config = test_config().await;
    let model_info = construct_model_info_offline("o3", &config);
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    features.enable(Feature::ShellZshFork);

    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Live),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let user_shell = Shell {
        shell_type: ShellType::Zsh,
        shell_path: PathBuf::from("/bin/zsh"),
        shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
    };

    assert_eq!(tools_config.shell_type, ConfigShellToolType::ShellCommand);
    assert_eq!(
        tools_config.shell_command_backend,
        ShellCommandBackendConfig::ZshFork
    );
    assert_eq!(
        tools_config.unified_exec_shell_mode,
        UnifiedExecShellMode::Direct
    );
    assert_eq!(
        tools_config
            .with_unified_exec_shell_mode_for_session(
                tool_user_shell_type(&user_shell),
                Some(&PathBuf::from(if cfg!(windows) {
                    r"C:\opt\codex\zsh"
                } else {
                    "/opt/codex/zsh"
                })),
                Some(&PathBuf::from(if cfg!(windows) {
                    r"C:\opt\codex\codex-execve-wrapper"
                } else {
                    "/opt/codex/codex-execve-wrapper"
                })),
            )
            .unified_exec_shell_mode,
        if cfg!(unix) {
            UnifiedExecShellMode::ZshFork(ZshForkConfig {
                shell_zsh_path: AbsolutePathBuf::from_absolute_path("/opt/codex/zsh").unwrap(),
                main_execve_wrapper_exe: AbsolutePathBuf::from_absolute_path(
                    "/opt/codex/codex-execve-wrapper",
                )
                .unwrap(),
            })
        } else {
            UnifiedExecShellMode::Direct
        }
    );
}

#[tokio::test]
async fn tool_suggest_requires_apps_and_plugins_features() {
    let model_info = search_capable_model_info().await;
    let discoverable_tools = Some(vec![discoverable_connector(
        "connector_2128aebfecb84f64a069897515042a44",
        "Google Calendar",
        "Plan events and schedules.",
    )]);
    let available_models = Vec::new();

    for disabled_feature in [Feature::Apps, Feature::Plugins] {
        let mut features = Features::with_defaults();
        features.enable(Feature::ToolSearch);
        features.enable(Feature::ToolSuggest);
        features.enable(Feature::Apps);
        features.enable(Feature::Plugins);
        features.disable(disabled_feature);

        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &available_models,
            features: &features,
            web_search_mode: Some(WebSearchMode::Cached),
            session_source: SessionSource::Cli,
            sandbox_policy: &SandboxPolicy::DangerFullAccess,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
        });
        let (tools, _) = build_specs_with_discoverable_tools(
            &tools_config,
            /*mcp_tools*/ None,
            /*app_tools*/ None,
            discoverable_tools.clone(),
            &[],
        )
        .build();

        assert!(
            !tools
                .iter()
                .any(|tool| tool.name() == TOOL_SUGGEST_TOOL_NAME),
            "tool_suggest should be absent when {disabled_feature:?} is disabled"
        );
    }
}

#[tokio::test]
async fn search_tool_description_handles_no_enabled_apps() {
    let model_info = search_capable_model_info().await;
    let mut features = Features::with_defaults();
    features.enable(Feature::Apps);
    features.enable(Feature::ToolSearch);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        Some(HashMap::new()),
        &[],
    )
    .build();
    let search_tool = find_tool(&tools, TOOL_SEARCH_TOOL_NAME);
    let ToolSpec::ToolSearch { description, .. } = &search_tool.spec else {
        panic!("expected tool_search tool");
    };

    assert!(description.contains("None currently enabled."));
    assert!(!description.contains("{{app_descriptions}}"));
}

#[tokio::test]
async fn search_tool_description_falls_back_to_connector_name_without_description() {
    let model_info = search_capable_model_info().await;
    let mut features = Features::with_defaults();
    features.enable(Feature::Apps);
    features.enable(Feature::ToolSearch);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        Some(HashMap::from([(
            "mcp__codex_apps__calendar_create_event".to_string(),
            ToolInfo {
                server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                tool_name: "_create_event".to_string(),
                tool_namespace: "mcp__codex_apps__calendar".to_string(),
                tool: mcp_tool(
                    "calendar_create_event",
                    "Create calendar event",
                    serde_json::json!({"type": "object"}),
                ),
                connector_id: Some("calendar".to_string()),
                connector_name: Some("Calendar".to_string()),
                plugin_display_names: Vec::new(),
                connector_description: None,
            },
        )])),
        &[],
    )
    .build();
    let search_tool = find_tool(&tools, TOOL_SEARCH_TOOL_NAME);
    let ToolSpec::ToolSearch { description, .. } = &search_tool.spec else {
        panic!("expected tool_search tool");
    };

    assert!(description.contains("- Calendar"));
    assert!(!description.contains("- Calendar:"));
}

#[tokio::test]
async fn search_tool_registers_namespaced_app_tool_aliases() {
    let model_info = search_capable_model_info().await;
    let mut features = Features::with_defaults();
    features.enable(Feature::Apps);
    features.enable(Feature::ToolSearch);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (_, registry) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        Some(HashMap::from([
            (
                "mcp__codex_apps__calendar_create_event".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "_create_event".to_string(),
                    tool_namespace: "mcp__codex_apps__calendar".to_string(),
                    tool: mcp_tool(
                        "calendar-create-event",
                        "Create calendar event",
                        serde_json::json!({"type": "object"}),
                    ),
                    connector_id: Some("calendar".to_string()),
                    connector_name: Some("Calendar".to_string()),
                    connector_description: None,
                    plugin_display_names: Vec::new(),
                },
            ),
            (
                "mcp__codex_apps__calendar_list_events".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "_list_events".to_string(),
                    tool_namespace: "mcp__codex_apps__calendar".to_string(),
                    tool: mcp_tool(
                        "calendar-list-events",
                        "List calendar events",
                        serde_json::json!({"type": "object"}),
                    ),
                    connector_id: Some("calendar".to_string()),
                    connector_name: Some("Calendar".to_string()),
                    connector_description: None,
                    plugin_display_names: Vec::new(),
                },
            ),
        ])),
        &[],
    )
    .build();

    let alias = tool_handler_key("_create_event", Some("mcp__codex_apps__calendar"));

    assert!(registry.has_handler(TOOL_SEARCH_TOOL_NAME, /*namespace*/ None));
    assert!(registry.has_handler(alias.as_str(), /*namespace*/ None));
}

#[tokio::test]
async fn test_mcp_tool_property_missing_type_defaults_to_string() {
    let config = test_config().await;
    let model_info = construct_model_info_offline("gpt-5-codex", &config);
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        Some(HashMap::from([(
            "dash/search".to_string(),
            mcp_tool(
                "search",
                "Search docs",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {"description": "search query"}
                    }
                }),
            ),
        )])),
        /*app_tools*/ None,
        &[],
    )
    .build();

    let tool = find_tool(&tools, "dash/search");
    assert_eq!(
        tool.spec,
        ToolSpec::Function(ResponsesApiTool {
            name: "dash/search".to_string(),
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([(
                    "query".to_string(),
                    JsonSchema::String {
                        description: Some("search query".to_string())
                    }
                )]),
                required: None,
                additional_properties: None,
            },
            description: "Search docs".to_string(),
            strict: false,
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            defer_loading: None,
        })
    );
}

#[tokio::test]
async fn test_mcp_tool_integer_normalized_to_number() {
    let config = test_config().await;
    let model_info = construct_model_info_offline("gpt-5-codex", &config);
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        Some(HashMap::from([(
            "dash/paginate".to_string(),
            mcp_tool(
                "paginate",
                "Pagination",
                serde_json::json!({
                    "type": "object",
                    "properties": {"page": {"type": "integer"}}
                }),
            ),
        )])),
        /*app_tools*/ None,
        &[],
    )
    .build();

    let tool = find_tool(&tools, "dash/paginate");
    assert_eq!(
        tool.spec,
        ToolSpec::Function(ResponsesApiTool {
            name: "dash/paginate".to_string(),
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([(
                    "page".to_string(),
                    JsonSchema::Number { description: None }
                )]),
                required: None,
                additional_properties: None,
            },
            description: "Pagination".to_string(),
            strict: false,
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            defer_loading: None,
        })
    );
}

#[tokio::test]
async fn test_mcp_tool_array_without_items_gets_default_string_items() {
    let config = test_config().await;
    let model_info = construct_model_info_offline("gpt-5-codex", &config);
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    features.enable(Feature::ApplyPatchFreeform);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        Some(HashMap::from([(
            "dash/tags".to_string(),
            mcp_tool(
                "tags",
                "Tags",
                serde_json::json!({
                    "type": "object",
                    "properties": {"tags": {"type": "array"}}
                }),
            ),
        )])),
        /*app_tools*/ None,
        &[],
    )
    .build();

    let tool = find_tool(&tools, "dash/tags");
    assert_eq!(
        tool.spec,
        ToolSpec::Function(ResponsesApiTool {
            name: "dash/tags".to_string(),
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([(
                    "tags".to_string(),
                    JsonSchema::Array {
                        items: Box::new(JsonSchema::String { description: None }),
                        description: None
                    }
                )]),
                required: None,
                additional_properties: None,
            },
            description: "Tags".to_string(),
            strict: false,
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            defer_loading: None,
        })
    );
}

#[tokio::test]
async fn test_mcp_tool_anyof_defaults_to_string() {
    let config = test_config().await;
    let model_info = construct_model_info_offline("gpt-5-codex", &config);
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        Some(HashMap::from([(
            "dash/value".to_string(),
            mcp_tool(
                "value",
                "AnyOf Value",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "value": {"anyOf": [{"type": "string"}, {"type": "number"}]}
                    }
                }),
            ),
        )])),
        /*app_tools*/ None,
        &[],
    )
    .build();

    let tool = find_tool(&tools, "dash/value");
    assert_eq!(
        tool.spec,
        ToolSpec::Function(ResponsesApiTool {
            name: "dash/value".to_string(),
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([(
                    "value".to_string(),
                    JsonSchema::String { description: None }
                )]),
                required: None,
                additional_properties: None,
            },
            description: "AnyOf Value".to_string(),
            strict: false,
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            defer_loading: None,
        })
    );
}

#[tokio::test]
async fn test_get_openai_tools_mcp_tools_with_additional_properties_schema() {
    let config = test_config().await;
    let model_info = construct_model_info_offline("gpt-5-codex", &config);
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        Some(HashMap::from([(
            "test_server/do_something_cool".to_string(),
            mcp_tool(
                "do_something_cool",
                "Do something cool",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "string_argument": {"type": "string"},
                        "number_argument": {"type": "number"},
                        "object_argument": {
                            "type": "object",
                            "properties": {
                                "string_property": {"type": "string"},
                                "number_property": {"type": "number"}
                            },
                            "required": ["string_property", "number_property"],
                            "additionalProperties": {
                                "type": "object",
                                "properties": {
                                    "addtl_prop": {"type": "string"}
                                },
                                "required": ["addtl_prop"],
                                "additionalProperties": false
                            }
                        }
                    }
                }),
            ),
        )])),
        /*app_tools*/ None,
        &[],
    )
    .build();

    let tool = find_tool(&tools, "test_server/do_something_cool");
    assert_eq!(
        tool.spec,
        ToolSpec::Function(ResponsesApiTool {
            name: "test_server/do_something_cool".to_string(),
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([
                    (
                        "string_argument".to_string(),
                        JsonSchema::String { description: None }
                    ),
                    (
                        "number_argument".to_string(),
                        JsonSchema::Number { description: None }
                    ),
                    (
                        "object_argument".to_string(),
                        JsonSchema::Object {
                            properties: BTreeMap::from([
                                (
                                    "string_property".to_string(),
                                    JsonSchema::String { description: None }
                                ),
                                (
                                    "number_property".to_string(),
                                    JsonSchema::Number { description: None }
                                ),
                            ]),
                            required: Some(vec![
                                "string_property".to_string(),
                                "number_property".to_string(),
                            ]),
                            additional_properties: Some(
                                JsonSchema::Object {
                                    properties: BTreeMap::from([(
                                        "addtl_prop".to_string(),
                                        JsonSchema::String { description: None }
                                    ),]),
                                    required: Some(vec!["addtl_prop".to_string(),]),
                                    additional_properties: Some(false.into()),
                                }
                                .into()
                            ),
                        },
                    ),
                ]),
                required: None,
                additional_properties: None,
            },
            description: "Do something cool".to_string(),
            strict: false,
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            defer_loading: None,
        })
    );
}

#[tokio::test]
async fn code_mode_only_restricts_model_tools_to_exec_tools() {
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeMode);
    features.enable(Feature::CodeModeOnly);

    assert_model_tools(
        "gpt-5.1-codex",
        &features,
        Some(WebSearchMode::Live),
        &["exec", "wait"],
    )
    .await;
}
