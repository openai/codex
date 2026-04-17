//! Core-independent adapter for constructing Codex tool registry specs.

use codex_mcp::ToolInfo;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tools::AdditionalProperties;
use codex_tools::ConfiguredToolSpec;
use codex_tools::DiscoverableTool;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolNamespace;
use codex_tools::ToolRegistryPlan;
use codex_tools::ToolRegistryPlanDeferredTool;
use codex_tools::ToolRegistryPlanMcpTool;
use codex_tools::ToolRegistryPlanParams;
use codex_tools::ToolSpec;
use codex_tools::ToolsConfig;
use codex_tools::WaitAgentTimeoutOptions;
use codex_tools::augment_tool_spec_for_code_mode;
use codex_tools::build_tool_registry_plan;
use std::collections::HashMap;
use std::collections::HashSet;

pub struct ToolSpecPlanParams<'a> {
    pub mcp_tools: Option<&'a HashMap<String, ToolInfo>>,
    pub deferred_mcp_tools: Option<&'a HashMap<String, ToolInfo>>,
    pub unavailable_called_tools: Vec<ToolName>,
    pub discoverable_tools: Option<&'a [DiscoverableTool]>,
    pub dynamic_tools: &'a [DynamicToolSpec],
    pub default_agent_type_description: &'a str,
    pub wait_agent_timeouts: WaitAgentTimeoutOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpecPlan {
    pub registry_plan: ToolRegistryPlan,
    pub unavailable_called_tools: Vec<ToolName>,
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
                        description: tool
                            .connector_description
                            .clone()
                            .or_else(|| tool.server_instructions.clone()),
                    },
                )
            })
            .collect(),
    }
}

fn map_deferred_mcp_tools_for_plan(
    deferred_mcp_tools: &HashMap<String, ToolInfo>,
) -> Vec<ToolRegistryPlanDeferredTool<'_>> {
    deferred_mcp_tools
        .values()
        .map(|tool| ToolRegistryPlanDeferredTool {
            name: tool.canonical_tool_name(),
            server_name: tool.server_name.as_str(),
            connector_name: tool.connector_name.as_deref(),
            connector_description: tool.connector_description.as_deref(),
        })
        .collect()
}

pub fn build_tool_spec_plan(config: &ToolsConfig, params: ToolSpecPlanParams<'_>) -> ToolSpecPlan {
    let mcp_tool_plan_inputs = params.mcp_tools.map(map_mcp_tools_for_plan);
    let deferred_mcp_tool_sources = params
        .deferred_mcp_tools
        .map(map_deferred_mcp_tools_for_plan);
    let mut registry_plan = build_tool_registry_plan(
        config,
        ToolRegistryPlanParams {
            mcp_tools: mcp_tool_plan_inputs
                .as_ref()
                .map(|inputs| inputs.mcp_tools.as_slice()),
            deferred_mcp_tools: deferred_mcp_tool_sources.as_deref(),
            tool_namespaces: mcp_tool_plan_inputs
                .as_ref()
                .map(|inputs| &inputs.tool_namespaces),
            discoverable_tools: params.discoverable_tools,
            dynamic_tools: params.dynamic_tools,
            default_agent_type_description: params.default_agent_type_description,
            wait_agent_timeouts: params.wait_agent_timeouts,
        },
    );
    let mut existing_spec_names = registry_plan
        .specs
        .iter()
        .map(|configured_tool| configured_tool.name().to_string())
        .collect::<HashSet<_>>();
    let mut unavailable_called_tools = Vec::new();

    for unavailable_tool in params.unavailable_called_tools {
        let tool_name = unavailable_tool.display();
        if existing_spec_names.insert(tool_name.clone()) {
            let spec = ToolSpec::Function(ResponsesApiTool {
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
            registry_plan.specs.push(ConfiguredToolSpec::new(
                spec, /*supports_parallel_tool_calls*/ false,
            ));
        }
        unavailable_called_tools.push(unavailable_tool);
    }

    ToolSpecPlan {
        registry_plan,
        unavailable_called_tools,
    }
}

fn unavailable_tool_message(tool_name: impl std::fmt::Display, next_step: &str) -> String {
    format!(
        "Tool `{tool_name}` is not currently available. It appeared in earlier tool calls in this conversation, but its implementation is not available in the current request. {next_step}"
    )
}
