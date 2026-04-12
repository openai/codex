use super::*;
use crate::AdditionalProperties;
use crate::ConfiguredToolSpec;
use crate::DiscoverablePluginInfo;
use crate::DiscoverableTool;
use crate::FreeformTool;
use crate::JsonSchema;
use crate::JsonSchemaPrimitiveType;
use crate::JsonSchemaType;
use crate::ResponsesApiTool;
use crate::ResponsesApiWebSearchFilters;
use crate::ResponsesApiWebSearchUserLocation;
use crate::ToolHandlerSpec;
use crate::ToolName;
use crate::ToolNamespace;
use crate::ToolRegistryPlanDeferredTool;
use crate::ToolRegistryPlanMcpTool;
use crate::ToolsConfigParams;
use crate::WaitAgentTimeoutOptions;
use crate::mcp_call_tool_result_output_schema;
use codex_app_server_protocol::AppInfo;
use codex_features::Feature;
use codex_features::Features;
use codex_protocol::config_types::WebSearchConfig;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::VIEW_IMAGE_TOOL_NAME;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;

const CODEX_APPS_MCP_SERVER_NAME: &str = "codex_apps";
const DEFAULT_AGENT_TYPE_DESCRIPTION: &str = "Test agent type description.";
const DEFAULT_WAIT_TIMEOUT_MS: i64 = 30_000;
const MIN_WAIT_TIMEOUT_MS: i64 = 10_000;
const MAX_WAIT_TIMEOUT_MS: i64 = 3_600_000;

#[test]
fn test_full_toolset_specs_for_gpt5_codex_unified_exec_web_search() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Live),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    let mut actual = BTreeMap::new();
    let mut duplicate_names = Vec::new();
    for tool in &tools {
        let name = tool.name().to_string();
        if actual.insert(name.clone(), tool.spec.clone()).is_some() {
            duplicate_names.push(name);
        }
    }
    assert!(
        duplicate_names.is_empty(),
        "duplicate tool entries detected: {duplicate_names:?}"
    );

    let mut expected = BTreeMap::new();
    for spec in [
        create_exec_command_tool(CommandToolOptions {
            allow_login_shell: true,
            exec_permission_approvals_enabled: false,
        }),
        create_write_stdin_tool(),
        create_update_plan_tool(),
        request_user_input_tool_spec(/*default_mode_request_user_input*/ false),
        create_apply_patch_freeform_tool(),
        ToolSpec::WebSearch {
            external_web_access: Some(true),
            filters: None,
            user_location: None,
            search_context_size: None,
            search_content_types: None,
        },
        create_view_image_tool(ViewImageToolOptions {
            can_request_original_image_detail: config.can_request_original_image_detail,
        }),
    ] {
        expected.insert(spec.name().to_string(), spec);
    }
    let collab_specs = if config.multi_agent_v2 {
        vec![
            create_spawn_agent_tool_v2(spawn_agent_tool_options(&config)),
            create_send_message_tool(),
            create_wait_agent_tool_v2(wait_agent_timeout_options()),
            create_close_agent_tool_v2(),
        ]
    } else {
        vec![
            create_spawn_agent_tool_v1(spawn_agent_tool_options(&config)),
            create_send_input_tool_v1(),
            create_wait_agent_tool_v1(wait_agent_timeout_options()),
            create_close_agent_tool_v1(),
        ]
    };
    for spec in collab_specs {
        expected.insert(spec.name().to_string(), spec);
    }
    if !config.multi_agent_v2 {
        let spec = create_resume_agent_tool();
        expected.insert(spec.name().to_string(), spec);
    }

    if config.exec_permission_approvals_enabled {
        let spec = create_request_permissions_tool(request_permissions_tool_description());
        expected.insert(spec.name().to_string(), spec);
    }

    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "tool name set mismatch"
    );

    for name in expected.keys() {
        let mut actual_spec = actual.get(name).expect("present").clone();
        let mut expected_spec = expected.get(name).expect("present").clone();
        strip_descriptions_tool(&mut actual_spec);
        strip_descriptions_tool(&mut expected_spec);
        assert_eq!(actual_spec, expected_spec, "spec mismatch for {name}");
    }
}

#[test]
fn test_build_specs_collab_tools_enabled() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::Collab);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert_contains_tool_names(
        &tools,
        &["spawn_agent", "send_input", "wait_agent", "close_agent"],
    );
    assert_lacks_tool_name(&tools, "spawn_agents_on_csv");
    assert_lacks_tool_name(&tools, "list_agents");

    let spawn_agent = find_tool(&tools, "spawn_agent");
    let ToolSpec::Function(ResponsesApiTool { parameters, .. }) = &spawn_agent.spec else {
        panic!("spawn_agent should be a function tool");
    };
    let (properties, _) = expect_object_schema(parameters);
    assert!(properties.contains_key("fork_context"));
    assert!(!properties.contains_key("fork_turns"));
}

#[test]
fn test_build_specs_multi_agent_v2_uses_task_names_and_hides_resume() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::Collab);
    features.enable(Feature::MultiAgentV2);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert_contains_tool_names(
        &tools,
        &[
            "spawn_agent",
            "send_message",
            "followup_task",
            "wait_agent",
            "close_agent",
            "list_agents",
        ],
    );

    let spawn_agent = find_tool(&tools, "spawn_agent");
    let ToolSpec::Function(ResponsesApiTool {
        parameters,
        output_schema,
        ..
    }) = &spawn_agent.spec
    else {
        panic!("spawn_agent should be a function tool");
    };
    let (properties, required) = expect_object_schema(parameters);
    assert!(properties.contains_key("task_name"));
    assert!(properties.contains_key("message"));
    assert!(properties.contains_key("fork_turns"));
    assert!(!properties.contains_key("items"));
    assert!(!properties.contains_key("fork_context"));
    assert_eq!(
        required,
        Some(&vec!["task_name".to_string(), "message".to_string()])
    );
    let output_schema = output_schema
        .as_ref()
        .expect("spawn_agent should define output schema");
    assert_eq!(output_schema["required"], json!(["task_name", "nickname"]));

    let send_message = find_tool(&tools, "send_message");
    let ToolSpec::Function(ResponsesApiTool {
        parameters,
        output_schema,
        ..
    }) = &send_message.spec
    else {
        panic!("send_message should be a function tool");
    };
    assert_eq!(output_schema, &None);
    let (properties, required) = expect_object_schema(parameters);
    assert!(properties.contains_key("target"));
    assert!(!properties.contains_key("interrupt"));
    assert!(properties.contains_key("message"));
    assert!(!properties.contains_key("items"));
    assert_eq!(
        required,
        Some(&vec!["target".to_string(), "message".to_string()])
    );

    let followup_task = find_tool(&tools, "followup_task");
    let ToolSpec::Function(ResponsesApiTool {
        parameters,
        output_schema,
        ..
    }) = &followup_task.spec
    else {
        panic!("followup_task should be a function tool");
    };
    assert_eq!(output_schema, &None);
    let (properties, required) = expect_object_schema(parameters);
    assert!(properties.contains_key("target"));
    assert!(properties.contains_key("message"));
    assert!(!properties.contains_key("items"));
    assert_eq!(
        required,
        Some(&vec!["target".to_string(), "message".to_string()])
    );

    let wait_agent = find_tool(&tools, "wait_agent");
    let ToolSpec::Function(ResponsesApiTool {
        parameters,
        output_schema,
        ..
    }) = &wait_agent.spec
    else {
        panic!("wait_agent should be a function tool");
    };
    let (properties, required) = expect_object_schema(parameters);
    assert!(!properties.contains_key("targets"));
    assert!(properties.contains_key("timeout_ms"));
    assert_eq!(required, None);
    let output_schema = output_schema
        .as_ref()
        .expect("wait_agent should define output schema");
    assert_eq!(
        output_schema["properties"]["message"]["description"],
        json!("Brief wait summary without the agent's final content.")
    );

    let list_agents = find_tool(&tools, "list_agents");
    let ToolSpec::Function(ResponsesApiTool {
        parameters,
        output_schema,
        ..
    }) = &list_agents.spec
    else {
        panic!("list_agents should be a function tool");
    };
    let (properties, required) = expect_object_schema(parameters);
    assert!(properties.contains_key("path_prefix"));
    assert_eq!(required, None);
    let output_schema = output_schema
        .as_ref()
        .expect("list_agents should define output schema");
    assert_eq!(
        output_schema["properties"]["agents"]["items"]["required"],
        json!(["agent_name", "agent_status", "last_task_message"])
    );
    assert_lacks_tool_name(&tools, "send_input");
    assert_lacks_tool_name(&tools, "resume_agent");
}

#[test]
fn test_build_specs_enable_fanout_enables_agent_jobs_and_collab_tools() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::SpawnCsv);
    features.normalize_dependencies();
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert_contains_tool_names(
        &tools,
        &[
            "spawn_agent",
            "send_input",
            "wait_agent",
            "close_agent",
            "spawn_agents_on_csv",
        ],
    );
}

#[test]
fn view_image_tool_omits_detail_without_original_detail_feature() {
    let mut model_info = model_info();
    model_info.supports_image_detail_original = true;
    let features = Features::with_defaults();
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    let view_image = find_tool(&tools, VIEW_IMAGE_TOOL_NAME);
    let ToolSpec::Function(ResponsesApiTool { parameters, .. }) = &view_image.spec else {
        panic!("view_image should be a function tool");
    };
    let (properties, _) = expect_object_schema(parameters);
    assert!(!properties.contains_key("detail"));
}

#[test]
fn view_image_tool_includes_detail_with_original_detail_feature() {
    let mut model_info = model_info();
    model_info.supports_image_detail_original = true;
    let mut features = Features::with_defaults();
    features.enable(Feature::ImageDetailOriginal);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    let view_image = find_tool(&tools, VIEW_IMAGE_TOOL_NAME);
    let ToolSpec::Function(ResponsesApiTool { parameters, .. }) = &view_image.spec else {
        panic!("view_image should be a function tool");
    };
    let (properties, _) = expect_object_schema(parameters);
    assert!(properties.contains_key("detail"));
    let description = expect_string_description(
        properties
            .get("detail")
            .expect("view_image detail should include a description"),
    );
    assert!(description.contains("only supported value is `original`"));
    assert!(description.contains("omit this field for default resized behavior"));
}

#[test]
fn disabled_environment_omits_environment_backed_tools() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    features.enable(Feature::JsRepl);
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
    })
    .with_has_environment(/*has_environment*/ false);
    tools_config
        .experimental_supported_tools
        .push("list_dir".to_string());
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert_lacks_tool_name(&tools, "exec_command");
    assert_lacks_tool_name(&tools, "write_stdin");
    assert_lacks_tool_name(&tools, "js_repl");
    assert_lacks_tool_name(&tools, "js_repl_reset");
    assert_lacks_tool_name(&tools, "apply_patch");
    assert_lacks_tool_name(&tools, "list_dir");
    assert_lacks_tool_name(&tools, VIEW_IMAGE_TOOL_NAME);
}

#[test]
fn test_build_specs_agent_job_worker_tools_enabled() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::SpawnCsv);
    features.normalize_dependencies();
    features.enable(Feature::Sqlite);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::SubAgent(SubAgentSource::Other(
            "agent_job:test".to_string(),
        )),
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert_contains_tool_names(
        &tools,
        &[
            "spawn_agent",
            "send_input",
            "resume_agent",
            "wait_agent",
            "close_agent",
            "spawn_agents_on_csv",
            "report_agent_job_result",
            REQUEST_USER_INPUT_TOOL_NAME,
        ],
    );
}

#[test]
fn request_user_input_description_reflects_default_mode_feature_flag() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    let request_user_input_tool = find_tool(&tools, REQUEST_USER_INPUT_TOOL_NAME);
    assert_eq!(
        request_user_input_tool.spec,
        request_user_input_tool_spec(/*default_mode_request_user_input*/ false)
    );

    features.enable(Feature::DefaultModeRequestUserInput);
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    let request_user_input_tool = find_tool(&tools, REQUEST_USER_INPUT_TOOL_NAME);
    assert_eq!(
        request_user_input_tool.spec,
        request_user_input_tool_spec(/*default_mode_request_user_input*/ true)
    );
}

#[test]
fn request_permissions_requires_feature_flag() {
    let model_info = model_info();
    let features = Features::with_defaults();
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    assert_lacks_tool_name(&tools, "request_permissions");

    let mut features = Features::with_defaults();
    features.enable(Feature::RequestPermissionsTool);
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    let request_permissions_tool = find_tool(&tools, "request_permissions");
    assert_eq!(
        request_permissions_tool.spec,
        create_request_permissions_tool(request_permissions_tool_description())
    );
}

#[test]
fn request_permissions_tool_is_independent_from_additional_permissions() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::ExecPermissionApprovals);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert_lacks_tool_name(&tools, "request_permissions");
}

#[test]
fn js_repl_requires_feature_flag() {
    let model_info = model_info();
    let features = Features::with_defaults();

    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert!(
        !tools.iter().any(|tool| tool.spec.name() == "js_repl"),
        "js_repl should be disabled when the feature is off"
    );
    assert!(
        !tools.iter().any(|tool| tool.spec.name() == "js_repl_reset"),
        "js_repl_reset should be disabled when the feature is off"
    );
}

#[test]
fn js_repl_enabled_adds_tools() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::JsRepl);

    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert_contains_tool_names(&tools, &["js_repl", "js_repl_reset"]);
}

#[test]
fn image_generation_tools_require_feature_and_supported_model() {
    let supported_model_info = model_info();
    let mut unsupported_model_info = supported_model_info.clone();
    unsupported_model_info.input_modalities = vec![InputModality::Text];
    let mut image_generation_disabled_features = Features::with_defaults();
    image_generation_disabled_features.disable(Feature::ImageGeneration);
    let mut image_generation_features = Features::with_defaults();
    image_generation_features.enable(Feature::ImageGeneration);

    let available_models = Vec::new();
    let default_tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &supported_model_info,
        available_models: &available_models,
        features: &image_generation_disabled_features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (default_tools, _) = build_specs(
        &default_tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    assert!(
        !default_tools
            .iter()
            .any(|tool| tool.spec.name() == "image_generation"),
        "image_generation should be disabled when the feature is disabled"
    );

    let supported_tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &supported_model_info,
        available_models: &available_models,
        features: &image_generation_features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (supported_tools, _) = build_specs(
        &supported_tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    assert_contains_tool_names(&supported_tools, &["image_generation"]);
    let image_generation_tool = find_tool(&supported_tools, "image_generation");
    assert_eq!(
        serde_json::to_value(&image_generation_tool.spec).expect("serialize image tool"),
        serde_json::json!({
            "type": "image_generation",
            "output_format": "png"
        })
    );

    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &unsupported_model_info,
        available_models: &available_models,
        features: &image_generation_features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    assert!(
        !tools
            .iter()
            .any(|tool| tool.spec.name() == "image_generation"),
        "image_generation should be disabled for unsupported models"
    );
}

#[test]
fn web_search_mode_cached_sets_external_web_access_false() {
    let model_info = model_info();
    let features = Features::with_defaults();

    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    let tool = find_tool(&tools, "web_search");
    assert_eq!(
        tool.spec,
        ToolSpec::WebSearch {
            external_web_access: Some(false),
            filters: None,
            user_location: None,
            search_context_size: None,
            search_content_types: None,
        }
    );
}

#[test]
fn web_search_mode_live_sets_external_web_access_true() {
    let model_info = model_info();
    let features = Features::with_defaults();

    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Live),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    let tool = find_tool(&tools, "web_search");
    assert_eq!(
        tool.spec,
        ToolSpec::WebSearch {
            external_web_access: Some(true),
            filters: None,
            user_location: None,
            search_context_size: None,
            search_content_types: None,
        }
    );
}

#[test]
fn web_search_config_is_forwarded_to_tool_spec() {
    let model_info = model_info();
    let features = Features::with_defaults();
    let web_search_config = WebSearchConfig {
        filters: Some(codex_protocol::config_types::WebSearchFilters {
            allowed_domains: Some(vec!["example.com".to_string()]),
        }),
        user_location: Some(codex_protocol::config_types::WebSearchUserLocation {
            r#type: codex_protocol::config_types::WebSearchUserLocationType::Approximate,
            country: Some("US".to_string()),
            region: Some("California".to_string()),
            city: Some("San Francisco".to_string()),
            timezone: Some("America/Los_Angeles".to_string()),
        }),
        search_context_size: Some(codex_protocol::config_types::WebSearchContextSize::High),
    };

    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Live),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    })
    .with_web_search_config(Some(web_search_config.clone()));
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    let tool = find_tool(&tools, "web_search");
    assert_eq!(
        tool.spec,
        ToolSpec::WebSearch {
            external_web_access: Some(true),
            filters: web_search_config
                .filters
                .map(ResponsesApiWebSearchFilters::from),
            user_location: web_search_config
                .user_location
                .map(ResponsesApiWebSearchUserLocation::from),
            search_context_size: web_search_config.search_context_size,
            search_content_types: None,
        }
    );
}

#[test]
fn web_search_tool_type_text_and_image_sets_search_content_types() {
    let mut model_info = model_info();
    model_info.web_search_tool_type = WebSearchToolType::TextAndImage;
    let features = Features::with_defaults();

    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Live),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    let tool = find_tool(&tools, "web_search");
    assert_eq!(
        tool.spec,
        ToolSpec::WebSearch {
            external_web_access: Some(true),
            filters: None,
            user_location: None,
            search_context_size: None,
            search_content_types: Some(vec!["text".to_string(), "image".to_string()]),
        }
    );
}

#[test]
fn mcp_resource_tools_are_hidden_without_mcp_servers() {
    let model_info = model_info();
    let features = Features::with_defaults();
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert!(
        !tools.iter().any(|tool| matches!(
            tool.spec.name(),
            "list_mcp_resources" | "list_mcp_resource_templates" | "read_mcp_resource"
        )),
        "MCP resource tools should be omitted when no MCP servers are configured"
    );
}

#[test]
fn mcp_resource_tools_are_included_when_mcp_servers_are_present() {
    let model_info = model_info();
    let features = Features::with_defaults();
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        Some(HashMap::new()),
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert_contains_tool_names(
        &tools,
        &[
            "list_mcp_resources",
            "list_mcp_resource_templates",
            "read_mcp_resource",
        ],
    );
}

#[test]
#[ignore]
fn test_parallel_support_flags() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert!(find_tool(&tools, "exec_command").supports_parallel_tool_calls);
    assert!(!find_tool(&tools, "write_stdin").supports_parallel_tool_calls);
}

#[test]
fn test_test_model_info_includes_sync_tool() {
    let mut model_info = model_info();
    model_info.experimental_supported_tools = vec!["test_sync_tool".to_string()];
    let features = Features::with_defaults();
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );

    assert!(tools.iter().any(|tool| tool.name() == "test_sync_tool"));
}

#[test]
fn test_build_specs_mcp_tools_converted() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Live),
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
                        "string_argument": { "type": "string" },
                        "number_argument": { "type": "number" },
                        "object_argument": {
                            "type": "object",
                            "properties": {
                                "string_property": { "type": "string" },
                                "number_property": { "type": "number" },
                            },
                            "required": ["string_property", "number_property"],
                            "additionalProperties": false,
                        },
                    },
                }),
            ),
        )])),
        /*deferred_mcp_tools*/ None,
        &[],
    );

    let tool = find_tool(&tools, "test_server/do_something_cool");
    assert_eq!(
        &tool.spec,
        &ToolSpec::Function(ResponsesApiTool {
            name: "test_server/do_something_cool".to_string(),
            parameters: JsonSchema::object(
                BTreeMap::from([
                    (
                        "string_argument".to_string(),
                        JsonSchema::string(/*description*/ None),
                    ),
                    (
                        "number_argument".to_string(),
                        JsonSchema::number(/*description*/ None),
                    ),
                    (
                        "object_argument".to_string(),
                        JsonSchema::object(
                            BTreeMap::from([
                                (
                                    "string_property".to_string(),
                                    JsonSchema::string(/*description*/ None),
                                ),
                                (
                                    "number_property".to_string(),
                                    JsonSchema::number(/*description*/ None),
                                ),
                            ]),
                            Some(vec![
                                "string_property".to_string(),
                                "number_property".to_string(),
                            ]),
                            Some(false.into()),
                        ),
                    ),
                ]),
                /*required*/ None,
                /*additional_properties*/ None
            ),
            description: "Do something cool".to_string(),
            strict: false,
            output_schema: Some(mcp_call_tool_result_output_schema(serde_json::json!({}))),
            defer_loading: None,
        })
    );
}

#[test]
fn test_build_specs_mcp_tools_sorted_by_name() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let tools_map = HashMap::from([
        (
            "test_server/do".to_string(),
            mcp_tool("do", "a", serde_json::json!({"type": "object"})),
        ),
        (
            "test_server/something".to_string(),
            mcp_tool("something", "b", serde_json::json!({"type": "object"})),
        ),
        (
            "test_server/cool".to_string(),
            mcp_tool("cool", "c", serde_json::json!({"type": "object"})),
        ),
    ]);

    let (tools, _) = build_specs(
        &tools_config,
        Some(tools_map),
        /*deferred_mcp_tools*/ None,
        &[],
    );

    let mcp_names: Vec<_> = tools
        .iter()
        .map(|tool| tool.name().to_string())
        .filter(|name| name.starts_with("test_server/"))
        .collect();
    let expected = vec![
        "test_server/cool".to_string(),
        "test_server/do".to_string(),
        "test_server/something".to_string(),
    ];
    assert_eq!(mcp_names, expected);
}

#[test]
fn search_tool_description_lists_each_mcp_source_once() {
    let model_info = search_capable_model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::Apps);
    features.enable(Feature::ToolSearch);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, handlers) = build_specs(
        &tools_config,
        Some(HashMap::from([
            (
                "mcp__codex_apps__calendar_create_event".to_string(),
                mcp_tool(
                    "calendar_create_event",
                    "Create calendar event",
                    serde_json::json!({"type": "object"}),
                ),
            ),
            (
                "mcp__rmcp__echo".to_string(),
                mcp_tool("echo", "Echo", serde_json::json!({"type": "object"})),
            ),
        ])),
        Some(vec![
            deferred_mcp_tool(
                "_create_event",
                "mcp__codex_apps__calendar",
                CODEX_APPS_MCP_SERVER_NAME,
                Some("Calendar"),
                Some("Plan events and manage your calendar."),
            ),
            deferred_mcp_tool(
                "_list_events",
                "mcp__codex_apps__calendar",
                CODEX_APPS_MCP_SERVER_NAME,
                Some("Calendar"),
                Some("Plan events and manage your calendar."),
            ),
            deferred_mcp_tool(
                "_search_threads",
                "mcp__codex_apps__gmail",
                CODEX_APPS_MCP_SERVER_NAME,
                Some("Gmail"),
                Some("Find and summarize email threads."),
            ),
            deferred_mcp_tool(
                "echo",
                "mcp__rmcp__",
                "rmcp",
                /*connector_name*/ None,
                /*connector_description*/ None,
            ),
        ]),
        &[],
    );

    let search_tool = find_tool(&tools, TOOL_SEARCH_TOOL_NAME);
    let ToolSpec::ToolSearch { description, .. } = &search_tool.spec else {
        panic!("expected tool_search tool");
    };
    let description = description.as_str();
    assert!(description.contains("- Calendar: Plan events and manage your calendar."));
    assert!(description.contains("- Gmail: Find and summarize email threads."));
    assert_eq!(
        description
            .matches("- Calendar: Plan events and manage your calendar.")
            .count(),
        1
    );
    assert!(description.contains("- rmcp"));
    assert!(!description.contains("mcp__rmcp__echo"));

    assert!(handlers.contains(&ToolHandlerSpec {
        name: ToolName::namespaced("mcp__codex_apps__calendar", "_create_event"),
        kind: ToolHandlerKind::Mcp,
    }));
    assert!(handlers.contains(&ToolHandlerSpec {
        name: ToolName::namespaced("mcp__rmcp__", "echo"),
        kind: ToolHandlerKind::Mcp,
    }));
}

#[test]
fn search_tool_requires_model_capability_and_feature_flag() {
    let model_info = search_capable_model_info();
    let deferred_mcp_tools = Some(vec![deferred_mcp_tool(
        "_create_event",
        "mcp__codex_apps__calendar",
        CODEX_APPS_MCP_SERVER_NAME,
        Some("Calendar"),
        /*connector_description*/ None,
    )]);

    let features = Features::with_defaults();
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &ModelInfo {
            supports_search_tool: false,
            ..model_info.clone()
        },
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        deferred_mcp_tools.clone(),
        &[],
    );
    assert_lacks_tool_name(&tools, TOOL_SEARCH_TOOL_NAME);

    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        deferred_mcp_tools.clone(),
        &[],
    );
    assert_lacks_tool_name(&tools, TOOL_SEARCH_TOOL_NAME);

    let mut features = Features::with_defaults();
    features.enable(Feature::ToolSearch);
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        deferred_mcp_tools,
        &[],
    );
    assert_contains_tool_names(&tools, &[TOOL_SEARCH_TOOL_NAME]);
}

#[test]
fn tool_suggest_is_not_registered_without_feature_flag() {
    let model_info = search_capable_model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::ToolSearch);
    features.enable(Feature::Apps);
    features.enable(Feature::Plugins);
    features.disable(Feature::ToolSuggest);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs_with_discoverable_tools(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        Some(vec![discoverable_connector(
            "connector_2128aebfecb84f64a069897515042a44",
            "Google Calendar",
            "Plan events and schedules.",
        )]),
        &[],
    );

    assert!(
        !tools
            .iter()
            .any(|tool| tool.name() == TOOL_SUGGEST_TOOL_NAME)
    );
}

#[test]
fn tool_suggest_can_be_registered_without_search_tool() {
    let model_info = ModelInfo {
        supports_search_tool: false,
        ..search_capable_model_info()
    };
    let mut features = Features::with_defaults();
    features.enable(Feature::Apps);
    features.enable(Feature::Plugins);
    features.enable(Feature::ToolSuggest);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });
    let (tools, _) = build_specs_with_discoverable_tools(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        Some(vec![discoverable_connector(
            "connector_2128aebfecb84f64a069897515042a44",
            "Google Calendar",
            "Plan events and schedules.",
        )]),
        &[],
    );

    assert_contains_tool_names(&tools, &[TOOL_SUGGEST_TOOL_NAME]);
    assert_lacks_tool_name(&tools, TOOL_SEARCH_TOOL_NAME);

    let tool_suggest = find_tool(&tools, TOOL_SUGGEST_TOOL_NAME);
    let ToolSpec::Function(ResponsesApiTool { description, .. }) = &tool_suggest.spec else {
        panic!("expected function tool");
    };
    assert!(description.contains(
        "Suggests a missing connector in an installed plugin, or in narrower cases a not installed but discoverable plugin"
    ));
    assert!(description.contains(
        "You've already tried to find a matching available tool for the user's request but couldn't find a good match. This includes `tool_search` (if available) and other means."
    ));
}

#[test]
fn tool_suggest_description_lists_discoverable_tools() {
    let model_info = search_capable_model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::Apps);
    features.enable(Feature::Plugins);
    features.enable(Feature::ToolSearch);
    features.enable(Feature::ToolSuggest);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let discoverable_tools = vec![
        discoverable_connector(
            "connector_2128aebfecb84f64a069897515042a44",
            "Google Calendar",
            "Plan events and schedules.",
        ),
        discoverable_connector(
            "connector_68df038e0ba48191908c8434991bbac2",
            "Gmail",
            "Find and summarize email threads.",
        ),
        DiscoverableTool::Plugin(Box::new(DiscoverablePluginInfo {
            id: "sample@test".to_string(),
            name: "Sample Plugin".to_string(),
            description: None,
            has_skills: true,
            mcp_server_names: vec!["sample-docs".to_string()],
            app_connector_ids: vec!["connector_sample".to_string()],
        })),
    ];

    let (tools, _) = build_specs_with_discoverable_tools(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        Some(discoverable_tools),
        &[],
    );

    let tool_suggest = find_tool(&tools, TOOL_SUGGEST_TOOL_NAME);
    let ToolSpec::Function(ResponsesApiTool {
        description,
        parameters,
        ..
    }) = &tool_suggest.spec
    else {
        panic!("expected function tool");
    };
    assert!(description.contains(
        "Suggests a missing connector in an installed plugin, or in narrower cases a not installed but discoverable plugin"
    ));
    assert!(description.contains("Google Calendar"));
    assert!(description.contains("Gmail"));
    assert!(description.contains("Sample Plugin"));
    assert!(description.contains("Plan events and schedules."));
    assert!(description.contains("Find and summarize email threads."));
    assert!(description.contains("id: `sample@test`, type: plugin, action: install"));
    assert!(description.contains("`action_type`: `install` or `enable`"));
    assert!(
        description.contains("skills; MCP servers: sample-docs; app connectors: connector_sample")
    );
    assert!(
        description.contains(
            "You've already tried to find a matching available tool for the user's request but couldn't find a good match. This includes `tool_search` (if available) and other means."
        )
    );
    assert!(description.contains(
        "For connectors/apps that are not installed but needed for an installed plugin, suggest to install them if the task requirements match precisely."
    ));
    assert!(description.contains(
        "For plugins that are not installed but discoverable, only suggest discoverable and installable plugins when the user's intent very explicitly and unambiguously matches that plugin itself."
    ));
    assert!(description.contains(
        "Do not suggest a plugin just because one of its connectors or capabilities seems relevant."
    ));
    assert!(description.contains(
        "Apply the stricter explicit-and-unambiguous rule for *discoverable tools* like plugin install suggestions; *missing tools* like connector install suggestions continue to use the normal clear-fit standard."
    ));
    assert!(description.contains("DO NOT explore or recommend tools that are not on this list."));
    assert!(!description.contains("{{discoverable_tools}}"));
    assert!(!description.contains("tool_search fails to find a good match"));
    let (_, required) = expect_object_schema(parameters);
    assert_eq!(
        required,
        Some(&vec![
            "tool_type".to_string(),
            "action_type".to_string(),
            "tool_id".to_string(),
            "suggest_reason".to_string(),
        ])
    );
}

#[test]
fn code_mode_augments_mcp_tool_descriptions_with_namespaced_sample() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeMode);
    features.enable(Feature::CodeModeOnly);
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        Some(HashMap::from([(
            "mcp__sample__echo".to_string(),
            mcp_tool(
                "echo",
                "Echo text",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    },
                    "required": ["message"],
                    "additionalProperties": false
                }),
            ),
        )])),
        /*deferred_mcp_tools*/ None,
        &[],
    );

    let ToolSpec::Function(ResponsesApiTool { description, .. }) =
        &find_tool(&tools, "mcp__sample__echo").spec
    else {
        panic!("expected function tool");
    };

    assert_eq!(
        description,
        r#"Echo text

exec tool declaration:
```ts
declare const tools: { mcp__sample__echo(args: { message: string; }): Promise<CallToolResult>; };
```"#
    );
}

#[test]
fn code_mode_preserves_nullable_and_literal_mcp_input_shapes() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeMode);
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        Some(HashMap::from([(
            "mcp__sample__fn".to_string(),
            mcp_tool(
                "fn",
                "Sample fn",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "open": {
                            "anyOf": [
                                {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "ref_id": {"type": "string"},
                                            "lineno": {"anyOf": [{"type": "integer"}, {"type": "null"}]}
                                        },
                                        "required": ["ref_id"],
                                        "additionalProperties": false
                                    }
                                },
                                {"type": "null"}
                            ]
                        },
                        "tagged_list": {
                            "anyOf": [
                                {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "kind": {"type": "const", "const": "tagged"},
                                            "variant": {"type": "enum", "enum": ["alpha", "beta"]},
                                            "scope": {"type": "enum", "enum": ["one", "two"]}
                                        },
                                        "required": ["kind", "variant", "scope"]
                                    }
                                },
                                {"type": "null"}
                            ]
                        },
                        "response_length": {"type": "enum", "enum": ["short", "medium", "long"]}
                    },
                    "additionalProperties": false
                }),
            ),
        )])),
        /*deferred_mcp_tools*/ None,
        &[],
    );

    let ToolSpec::Function(ResponsesApiTool { description, .. }) =
        &find_tool(&tools, "mcp__sample__fn").spec
    else {
        panic!("expected function tool");
    };

    assert!(description.contains(
        r#"exec tool declaration:
```ts
declare const tools: { mcp__sample__fn(args: { open?: Array<{ lineno?: number | null; ref_id: string; }> | null; response_length?: "short" | "medium" | "long"; tagged_list?: Array<{ kind: "tagged"; scope: "one" | "two"; variant: "alpha" | "beta"; }> | null; }): Promise<CallToolResult>; };
```"#
    ));
}

#[test]
fn code_mode_augments_builtin_tool_descriptions_with_typed_sample() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeMode);
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    let ToolSpec::Function(ResponsesApiTool { description, .. }) =
        &find_tool(&tools, VIEW_IMAGE_TOOL_NAME).spec
    else {
        panic!("expected function tool");
    };

    assert_eq!(
        description,
        "View a local image from the filesystem (only use if given a full filepath by the user, and the image isn't already attached to the thread context within <image ...> tags).\n\nexec tool declaration:\n```ts\ndeclare const tools: { view_image(args: {\n  // Local filesystem path to an image file\n  path: string;\n}): Promise<{\n  // Image detail hint returned by view_image. Returns `original` when original resolution is preserved, otherwise `null`.\n  detail: string | null;\n  // Data URL for the loaded image.\n  image_url: string;\n}>; };\n```"
    );
}

#[test]
fn code_mode_only_exec_description_includes_full_nested_tool_details() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeMode);
    features.enable(Feature::CodeModeOnly);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    let ToolSpec::Freeform(FreeformTool { description, .. }) = &find_tool(&tools, "exec").spec
    else {
        panic!("expected freeform tool");
    };

    assert!(!description.contains("Enabled nested tools:"));
    assert!(!description.contains("Nested tool reference:"));
    assert!(description.starts_with(
        "Use `exec/wait` tool to run all other tools, do not attempt to use any other tools directly"
    ));
    assert!(description.contains("### `update_plan`"));
    assert!(description.contains("### `view_image`"));
}

#[test]
fn code_mode_exec_description_omits_nested_tool_details_when_not_code_mode_only() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeMode);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let (tools, _) = build_specs(
        &tools_config,
        /*mcp_tools*/ None,
        /*deferred_mcp_tools*/ None,
        &[],
    );
    let ToolSpec::Freeform(FreeformTool { description, .. }) = &find_tool(&tools, "exec").spec
    else {
        panic!("expected freeform tool");
    };

    assert!(!description.starts_with(
        "Use `exec/wait` tool to run all other tools, do not attempt to use any other tools directly"
    ));
    assert!(!description.contains("### `update_plan`"));
    assert!(!description.contains("### `view_image`"));
}

fn model_info() -> ModelInfo {
    serde_json::from_value(json!({
        "slug": "gpt-5-codex",
        "display_name": "GPT-5 Codex",
        "description": null,
        "supported_reasoning_levels": [],
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": 1,
        "availability_nux": null,
        "upgrade": null,
        "base_instructions": "base",
        "model_messages": null,
        "supports_reasoning_summaries": false,
        "default_reasoning_summary": "auto",
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": "freeform",
        "truncation_policy": {
            "mode": "bytes",
            "limit": 10000
        },
        "supports_parallel_tool_calls": false,
        "supports_image_detail_original": false,
        "context_window": null,
        "auto_compact_token_limit": null,
        "effective_context_window_percent": 95,
        "experimental_supported_tools": [],
        "input_modalities": ["text", "image"],
        "supports_search_tool": false
    }))
    .expect("deserialize test model")
}

fn search_capable_model_info() -> ModelInfo {
    ModelInfo {
        supports_search_tool: true,
        ..model_info()
    }
}

fn build_specs<'a>(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    deferred_mcp_tools: Option<Vec<ToolRegistryPlanDeferredTool<'a>>>,
    dynamic_tools: &[DynamicToolSpec],
) -> (Vec<ConfiguredToolSpec>, Vec<ToolHandlerSpec>) {
    build_specs_with_discoverable_tools(
        config,
        mcp_tools,
        deferred_mcp_tools,
        /*discoverable_tools*/ None,
        dynamic_tools,
    )
}

fn build_specs_with_discoverable_tools<'a>(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    deferred_mcp_tools: Option<Vec<ToolRegistryPlanDeferredTool<'a>>>,
    discoverable_tools: Option<Vec<DiscoverableTool>>,
    dynamic_tools: &[DynamicToolSpec],
) -> (Vec<ConfiguredToolSpec>, Vec<ToolHandlerSpec>) {
    build_specs_with_optional_tool_namespaces(
        config,
        mcp_tools,
        deferred_mcp_tools,
        /*tool_namespaces*/ None,
        discoverable_tools,
        dynamic_tools,
    )
}

fn build_specs_with_optional_tool_namespaces<'a>(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, rmcp::model::Tool>>,
    deferred_mcp_tools: Option<Vec<ToolRegistryPlanDeferredTool<'a>>>,
    tool_namespaces: Option<HashMap<ToolName, ToolNamespace>>,
    discoverable_tools: Option<Vec<DiscoverableTool>>,
    dynamic_tools: &[DynamicToolSpec],
) -> (Vec<ConfiguredToolSpec>, Vec<ToolHandlerSpec>) {
    let mcp_tool_inputs = mcp_tools.as_ref().map(|mcp_tools| {
        mcp_tools
            .iter()
            .map(|(qualified_name, tool)| {
                let raw_tool_name = tool.name.as_ref();
                let callable_namespace = qualified_name
                    .strip_suffix(raw_tool_name)
                    .filter(|namespace| !namespace.is_empty())
                    .unwrap_or("mcp__test_server__");

                ToolRegistryPlanMcpTool {
                    name: ToolName::namespaced(callable_namespace.to_string(), raw_tool_name),
                    tool,
                }
            })
            .collect::<Vec<_>>()
    });
    let plan = build_tool_registry_plan(
        config,
        ToolRegistryPlanParams {
            mcp_tools: mcp_tool_inputs.as_deref(),
            deferred_mcp_tools: deferred_mcp_tools.as_deref(),
            tool_namespaces: tool_namespaces.as_ref(),
            discoverable_tools: discoverable_tools.as_deref(),
            dynamic_tools,
            default_agent_type_description: DEFAULT_AGENT_TYPE_DESCRIPTION,
            wait_agent_timeouts: wait_agent_timeout_options(),
        },
    );
    (plan.specs, plan.handlers)
}

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

#[test]
fn code_mode_augments_mcp_tool_descriptions_with_structured_output_sample() {
    let model_info = model_info();
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeMode);
    features.enable(Feature::CodeModeOnly);
    features.enable(Feature::UnifiedExec);
    let available_models = Vec::new();
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &model_info,
        available_models: &available_models,
        features: &features,
        image_generation_tool_auth_allowed: true,
        web_search_mode: Some(WebSearchMode::Cached),
        session_source: SessionSource::Cli,
        sandbox_policy: &SandboxPolicy::DangerFullAccess,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
    });

    let mut tool = mcp_tool(
        "echo",
        "Echo text",
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {"type": "string"}
            },
            "required": ["message"],
            "additionalProperties": false
        }),
    );
    tool.output_schema = Some(std::sync::Arc::new(rmcp::model::object(
        serde_json::json!({
            "type": "object",
            "properties": {
                "echo": {"type": "string"},
                "env": {
                    "anyOf": [
                        {"type": "string"},
                        {"type": "null"}
                    ]
                }
            },
            "required": ["echo", "env"],
            "additionalProperties": false
        }),
    )));

    let (tools, _) = build_specs(
        &tools_config,
        Some(HashMap::from([("mcp__sample__echo".to_string(), tool)])),
        /*deferred_mcp_tools*/ None,
        &[],
    );

    let ToolSpec::Function(ResponsesApiTool { description, .. }) =
        &find_tool(&tools, "mcp__sample__echo").spec
    else {
        panic!("expected function tool");
    };

    assert_eq!(
        description,
        r#"Echo text

exec tool declaration:
```ts
declare const tools: { mcp__sample__echo(args: { message: string; }): Promise<CallToolResult<{ echo: string; env: string | null; }>>; };
```"#
    );
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

fn deferred_mcp_tool<'a>(
    tool_name: &'a str,
    tool_namespace: &'a str,
    server_name: &'a str,
    connector_name: Option<&'a str>,
    connector_description: Option<&'a str>,
) -> ToolRegistryPlanDeferredTool<'a> {
    ToolRegistryPlanDeferredTool {
        name: ToolName::namespaced(tool_namespace, tool_name),
        server_name,
        connector_name,
        connector_description,
    }
}

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

fn assert_lacks_tool_name(tools: &[ConfiguredToolSpec], expected_absent: &str) {
    let names = tools
        .iter()
        .map(ConfiguredToolSpec::name)
        .collect::<Vec<_>>();
    assert!(
        !names.contains(&expected_absent),
        "expected tool {expected_absent} to be absent; had: {names:?}"
    );
}

fn request_user_input_tool_spec(default_mode_request_user_input: bool) -> ToolSpec {
    create_request_user_input_tool(request_user_input_tool_description(
        default_mode_request_user_input,
    ))
}

fn spawn_agent_tool_options(config: &ToolsConfig) -> SpawnAgentToolOptions<'_> {
    SpawnAgentToolOptions {
        available_models: &config.available_models,
        agent_type_description: agent_type_description(config, DEFAULT_AGENT_TYPE_DESCRIPTION),
        hide_agent_type_model_reasoning: config.hide_spawn_agent_metadata,
        include_usage_hint: config.spawn_agent_usage_hint,
        usage_hint_text: config.spawn_agent_usage_hint_text.clone(),
    }
}

fn wait_agent_timeout_options() -> WaitAgentTimeoutOptions {
    WaitAgentTimeoutOptions {
        default_timeout_ms: DEFAULT_WAIT_TIMEOUT_MS,
        min_timeout_ms: MIN_WAIT_TIMEOUT_MS,
        max_timeout_ms: MAX_WAIT_TIMEOUT_MS,
    }
}

fn find_tool<'a>(tools: &'a [ConfiguredToolSpec], expected_name: &str) -> &'a ConfiguredToolSpec {
    tools
        .iter()
        .find(|tool| tool.name() == expected_name)
        .unwrap_or_else(|| panic!("expected tool {expected_name}"))
}

fn expect_object_schema(
    schema: &JsonSchema,
) -> (&BTreeMap<String, JsonSchema>, Option<&Vec<String>>) {
    assert_eq!(
        schema.schema_type,
        Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object))
    );
    let properties = schema
        .properties
        .as_ref()
        .expect("expected object properties");
    (properties, schema.required.as_ref())
}

fn expect_string_description(schema: &JsonSchema) -> &str {
    assert_eq!(
        schema.schema_type,
        Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::String))
    );
    schema.description.as_deref().expect("expected description")
}

fn strip_descriptions_schema(schema: &mut JsonSchema) {
    if let Some(variants) = &mut schema.any_of {
        for variant in variants {
            strip_descriptions_schema(variant);
        }
    }
    if let Some(items) = &mut schema.items {
        strip_descriptions_schema(items);
    }
    if let Some(properties) = &mut schema.properties {
        for value in properties.values_mut() {
            strip_descriptions_schema(value);
        }
    }
    if let Some(AdditionalProperties::Schema(schema)) = &mut schema.additional_properties {
        strip_descriptions_schema(schema);
    }
    schema.description = None;
}

fn strip_descriptions_tool(spec: &mut ToolSpec) {
    match spec {
        ToolSpec::ToolSearch { parameters, .. } => strip_descriptions_schema(parameters),
        ToolSpec::Function(ResponsesApiTool { parameters, .. }) => {
            strip_descriptions_schema(parameters);
        }
        ToolSpec::Freeform(FreeformTool { .. })
        | ToolSpec::LocalShell {}
        | ToolSpec::ImageGeneration { .. }
        | ToolSpec::WebSearch { .. } => {}
    }
}
