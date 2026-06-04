use std::collections::BTreeMap;
use std::sync::Arc;

use codex_features::Feature;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_mcp::ToolInfo;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_tools::DiscoverablePluginInfo;
use codex_tools::DiscoverableTool;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolCall as ExtensionToolCall;
use codex_tools::ToolExecutor;
use codex_tools::ToolExposure;
use codex_tools::ToolName;
use codex_tools::ToolOutput;
use codex_tools::ToolSpec;
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::session::tests::make_session_and_context;
use crate::session::turn_context::TurnContext;
use crate::tools::handlers::multi_agents_spec::MULTI_AGENT_V1_NAMESPACE;
use crate::tools::router::ToolRouter;
use crate::tools::router::ToolRouterParams;

#[derive(Default)]
struct ToolPlanInputs {
    mcp_tools: Option<Vec<ToolInfo>>,
    deferred_mcp_tools: Option<Vec<ToolInfo>>,
    discoverable_tools: Option<Vec<DiscoverableTool>>,
    extension_tool_executors: Vec<Arc<dyn ToolExecutor<ExtensionToolCall>>>,
    dynamic_tools: Vec<DynamicToolSpec>,
}

struct ToolPlanProbe {
    visible_specs: Vec<ToolSpec>,
    visible_names: Vec<String>,
    namespace_functions: BTreeMap<String, Vec<String>>,
    registered_names: Vec<String>,
    exposures: BTreeMap<String, ToolExposure>,
}

impl ToolPlanProbe {
    fn from_router(router: ToolRouter) -> Self {
        let visible_specs = router.model_visible_specs();
        let visible_names = visible_specs
            .iter()
            .map(|spec| spec.name().to_string())
            .collect::<Vec<_>>();
        let namespace_functions = visible_specs
            .iter()
            .filter_map(|spec| match spec {
                ToolSpec::Namespace(namespace) => Some((
                    namespace.name.clone(),
                    namespace
                        .tools
                        .iter()
                        .map(|tool| match tool {
                            ResponsesApiNamespaceTool::Function(tool) => tool.name.clone(),
                        })
                        .collect::<Vec<_>>(),
                )),
                ToolSpec::Function(_)
                | ToolSpec::ToolSearch { .. }
                | ToolSpec::ImageGeneration { .. }
                | ToolSpec::WebSearch { .. }
                | ToolSpec::Freeform(_) => None,
            })
            .collect::<BTreeMap<_, _>>();
        let registered_tool_names = router.registered_tool_names_for_test();
        let registered_names = registered_tool_names
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let exposures = registered_tool_names
            .iter()
            .filter_map(|name| {
                router
                    .tool_exposure_for_test(name)
                    .map(|exposure| (name.to_string(), exposure))
            })
            .collect::<BTreeMap<_, _>>();

        Self {
            visible_specs,
            visible_names,
            namespace_functions,
            registered_names,
            exposures,
        }
    }

    fn assert_visible_contains(&self, expected: &[&str]) {
        for name in expected {
            assert!(
                self.visible_names.iter().any(|visible| visible == name),
                "expected visible tool `{name}` in {:?}",
                self.visible_names
            );
        }
    }

    fn assert_visible_lacks(&self, expected_absent: &[&str]) {
        for name in expected_absent {
            assert!(
                !self.visible_names.iter().any(|visible| visible == name),
                "expected visible tool `{name}` to be absent from {:?}",
                self.visible_names
            );
        }
    }

    fn assert_registered_contains(&self, expected: &[&str]) {
        for name in expected {
            assert!(
                self.registered_names
                    .iter()
                    .any(|registered| registered == name),
                "expected registered tool `{name}` in {:?}",
                self.registered_names
            );
        }
    }

    fn assert_registered_lacks(&self, expected_absent: &[&str]) {
        for name in expected_absent {
            assert!(
                !self
                    .registered_names
                    .iter()
                    .any(|registered| registered == name),
                "expected registered tool `{name}` to be absent from {:?}",
                self.registered_names
            );
        }
    }

    fn namespace_function_names(&self, namespace: &str) -> &[String] {
        self.namespace_functions
            .get(namespace)
            .map_or(&[], Vec::as_slice)
    }

    fn visible_spec(&self, name: &str) -> &ToolSpec {
        self.visible_specs
            .iter()
            .find(|spec| spec.name() == name)
            .unwrap_or_else(|| panic!("expected visible spec `{name}` in {:?}", self.visible_names))
    }

    fn exposure(&self, name: &str) -> ToolExposure {
        *self
            .exposures
            .get(name)
            .unwrap_or_else(|| panic!("expected registered tool `{name}`"))
    }
}

async fn probe_with(
    configure_turn: impl FnOnce(&mut TurnContext),
    inputs: ToolPlanInputs,
) -> ToolPlanProbe {
    let (_session, mut turn) = make_session_and_context().await;
    configure_turn(&mut turn);
    let router = ToolRouter::from_turn_context(
        &turn,
        ToolRouterParams {
            mcp_tools: inputs.mcp_tools,
            deferred_mcp_tools: inputs.deferred_mcp_tools,
            discoverable_tools: inputs.discoverable_tools,
            extension_tool_executors: inputs.extension_tool_executors,
            dynamic_tools: inputs.dynamic_tools.as_slice(),
        },
    );
    ToolPlanProbe::from_router(router)
}

async fn probe(configure_turn: impl FnOnce(&mut TurnContext)) -> ToolPlanProbe {
    probe_with(configure_turn, ToolPlanInputs::default()).await
}

fn set_feature(turn: &mut TurnContext, feature: Feature, enabled: bool) {
    if enabled {
        turn.features
            .enable(feature)
            .expect("test feature should be enableable");
    } else {
        turn.features
            .disable(feature)
            .expect("test feature should be disableable");
    }

    let mut config = (*turn.config).clone();
    if enabled {
        config
            .features
            .enable(feature)
            .expect("test feature should be enableable in config");
    } else {
        config
            .features
            .disable(feature)
            .expect("test feature should be disableable in config");
    }
    turn.multi_agent_version = config.multi_agent_version_from_features();
    turn.config = Arc::new(config);
    turn.tool_mode = turn.model_info.tool_mode.unwrap_or_else(|| {
        if turn.config.features.enabled(Feature::CodeModeOnly) {
            ToolMode::CodeModeOnly
        } else if turn.config.features.enabled(Feature::CodeMode) {
            ToolMode::CodeMode
        } else {
            ToolMode::Direct
        }
    });
}

fn set_features(turn: &mut TurnContext, features: &[Feature]) {
    for feature in features {
        set_feature(turn, *feature, /*enabled*/ true);
    }
}

fn zsh_fork_config_for_spec_plan_tests() -> codex_tools::ZshForkConfig {
    let placeholder_exe = codex_utils_absolute_path::AbsolutePathBuf::try_from(
        std::env::current_exe().expect("current exe path"),
    )
    .expect("current exe should be absolute");

    // Spec planning only checks whether the shell mode is ZshFork. These paths
    // are never executed, so use a stable absolute placeholder instead of
    // depending on packaged zsh-fork artifacts in schema tests.
    codex_tools::ZshForkConfig {
        shell_zsh_path: placeholder_exe.clone(),
        main_execve_wrapper_exe: placeholder_exe,
    }
}

fn update_config(turn: &mut TurnContext, update: impl FnOnce(&mut crate::config::Config)) {
    let mut config = (*turn.config).clone();
    update(&mut config);
    turn.config = Arc::new(config);
}

fn set_web_search_mode(turn: &mut TurnContext, mode: WebSearchMode) {
    update_config(turn, |config| {
        config
            .web_search_mode
            .set(mode)
            .expect("test web search mode should be accepted");
    });
}

fn use_chatgpt_auth(turn: &mut TurnContext) {
    turn.auth_manager = Some(AuthManager::from_auth_for_testing(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    ));
    turn.provider = create_model_provider(
        turn.config.model_provider.clone(),
        turn.auth_manager.clone(),
    );
}

fn use_bedrock_provider(turn: &mut TurnContext) {
    let provider_info = ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None);
    update_config(turn, |config| {
        config.model_provider_id = AMAZON_BEDROCK_PROVIDER_ID.to_string();
        config.model_provider = provider_info.clone();
    });
    turn.provider = create_model_provider(provider_info, turn.auth_manager.clone());
}

struct WebRunExtensionTool;

#[async_trait::async_trait]
impl ToolExecutor<ExtensionToolCall> for WebRunExtensionTool {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced("web", "run")
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Namespace(codex_tools::ResponsesApiNamespace {
            name: "web".to_string(),
            description: "Test web namespace.".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "run".to_string(),
                description: "Test standalone web search tool.".to_string(),
                strict: false,
                defer_loading: None,
                parameters: codex_tools::JsonSchema::default(),
                output_schema: None,
            })],
        })
    }

    async fn handle(
        &self,
        _call: ExtensionToolCall,
    ) -> Result<Box<dyn ToolOutput>, codex_tools::FunctionCallError> {
        Ok(Box::new(codex_tools::JsonToolOutput::new(json!({}))))
    }
}

struct DeferredExtensionTool;

#[async_trait::async_trait]
impl ToolExecutor<ExtensionToolCall> for DeferredExtensionTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("extension_echo")
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(ResponsesApiTool {
            name: "extension_echo".to_string(),
            description: "Echoes arguments through an extension tool.".to_string(),
            strict: true,
            defer_loading: None,
            parameters: codex_tools::JsonSchema::object(
                BTreeMap::from([(
                    "message".to_string(),
                    codex_tools::JsonSchema::string(/*description*/ None),
                )]),
                Some(vec!["message".to_string()]),
                Some(false.into()),
            ),
            output_schema: None,
        })
    }

    fn exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    async fn handle(
        &self,
        _call: ExtensionToolCall,
    ) -> Result<Box<dyn ToolOutput>, codex_tools::FunctionCallError> {
        panic!("spec planning should not execute extension tools")
    }
}

fn duplicate_primary_environment(turn: &mut TurnContext) {
    let mut second_environment = turn.environments.turn_environments[0].clone();
    second_environment.environment_id = "secondary".to_string();
    turn.environments.turn_environments.push(second_environment);
}

fn mcp_tool(server: &str, namespace: &str, name: &str) -> ToolInfo {
    ToolInfo {
        server_name: server.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: name.to_string(),
        callable_namespace: namespace.to_string(),
        namespace_description: Some(format!("Tools from {server}.")),
        tool: rmcp::model::Tool::new(
            name.to_string(),
            format!("{name} test tool"),
            Arc::new(rmcp::model::object(json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }))),
        ),
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
    }
}

fn invalid_mcp_tool(server: &str, namespace: &str, name: &str) -> ToolInfo {
    let mut tool = mcp_tool(server, namespace, name);
    tool.tool.input_schema = Arc::new(rmcp::model::object(json!({
        "type": "null",
    })));
    tool
}

fn dynamic_tool(namespace: Option<&str>, name: &str, defer_loading: bool) -> DynamicToolSpec {
    DynamicToolSpec {
        namespace: namespace.map(str::to_string),
        name: name.to_string(),
        description: format!("{name} dynamic tool"),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false,
        }),
        defer_loading,
    }
}

fn discoverable_plugin(id: &str, name: &str) -> DiscoverableTool {
    DiscoverablePluginInfo {
        id: id.to_string(),
        remote_plugin_id: None,
        name: name.to_string(),
        description: Some(format!("{name} plugin")),
        has_skills: false,
        mcp_server_names: Vec::new(),
        app_connector_ids: Vec::new(),
    }
    .into()
}

fn has_parameter(spec: &ToolSpec, parameter_name: &str) -> bool {
    serde_json::to_value(spec)
        .expect("tool spec should serialize")
        .pointer(&format!("/parameters/properties/{parameter_name}"))
        .is_some()
}

fn apply_patch_accepts_environment_id(spec: &ToolSpec) -> bool {
    match spec {
        ToolSpec::Freeform(tool) if tool.name == "apply_patch" => {
            tool.format.definition.contains("Environment ID")
        }
        _ => false,
    }
}

#[tokio::test]
async fn request_user_input_tool_respects_experimental_config_gate() {
    let enabled = probe(|_| {}).await;
    enabled.assert_visible_contains(&["request_user_input"]);
    enabled.assert_registered_contains(&["request_user_input"]);

    let disabled = probe(|turn| {
        update_config(turn, |config| {
            config.experimental_request_user_input_enabled = false;
        });
    })
    .await;
    disabled.assert_visible_lacks(&["request_user_input"]);
    disabled.assert_registered_lacks(&["request_user_input"]);
}

#[tokio::test]
async fn shell_family_registers_visible_unified_exec_and_hidden_legacy_shell() {
    let plan = probe(|turn| {
        set_features(turn, &[Feature::ShellTool, Feature::UnifiedExec]);
        set_feature(turn, Feature::ShellZshFork, /*enabled*/ false);
        turn.model_info.shell_type = ConfigShellToolType::ShellCommand;
    })
    .await;

    plan.assert_visible_contains(&["exec_command", "write_stdin"]);
    plan.assert_visible_lacks(&["shell_command"]);
    plan.assert_registered_contains(&["exec_command", "write_stdin", "shell_command"]);
    assert_eq!(plan.exposure("shell_command"), ToolExposure::Hidden);
    assert!(has_parameter(plan.visible_spec("exec_command"), "shell"));
}

#[tokio::test]
async fn shell_zsh_fork_stays_standalone_until_unified_exec_composition_is_enabled() {
    let standalone = probe(|turn| {
        set_features(turn, &[Feature::ShellTool, Feature::UnifiedExec]);
        set_feature(turn, Feature::ShellZshFork, /*enabled*/ true);
        set_feature(turn, Feature::UnifiedExecZshFork, /*enabled*/ false);
        turn.model_info.shell_type = ConfigShellToolType::ShellCommand;
    })
    .await;

    standalone.assert_visible_contains(&["shell_command"]);
    standalone.assert_visible_lacks(&["exec_command", "write_stdin"]);
    standalone.assert_registered_contains(&["shell_command"]);
    standalone.assert_registered_lacks(&["exec_command", "write_stdin"]);

    let composed = probe(|turn| {
        set_features(
            turn,
            &[
                Feature::ShellTool,
                Feature::UnifiedExec,
                Feature::ShellZshFork,
                Feature::UnifiedExecZshFork,
            ],
        );
        turn.model_info.shell_type = ConfigShellToolType::ShellCommand;
    })
    .await;

    if codex_utils_pty::conpty_supported() {
        composed.assert_visible_contains(&["exec_command", "write_stdin"]);
        composed.assert_visible_lacks(&["shell_command"]);
        composed.assert_registered_contains(&["exec_command", "write_stdin", "shell_command"]);
        assert_eq!(composed.exposure("shell_command"), ToolExposure::Hidden);
    } else {
        composed.assert_visible_contains(&["shell_command"]);
        composed.assert_visible_lacks(&["exec_command", "write_stdin"]);
    }
}

#[tokio::test]
async fn zsh_fork_unified_exec_hides_shell_parameter() {
    if !codex_utils_pty::conpty_supported() {
        return;
    }

    let plan = probe(|turn| {
        set_features(
            turn,
            &[
                Feature::ShellTool,
                Feature::UnifiedExec,
                Feature::ShellZshFork,
                Feature::UnifiedExecZshFork,
            ],
        );
        turn.unified_exec_shell_mode =
            codex_tools::UnifiedExecShellMode::ZshFork(zsh_fork_config_for_spec_plan_tests());
    })
    .await;

    plan.assert_visible_contains(&["exec_command", "write_stdin"]);
    assert!(!has_parameter(plan.visible_spec("exec_command"), "shell"));
}

#[tokio::test]
async fn zsh_fork_unified_exec_keeps_shell_parameter_when_remote_environment_available() {
    if !codex_utils_pty::conpty_supported() {
        return;
    }

    let plan = probe(|turn| {
        set_features(
            turn,
            &[
                Feature::ShellTool,
                Feature::UnifiedExec,
                Feature::ShellZshFork,
                Feature::UnifiedExecZshFork,
            ],
        );
        turn.unified_exec_shell_mode =
            codex_tools::UnifiedExecShellMode::ZshFork(zsh_fork_config_for_spec_plan_tests());
        let remote_cwd = turn
            .environments
            .primary()
            .expect("primary environment")
            .cwd
            .clone();
        turn.environments
            .turn_environments
            .push(crate::session::turn_context::TurnEnvironment {
                environment_id: "remote".to_string(),
                environment: Arc::new(
                    codex_exec_server::Environment::create_for_tests(Some(
                        "ws://127.0.0.1:1/remote-exec-server".to_string(),
                    ))
                    .expect("remote test environment"),
                ),
                cwd: remote_cwd,
                shell: None,
            });
    })
    .await;

    plan.assert_visible_contains(&["exec_command", "write_stdin"]);
    assert!(has_parameter(plan.visible_spec("exec_command"), "shell"));
    assert!(has_parameter(
        plan.visible_spec("exec_command"),
        "environment_id"
    ));
}

#[tokio::test]
async fn environment_count_controls_environment_backed_tools() {
    let no_environment = probe(|turn| {
        turn.environments.turn_environments.clear();
        set_feature(turn, Feature::ShellTool, /*enabled*/ true);
        turn.model_info.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
    })
    .await;
    no_environment.assert_visible_lacks(&[
        "shell_command",
        "exec_command",
        "apply_patch",
        "view_image",
    ]);
    no_environment.assert_registered_lacks(&[
        "shell_command",
        "exec_command",
        "apply_patch",
        "view_image",
    ]);

    let multiple_environments = probe(|turn| {
        duplicate_primary_environment(turn);
        set_feature(turn, Feature::ShellTool, /*enabled*/ true);
        set_feature(turn, Feature::UnifiedExec, /*enabled*/ true);
        turn.model_info.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
    })
    .await;
    multiple_environments.assert_visible_contains(&["exec_command", "apply_patch", "view_image"]);
    assert!(has_parameter(
        multiple_environments.visible_spec("exec_command"),
        "environment_id"
    ));
    assert!(apply_patch_accepts_environment_id(
        multiple_environments.visible_spec("apply_patch")
    ));
    assert!(has_parameter(
        multiple_environments.visible_spec("view_image"),
        "environment_id"
    ));
}

#[tokio::test]
async fn host_context_gates_goal_and_agent_job_tools() {
    let feature_disabled = probe(|turn| {
        set_feature(turn, Feature::Goals, /*enabled*/ false);
        turn.goal_tools_supported = true;
    })
    .await;
    feature_disabled.assert_visible_lacks(&["get_goal", "create_goal", "update_goal"]);

    let host_disabled = probe(|turn| {
        set_feature(turn, Feature::Goals, /*enabled*/ true);
        turn.goal_tools_supported = false;
    })
    .await;
    host_disabled.assert_visible_lacks(&["get_goal", "create_goal", "update_goal"]);

    let enabled = probe(|turn| {
        set_feature(turn, Feature::Goals, /*enabled*/ true);
        turn.goal_tools_supported = true;
    })
    .await;
    enabled.assert_visible_contains(&["get_goal", "create_goal", "update_goal"]);

    let review_thread = probe(|turn| {
        set_feature(turn, Feature::Goals, /*enabled*/ true);
        turn.goal_tools_supported = true;
        turn.session_source = SessionSource::SubAgent(SubAgentSource::Review);
    })
    .await;
    review_thread.assert_visible_lacks(&["get_goal", "create_goal", "update_goal"]);

    let normal_agent_job = probe(|turn| {
        set_feature(turn, Feature::SpawnCsv, /*enabled*/ true);
    })
    .await;
    normal_agent_job.assert_visible_contains(&["spawn_agents_on_csv"]);
    normal_agent_job.assert_visible_lacks(&["report_agent_job_result"]);

    let worker_agent_job = probe(|turn| {
        set_feature(turn, Feature::SpawnCsv, /*enabled*/ true);
        turn.session_source =
            SessionSource::SubAgent(SubAgentSource::Other("agent_job:42".to_string()));
    })
    .await;
    worker_agent_job.assert_visible_contains(&["spawn_agents_on_csv", "report_agent_job_result"]);
}

#[tokio::test]
async fn mcp_and_tool_search_follow_direct_and_deferred_tool_exposure() {
    let direct_mcp = probe_with(
        |_| {},
        ToolPlanInputs {
            mcp_tools: Some(vec![mcp_tool("direct", "mcp__direct", "lookup")]),
            ..ToolPlanInputs::default()
        },
    )
    .await;
    direct_mcp.assert_visible_contains(&[
        "list_mcp_resources",
        "list_mcp_resource_templates",
        "read_mcp_resource",
    ]);
    assert_eq!(
        direct_mcp.namespace_function_names("mcp__direct"),
        &["lookup".to_string()]
    );

    let searchable_mcp = ToolPlanInputs {
        deferred_mcp_tools: Some(vec![mcp_tool("searchable", "mcp__searchable", "lookup")]),
        ..ToolPlanInputs::default()
    };

    let missing_model_capability = probe_with(
        |turn| {
            turn.model_info.supports_search_tool = false;
        },
        ToolPlanInputs {
            deferred_mcp_tools: searchable_mcp.deferred_mcp_tools.clone(),
            ..ToolPlanInputs::default()
        },
    )
    .await;
    missing_model_capability.assert_visible_lacks(&["tool_search"]);

    let missing_deferred_tools = probe(|turn| {
        set_feature(turn, Feature::Collab, /*enabled*/ false);
        turn.model_info.supports_search_tool = true;
    })
    .await;
    missing_deferred_tools.assert_visible_lacks(&["tool_search"]);
    missing_deferred_tools.assert_visible_lacks(&[
        "list_mcp_resources",
        "list_mcp_resource_templates",
        "read_mcp_resource",
    ]);

    let bedrock_namespace_capability = probe_with(
        |turn| {
            turn.model_info.supports_search_tool = true;
            use_bedrock_provider(turn);
        },
        ToolPlanInputs {
            deferred_mcp_tools: searchable_mcp.deferred_mcp_tools.clone(),
            ..ToolPlanInputs::default()
        },
    )
    .await;
    bedrock_namespace_capability.assert_visible_contains(&["tool_search"]);

    let enabled = probe_with(
        |turn| {
            turn.model_info.supports_search_tool = true;
        },
        searchable_mcp,
    )
    .await;
    enabled.assert_visible_contains(&["tool_search"]);
    enabled.assert_registered_contains(&[
        "tool_search",
        &ToolName::namespaced("mcp__searchable", "lookup").to_string(),
    ]);
}

#[tokio::test]
async fn deferred_extension_tools_are_discoverable_with_tool_search() {
    let plan = probe_with(
        |turn| {
            turn.model_info.supports_search_tool = true;
        },
        ToolPlanInputs {
            extension_tool_executors: vec![Arc::new(DeferredExtensionTool)],
            ..ToolPlanInputs::default()
        },
    )
    .await;

    plan.assert_visible_contains(&["tool_search"]);
    plan.assert_visible_lacks(&["extension_echo"]);
    plan.assert_registered_contains(&["extension_echo"]);
    assert_eq!(plan.exposure("extension_echo"), ToolExposure::Deferred);
}

#[tokio::test]
async fn invalid_mcp_tools_are_not_registered() {
    let plan = probe_with(
        |_| {},
        ToolPlanInputs {
            mcp_tools: Some(vec![invalid_mcp_tool("invalid", "mcp__invalid", "lookup")]),
            ..ToolPlanInputs::default()
        },
    )
    .await;

    plan.assert_visible_lacks(&["mcp__invalid"]);
    plan.assert_registered_lacks(&[&ToolName::namespaced("mcp__invalid", "lookup").to_string()]);
}

#[tokio::test]
async fn request_plugin_install_requires_all_discovery_features_and_discoverable_tools() {
    let discoverable_tools = Some(vec![discoverable_plugin("github", "GitHub")]);
    for disabled_feature in [Feature::ToolSuggest, Feature::Apps, Feature::Plugins] {
        let plan = probe_with(
            |turn| {
                set_features(
                    turn,
                    &[Feature::ToolSuggest, Feature::Apps, Feature::Plugins],
                );
                set_feature(turn, disabled_feature, /*enabled*/ false);
            },
            ToolPlanInputs {
                discoverable_tools: discoverable_tools.clone(),
                ..ToolPlanInputs::default()
            },
        )
        .await;
        plan.assert_visible_lacks(&[
            "list_available_plugins_to_install",
            "request_plugin_install",
        ]);
    }

    let no_candidates = probe(|turn| {
        set_features(
            turn,
            &[Feature::ToolSuggest, Feature::Apps, Feature::Plugins],
        );
    })
    .await;
    no_candidates.assert_visible_lacks(&[
        "list_available_plugins_to_install",
        "request_plugin_install",
    ]);

    let enabled = probe_with(
        |turn| {
            set_features(
                turn,
                &[Feature::ToolSuggest, Feature::Apps, Feature::Plugins],
            );
        },
        ToolPlanInputs {
            discoverable_tools,
            ..ToolPlanInputs::default()
        },
    )
    .await;
    enabled.assert_visible_contains(&[
        "list_available_plugins_to_install",
        "request_plugin_install",
    ]);
}

#[tokio::test]
async fn install_suggestion_tools_stay_visible_without_tool_search() {
    let plan = probe_with(
        |turn| {
            turn.model_info.supports_search_tool = false;
            set_features(
                turn,
                &[Feature::ToolSuggest, Feature::Apps, Feature::Plugins],
            );
        },
        ToolPlanInputs {
            discoverable_tools: Some(vec![discoverable_plugin("github", "GitHub")]),
            ..ToolPlanInputs::default()
        },
    )
    .await;

    plan.assert_visible_contains(&[
        "list_available_plugins_to_install",
        "request_plugin_install",
    ]);
    plan.assert_visible_lacks(&["tool_search"]);
}

#[tokio::test]
async fn request_plugin_install_description_defers_inventory_to_list_tool() {
    let plan = probe_with(
        |turn| {
            set_features(
                turn,
                &[Feature::ToolSuggest, Feature::Apps, Feature::Plugins],
            );
        },
        ToolPlanInputs {
            discoverable_tools: Some(vec![discoverable_plugin("github", "GitHub")]),
            ..ToolPlanInputs::default()
        },
    )
    .await;

    let ToolSpec::Function(ResponsesApiTool {
        description: list_description,
        ..
    }) = plan.visible_spec("list_available_plugins_to_install")
    else {
        panic!("expected list_available_plugins_to_install function spec");
    };
    assert!(list_description.contains(
        "Returns known plugins and connectors that can be passed to `request_plugin_install`."
    ));

    let ToolSpec::Function(ResponsesApiTool {
        description: request_description,
        ..
    }) = plan.visible_spec("request_plugin_install")
    else {
        panic!("expected request_plugin_install function spec");
    };
    assert!(request_description.contains(
        "Use this tool only after `list_available_plugins_to_install` returns a plugin or connector that exactly matches the user's explicit request."
    ));
    assert!(!request_description.contains("github"));
}

#[tokio::test]
async fn code_mode_only_exposes_code_executor_and_hides_nested_tools() {
    let input = ToolPlanInputs {
        dynamic_tools: vec![dynamic_tool(
            Some("codex_app"),
            "lookup",
            /*defer_loading*/ false,
        )],
        ..ToolPlanInputs::default()
    };
    let plain = probe_with(|_| {}, input).await;
    assert_eq!(
        plain.namespace_function_names("codex_app"),
        &["lookup".to_string()]
    );
    plain.assert_visible_lacks(&[
        codex_code_mode::PUBLIC_TOOL_NAME,
        codex_code_mode::WAIT_TOOL_NAME,
    ]);

    let code_mode_only = probe_with(
        |turn| {
            set_features(turn, &[Feature::CodeMode, Feature::CodeModeOnly]);
        },
        ToolPlanInputs {
            dynamic_tools: vec![dynamic_tool(
                Some("codex_app"),
                "lookup",
                /*defer_loading*/ false,
            )],
            ..ToolPlanInputs::default()
        },
    )
    .await;
    code_mode_only.assert_visible_contains(&[
        codex_code_mode::PUBLIC_TOOL_NAME,
        codex_code_mode::WAIT_TOOL_NAME,
    ]);
    assert_eq!(
        code_mode_only.namespace_function_names("codex_app"),
        Vec::<String>::new().as_slice()
    );
}

#[tokio::test]
async fn multi_agent_feature_selects_one_agent_tool_family() {
    let v1 = probe(|turn| {
        set_feature(turn, Feature::Collab, /*enabled*/ true);
        set_feature(turn, Feature::MultiAgentV2, /*enabled*/ false);
    })
    .await;
    v1.assert_visible_contains(&[MULTI_AGENT_V1_NAMESPACE]);
    v1.assert_visible_lacks(&[
        "spawn_agent",
        "send_input",
        "resume_agent",
        "wait_agent",
        "close_agent",
        "send_message",
        "followup_task",
        "assign_task",
        "list_agents",
    ]);
    assert_eq!(
        v1.namespace_function_names(MULTI_AGENT_V1_NAMESPACE),
        &[
            "close_agent".to_string(),
            "resume_agent".to_string(),
            "send_input".to_string(),
            "spawn_agent".to_string(),
            "wait_agent".to_string(),
        ]
    );

    let v2 = probe(|turn| {
        set_feature(turn, Feature::MultiAgentV2, /*enabled*/ true);
        update_config(turn, |config| {
            config.multi_agent_v2.max_concurrent_threads_per_session = 17;
        });
    })
    .await;
    v2.assert_visible_contains(&[
        "spawn_agent",
        "send_message",
        "followup_task",
        "wait_agent",
        "close_agent",
        "list_agents",
    ]);
    v2.assert_visible_lacks(&["send_input", "resume_agent", "assign_task"]);
    let spawn_agent_description = match v2.visible_spec("spawn_agent") {
        ToolSpec::Function(tool) => tool.description.as_str(),
        other => panic!("expected spawn_agent function spec, got {other:?}"),
    };
    assert!(spawn_agent_description.contains("max_concurrent_threads_per_session = 17"));

    let direct_model_only = probe(|turn| {
        set_features(
            turn,
            &[
                Feature::CodeMode,
                Feature::CodeModeOnly,
                Feature::MultiAgentV2,
            ],
        );
        update_config(turn, |config| {
            config.multi_agent_v2.non_code_mode_only = true;
        });
    })
    .await;
    direct_model_only.assert_visible_contains(&["spawn_agent", "send_message", "wait_agent"]);
    assert_eq!(
        direct_model_only.exposure("spawn_agent"),
        ToolExposure::DirectModelOnly
    );
}

#[tokio::test]
async fn tool_mode_selector_overrides_feature_flags() {
    let direct = probe(|turn| {
        set_features(turn, &[Feature::CodeMode, Feature::CodeModeOnly]);
        turn.model_info.tool_mode = Some(ToolMode::Direct);
        turn.tool_mode = ToolMode::Direct;
    })
    .await;
    direct.assert_visible_lacks(&[
        codex_code_mode::PUBLIC_TOOL_NAME,
        codex_code_mode::WAIT_TOOL_NAME,
    ]);
}

#[tokio::test]
async fn v1_multi_agent_tools_defer_when_tool_search_available() {
    let plan = probe(|turn| {
        turn.model_info.supports_search_tool = true;
        set_feature(turn, Feature::Collab, /*enabled*/ true);
        set_feature(turn, Feature::MultiAgentV2, /*enabled*/ false);
    })
    .await;

    plan.assert_visible_contains(&["tool_search"]);
    plan.assert_visible_lacks(&[
        "spawn_agent",
        "send_input",
        "resume_agent",
        "wait_agent",
        "close_agent",
    ]);
    for tool_name in [
        "spawn_agent",
        "send_input",
        "resume_agent",
        "wait_agent",
        "close_agent",
    ] {
        let namespaced_tool_name = ToolName::namespaced(MULTI_AGENT_V1_NAMESPACE, tool_name);
        let namespaced_tool_name = namespaced_tool_name.to_string();
        assert!(
            plan.registered_names.contains(&namespaced_tool_name),
            "expected namespaced runtime for {tool_name}"
        );
        assert!(
            !plan
                .registered_names
                .contains(&ToolName::plain(tool_name).to_string()),
            "expected no plain runtime for deferred {tool_name}"
        );
        assert_eq!(plan.exposure(&namespaced_tool_name), ToolExposure::Deferred);
    }
    let ToolSpec::ToolSearch { description, .. } = plan.visible_spec("tool_search") else {
        panic!("expected visible tool_search spec");
    };
    assert!(description.contains("- Multi-agent tools: Spawn and manage sub-agents."));
}

#[tokio::test]
async fn multi_agent_v2_can_use_configured_tool_namespace() {
    let namespaced = probe(|turn| {
        set_feature(turn, Feature::MultiAgentV2, /*enabled*/ true);
        update_config(turn, |config| {
            config.multi_agent_v2.tool_namespace = Some("agents".to_string());
        });
    })
    .await;

    namespaced.assert_visible_contains(&["agents"]);
    namespaced.assert_visible_lacks(&["assign_task"]);
    assert!(
        !namespaced
            .registered_names
            .contains(&ToolName::namespaced("agents", "assign_task").to_string()),
        "expected no namespaced runtime for assign_task"
    );
    assert!(
        !namespaced
            .namespace_function_names("agents")
            .iter()
            .any(|name| name == "assign_task"),
        "expected assign_task to be absent from agents namespace"
    );
    for tool_name in [
        "spawn_agent",
        "send_message",
        "followup_task",
        "wait_agent",
        "close_agent",
        "list_agents",
    ] {
        namespaced.assert_visible_lacks(&[tool_name]);
        assert!(
            namespaced
                .registered_names
                .contains(&ToolName::namespaced("agents", tool_name).to_string()),
            "expected namespaced runtime for {tool_name}"
        );
        assert!(
            !namespaced
                .registered_names
                .contains(&ToolName::plain(tool_name).to_string()),
            "expected no plain runtime for {tool_name}"
        );
        assert!(
            namespaced
                .namespace_function_names("agents")
                .iter()
                .any(|name| name == tool_name),
            "expected {tool_name} in agents namespace"
        );
    }
}

#[tokio::test]
async fn multi_agent_v2_namespace_is_supported_by_bedrock_provider() {
    let plan = probe(|turn| {
        set_feature(turn, Feature::MultiAgentV2, /*enabled*/ true);
        update_config(turn, |config| {
            config.multi_agent_v2.tool_namespace = Some("agents".to_string());
        });
        use_bedrock_provider(turn);
    })
    .await;

    plan.assert_visible_contains(&["agents"]);
    plan.assert_visible_lacks(&["spawn_agent", "send_message", "list_agents"]);
    assert!(
        !plan
            .registered_names
            .contains(&ToolName::plain("spawn_agent").to_string())
    );
    assert!(
        plan.registered_names
            .contains(&ToolName::namespaced("agents", "spawn_agent").to_string())
    );
}

#[tokio::test]
async fn code_mode_only_can_expose_namespaced_multi_agent_v2_as_normal_tools() {
    let plan = probe(|turn| {
        set_features(
            turn,
            &[
                Feature::CodeMode,
                Feature::CodeModeOnly,
                Feature::MultiAgentV2,
            ],
        );
        update_config(turn, |config| {
            config.multi_agent_v2.non_code_mode_only = true;
            config.multi_agent_v2.tool_namespace = Some("agents".to_string());
        });
    })
    .await;

    assert_eq!(
        plan.visible_names,
        vec![
            "exec",
            "wait",
            "agents",
            // Hosted Responses tools.
            "web_search",
        ]
    );
    assert!(
        !plan
            .namespace_function_names("agents")
            .iter()
            .any(|name| name == "assign_task"),
        "expected assign_task to be absent from agents namespace"
    );
    for tool_name in [
        "spawn_agent",
        "send_message",
        "followup_task",
        "wait_agent",
        "close_agent",
        "list_agents",
    ] {
        assert!(
            plan.namespace_function_names("agents")
                .iter()
                .any(|name| name == tool_name),
            "expected {tool_name} in agents namespace"
        );
    }
}

#[tokio::test]
async fn hosted_tools_follow_provider_auth_model_and_config_gates() {
    let api_key_auth = probe(|turn| {
        set_feature(turn, Feature::ImageGeneration, /*enabled*/ true);
        turn.model_info.input_modalities = vec![InputModality::Image];
    })
    .await;
    api_key_auth.assert_visible_lacks(&["image_generation"]);

    let image_generation = probe(|turn| {
        use_chatgpt_auth(turn);
        set_feature(turn, Feature::ImageGeneration, /*enabled*/ true);
        turn.model_info.input_modalities = vec![InputModality::Image];
    })
    .await;
    image_generation.assert_visible_contains(&["image_generation"]);

    let extension_flag_without_imagegen_tool = probe(|turn| {
        use_chatgpt_auth(turn);
        set_feature(turn, Feature::ImageGeneration, /*enabled*/ true);
        set_feature(turn, Feature::ImageGenExt, /*enabled*/ true);
        turn.model_info.input_modalities = vec![InputModality::Image];
    })
    .await;
    extension_flag_without_imagegen_tool.assert_visible_contains(&["image_generation"]);
    extension_flag_without_imagegen_tool.assert_visible_lacks(&["image_gen"]);

    let live_web_search = probe(|turn| {
        set_web_search_mode(turn, WebSearchMode::Live);
        turn.model_info.web_search_tool_type = WebSearchToolType::TextAndImage;
    })
    .await;
    assert_eq!(
        live_web_search.visible_spec("web_search"),
        &ToolSpec::WebSearch {
            external_web_access: Some(true),
            filters: None,
            user_location: None,
            search_context_size: None,
            search_content_types: Some(vec!["text".to_string(), "image".to_string()]),
        }
    );

    let code_mode_only = probe(|turn| {
        use_chatgpt_auth(turn);
        set_features(turn, &[Feature::CodeModeOnly, Feature::MultiAgentV2]);
        set_web_search_mode(turn, WebSearchMode::Live);
        turn.model_info.input_modalities = vec![InputModality::Image];
    })
    .await;
    assert_eq!(
        code_mode_only.visible_names,
        vec![
            // Code-mode entrypoints.
            codex_code_mode::PUBLIC_TOOL_NAME,
            codex_code_mode::WAIT_TOOL_NAME,
            // Multi-agent v2 tools.
            "spawn_agent",
            "send_message",
            "followup_task",
            "wait_agent",
            "close_agent",
            "list_agents",
            // Hosted Responses tools.
            "web_search",
            "image_generation",
        ]
    );

    let standalone_web_search_without_web_run = probe(|turn| {
        set_feature(turn, Feature::StandaloneWebSearch, /*enabled*/ true);
        set_web_search_mode(turn, WebSearchMode::Live);
    })
    .await;
    standalone_web_search_without_web_run.assert_visible_contains(&["web_search"]);

    let standalone_web_search = probe_with(
        |turn| {
            set_feature(turn, Feature::StandaloneWebSearch, /*enabled*/ true);
            set_web_search_mode(turn, WebSearchMode::Live);
        },
        ToolPlanInputs {
            extension_tool_executors: vec![Arc::new(WebRunExtensionTool)],
            ..Default::default()
        },
    )
    .await;
    standalone_web_search.assert_visible_lacks(&["web_search"]);

    let unsupported_provider = probe(|turn| {
        set_web_search_mode(turn, WebSearchMode::Live);
        use_bedrock_provider(turn);
    })
    .await;
    unsupported_provider.assert_visible_lacks(&["web_search"]);
}
