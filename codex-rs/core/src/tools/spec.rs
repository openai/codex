use crate::shell::Shell;
use crate::shell::ShellType;
use crate::tools::handlers::agent_jobs::ReportAgentJobResultHandler;
use crate::tools::handlers::agent_jobs::SpawnAgentsOnCsvHandler;
use crate::tools::handlers::multi_agents_common::DEFAULT_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MAX_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MIN_WAIT_TIMEOUT_MS;
use crate::tools::registry::ToolRegistryBuilder;
use codex_mcp::ToolInfo;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tools::AdditionalProperties;
use codex_tools::DiscoverableTool;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolHandlerKind;
use codex_tools::ToolName;
use codex_tools::ToolNamespace;
use codex_tools::ToolRegistryPlanDeferredTool;
use codex_tools::ToolRegistryPlanMcpTool;
use codex_tools::ToolRegistryPlanParams;
use codex_tools::ToolUserShellType;
use codex_tools::ToolsConfig;
use codex_tools::WaitAgentTimeoutOptions;
use codex_tools::augment_tool_spec_for_code_mode;
use codex_tools::build_tool_registry_plan;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

pub(crate) fn tool_user_shell_type(user_shell: &Shell) -> ToolUserShellType {
    match user_shell.shell_type {
        ShellType::Zsh => ToolUserShellType::Zsh,
        ShellType::Bash => ToolUserShellType::Bash,
        ShellType::PowerShell => ToolUserShellType::PowerShell,
        ShellType::Sh => ToolUserShellType::Sh,
        ShellType::Cmd => ToolUserShellType::Cmd,
    }
}

struct McpToolPlanInputs<'a> {
    mcp_tools: Vec<ToolRegistryPlanMcpTool<'a>>,
    tool_namespaces: HashMap<String, ToolNamespace>,
}

fn map_mcp_tools_for_plan(mcp_tools: &HashMap<String, ToolInfo>) -> McpToolPlanInputs<'_> {
    McpToolPlanInputs {
        mcp_tools: mcp_tools
            .values()
            .map(|tool| ToolRegistryPlanMcpTool {
                name: tool.canonical_tool_name(),
                tool: &tool.tool,
            })
            .collect(),
        tool_namespaces: mcp_tools
            .values()
            .map(|tool| {
                (
                    tool.callable_namespace.clone(),
                    ToolNamespace {
                        name: tool.callable_namespace.clone(),
                        description: tool.namespace_description.clone(),
                    },
                )
            })
            .collect(),
    }
}

pub(crate) fn build_specs_with_discoverable_tools(
    config: &ToolsConfig,
    mcp_tools: Option<HashMap<String, ToolInfo>>,
    deferred_mcp_tools: Option<HashMap<String, ToolInfo>>,
    unavailable_called_tools: Vec<ToolName>,
    discoverable_tools: Option<Vec<DiscoverableTool>>,
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    use crate::tools::handlers::ApplyPatchHandler;
    use crate::tools::handlers::CodeModeExecuteHandler;
    use crate::tools::handlers::CodeModeWaitHandler;
    use crate::tools::handlers::ContainerExecHandler;
    use crate::tools::handlers::CreateGoalHandler;
    use crate::tools::handlers::DynamicToolHandler;
    use crate::tools::handlers::ExecCommandHandler;
    use crate::tools::handlers::GetGoalHandler;
    use crate::tools::handlers::ListMcpResourceTemplatesHandler;
    use crate::tools::handlers::ListMcpResourcesHandler;
    use crate::tools::handlers::LocalShellHandler;
    use crate::tools::handlers::McpHandler;
    use crate::tools::handlers::PlanHandler;
    use crate::tools::handlers::ReadMcpResourceHandler;
    use crate::tools::handlers::RequestPermissionsHandler;
    use crate::tools::handlers::RequestPluginInstallHandler;
    use crate::tools::handlers::RequestUserInputHandler;
    use crate::tools::handlers::ShellCommandHandler;
    use crate::tools::handlers::ShellHandler;
    use crate::tools::handlers::TestSyncHandler;
    use crate::tools::handlers::ToolSearchHandler;
    use crate::tools::handlers::UnavailableToolHandler;
    use crate::tools::handlers::UpdateGoalHandler;
    use crate::tools::handlers::ViewImageHandler;
    use crate::tools::handlers::WriteStdinHandler;
    use crate::tools::handlers::multi_agents::CloseAgentHandler;
    use crate::tools::handlers::multi_agents::ResumeAgentHandler;
    use crate::tools::handlers::multi_agents::SendInputHandler;
    use crate::tools::handlers::multi_agents::SpawnAgentHandler;
    use crate::tools::handlers::multi_agents::WaitAgentHandler;
    use crate::tools::handlers::multi_agents_v2::CloseAgentHandler as CloseAgentHandlerV2;
    use crate::tools::handlers::multi_agents_v2::FollowupTaskHandler as FollowupTaskHandlerV2;
    use crate::tools::handlers::multi_agents_v2::ListAgentsHandler as ListAgentsHandlerV2;
    use crate::tools::handlers::multi_agents_v2::SendMessageHandler as SendMessageHandlerV2;
    use crate::tools::handlers::multi_agents_v2::SpawnAgentHandler as SpawnAgentHandlerV2;
    use crate::tools::handlers::multi_agents_v2::WaitAgentHandler as WaitAgentHandlerV2;
    use crate::tools::handlers::unavailable_tool_message;
    use crate::tools::tool_search_entry::build_tool_search_entries_for_config;

    let mut builder = ToolRegistryBuilder::new();
    let mcp_tool_plan_inputs = mcp_tools.as_ref().map(map_mcp_tools_for_plan);
    let deferred_mcp_tool_sources = deferred_mcp_tools.as_ref().map(|tools| {
        tools
            .values()
            .map(|tool| ToolRegistryPlanDeferredTool {
                name: tool.canonical_tool_name(),
                server_name: tool.server_name.as_str(),
                connector_name: tool.connector_name.as_deref(),
                description: tool.namespace_description.as_deref(),
            })
            .collect::<Vec<_>>()
    });
    let default_agent_type_description =
        crate::agent::role::spawn_tool_spec::build(&std::collections::BTreeMap::new());
    let min_wait_timeout_ms = if config.multi_agent_v2 {
        config
            .wait_agent_min_timeout_ms
            .unwrap_or(MIN_WAIT_TIMEOUT_MS)
            .clamp(1, MAX_WAIT_TIMEOUT_MS)
    } else {
        MIN_WAIT_TIMEOUT_MS
    };
    let default_wait_timeout_ms =
        DEFAULT_WAIT_TIMEOUT_MS.clamp(min_wait_timeout_ms, MAX_WAIT_TIMEOUT_MS);
    let plan = build_tool_registry_plan(
        config,
        ToolRegistryPlanParams {
            mcp_tools: mcp_tool_plan_inputs
                .as_ref()
                .map(|inputs| inputs.mcp_tools.as_slice()),
            deferred_mcp_tools: deferred_mcp_tool_sources.as_deref(),
            tool_namespaces: mcp_tool_plan_inputs
                .as_ref()
                .map(|inputs| &inputs.tool_namespaces),
            discoverable_tools: discoverable_tools.as_deref(),
            dynamic_tools,
            default_agent_type_description: &default_agent_type_description,
            wait_agent_timeouts: WaitAgentTimeoutOptions {
                default_timeout_ms: default_wait_timeout_ms,
                min_timeout_ms: min_wait_timeout_ms,
                max_timeout_ms: MAX_WAIT_TIMEOUT_MS,
            },
        },
    );
    let deferred_dynamic_tools = dynamic_tools
        .iter()
        .filter(|tool| tool.defer_loading && (config.namespace_tools || tool.namespace.is_none()))
        .cloned()
        .collect::<Vec<_>>();
    let mut existing_spec_names = plan
        .specs
        .iter()
        .map(|configured_tool| configured_tool.name().to_string())
        .collect::<HashSet<_>>();

    for spec in plan.specs {
        if spec.supports_parallel_tool_calls {
            builder.push_spec_with_parallel_support(
                spec.spec, /*supports_parallel_tool_calls*/ true,
            );
        } else {
            builder.push_spec(spec.spec);
        }
    }

    for handler in plan.handlers {
        let name = handler.name;
        match handler.kind {
            ToolHandlerKind::ApplyPatch => {
                builder.register_handler(Arc::new(ApplyPatchHandler));
            }
            ToolHandlerKind::CloseAgentV1 => {
                builder.register_handler(Arc::new(CloseAgentHandler));
            }
            ToolHandlerKind::CloseAgentV2 => {
                builder.register_handler(Arc::new(CloseAgentHandlerV2));
            }
            ToolHandlerKind::CodeModeExecute => {
                builder.register_handler(Arc::new(CodeModeExecuteHandler));
            }
            ToolHandlerKind::CodeModeWait => {
                builder.register_handler(Arc::new(CodeModeWaitHandler));
            }
            ToolHandlerKind::ContainerExec => {
                builder.register_handler(Arc::new(ContainerExecHandler));
            }
            ToolHandlerKind::CreateGoal => {
                builder.register_handler(Arc::new(CreateGoalHandler));
            }
            ToolHandlerKind::DynamicTool => {
                builder.register_handler(Arc::new(DynamicToolHandler::new(name)));
            }
            ToolHandlerKind::ExecCommand => {
                builder.register_handler(Arc::new(ExecCommandHandler));
            }
            ToolHandlerKind::FollowupTaskV2 => {
                builder.register_handler(Arc::new(FollowupTaskHandlerV2));
            }
            ToolHandlerKind::GetGoal => {
                builder.register_handler(Arc::new(GetGoalHandler));
            }
            ToolHandlerKind::ListAgentsV2 => {
                builder.register_handler(Arc::new(ListAgentsHandlerV2));
            }
            ToolHandlerKind::ListMcpResources => {
                builder.register_handler(Arc::new(ListMcpResourcesHandler));
            }
            ToolHandlerKind::ListMcpResourceTemplates => {
                builder.register_handler(Arc::new(ListMcpResourceTemplatesHandler));
            }
            ToolHandlerKind::LocalShell => {
                builder.register_handler(Arc::new(LocalShellHandler));
            }
            ToolHandlerKind::Mcp => {
                builder.register_handler(Arc::new(McpHandler::new(name)));
            }
            ToolHandlerKind::Plan => {
                builder.register_handler(Arc::new(PlanHandler));
            }
            ToolHandlerKind::ReadMcpResource => {
                builder.register_handler(Arc::new(ReadMcpResourceHandler));
            }
            ToolHandlerKind::ReportAgentJobResult => {
                builder.register_handler(Arc::new(ReportAgentJobResultHandler));
            }
            ToolHandlerKind::RequestPermissions => {
                builder.register_handler(Arc::new(RequestPermissionsHandler));
            }
            ToolHandlerKind::RequestUserInput => {
                builder.register_handler(Arc::new(RequestUserInputHandler {
                    available_modes: config.request_user_input_available_modes.clone(),
                }));
            }
            ToolHandlerKind::ResumeAgentV1 => {
                builder.register_handler(Arc::new(ResumeAgentHandler));
            }
            ToolHandlerKind::SendInputV1 => {
                builder.register_handler(Arc::new(SendInputHandler));
            }
            ToolHandlerKind::SendMessageV2 => {
                builder.register_handler(Arc::new(SendMessageHandlerV2));
            }
            ToolHandlerKind::Shell => {
                builder.register_handler(Arc::new(ShellHandler));
            }
            ToolHandlerKind::ShellCommand => {
                builder.register_handler(Arc::new(ShellCommandHandler::from(
                    config.shell_command_backend,
                )));
            }
            ToolHandlerKind::SpawnAgentsOnCsv => {
                builder.register_handler(Arc::new(SpawnAgentsOnCsvHandler));
            }
            ToolHandlerKind::SpawnAgentV1 => {
                builder.register_handler(Arc::new(SpawnAgentHandler));
            }
            ToolHandlerKind::SpawnAgentV2 => {
                builder.register_handler(Arc::new(SpawnAgentHandlerV2));
            }
            ToolHandlerKind::TestSync => {
                builder.register_handler(Arc::new(TestSyncHandler));
            }
            ToolHandlerKind::ToolSearch => {
                let entries = build_tool_search_entries_for_config(
                    config,
                    deferred_mcp_tools.as_ref(),
                    &deferred_dynamic_tools,
                );
                builder.register_handler(Arc::new(ToolSearchHandler::new(entries)));
            }
            ToolHandlerKind::RequestPluginInstall => {
                builder.register_handler(Arc::new(RequestPluginInstallHandler));
            }
            ToolHandlerKind::UpdateGoal => {
                builder.register_handler(Arc::new(UpdateGoalHandler));
            }
            ToolHandlerKind::ViewImage => {
                builder.register_handler(Arc::new(ViewImageHandler));
            }
            ToolHandlerKind::WaitAgentV1 => {
                builder.register_handler(Arc::new(WaitAgentHandler));
            }
            ToolHandlerKind::WaitAgentV2 => {
                builder.register_handler(Arc::new(WaitAgentHandlerV2));
            }
            ToolHandlerKind::WriteStdin => {
                builder.register_handler(Arc::new(WriteStdinHandler));
            }
        }
    }
    if let Some(deferred_mcp_tools) = deferred_mcp_tools.as_ref() {
        for (_, tool) in deferred_mcp_tools.iter().filter(|(name, _)| {
            !mcp_tools
                .as_ref()
                .is_some_and(|tools| tools.contains_key(*name))
        }) {
            builder.register_handler(Arc::new(McpHandler::new(tool.canonical_tool_name())));
        }
    }

    for unavailable_tool in unavailable_called_tools {
        let tool_name = unavailable_tool.display();
        if existing_spec_names.insert(tool_name.clone()) {
            let spec = codex_tools::ToolSpec::Function(ResponsesApiTool {
                name: tool_name.clone(),
                description: unavailable_tool_message(
                    &tool_name,
                    "Calling this placeholder returns an error explaining that the tool is unavailable.",
                ),
                strict: false,
                parameters: JsonSchema::object(
                    Default::default(),
                    /*required*/ None,
                    Some(AdditionalProperties::Boolean(false)),
                ),
                output_schema: None,
                defer_loading: None,
            });
            let spec = if config.code_mode_enabled {
                augment_tool_spec_for_code_mode(spec)
            } else {
                spec
            };
            builder.push_spec(spec);
        }
        builder.register_handler(Arc::new(UnavailableToolHandler::new(unavailable_tool)));
    }
    builder
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
