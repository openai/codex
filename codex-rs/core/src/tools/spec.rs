use crate::shell::Shell;
use crate::shell::ShellType;
use crate::tools::flat_tool_name;
use crate::tools::handlers::multi_agents_common::DEFAULT_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MAX_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MIN_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_spec::WaitAgentTimeoutOptions;
use crate::tools::registry::ToolRegistryBuilder;
use crate::tools::spec_plan::build_tool_registry_builder;
use crate::tools::spec_plan_types::ToolNamespace;
use crate::tools::spec_plan_types::ToolRegistryBuildDeferredTool;
use crate::tools::spec_plan_types::ToolRegistryBuildMcpTool;
use crate::tools::spec_plan_types::ToolRegistryBuildParams;
use codex_mcp::ToolInfo;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tool_api::ToolBundle as ExtensionToolBundle;
use codex_tools::AdditionalProperties;
use codex_tools::DiscoverableTool;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolUserShellType;
use codex_tools::ToolsConfig;
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
    mcp_tools: Vec<ToolRegistryBuildMcpTool<'a>>,
    tool_namespaces: HashMap<String, ToolNamespace>,
}

fn map_mcp_tools_for_plan(mcp_tools: &[ToolInfo]) -> McpToolPlanInputs<'_> {
    McpToolPlanInputs {
        mcp_tools: mcp_tools
            .iter()
            .map(|tool| ToolRegistryBuildMcpTool {
                name: tool.canonical_tool_name(),
                tool: &tool.tool,
            })
            .collect(),
        tool_namespaces: mcp_tools
            .iter()
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
    mcp_tools: Option<Vec<ToolInfo>>,
    deferred_mcp_tools: Option<Vec<ToolInfo>>,
    unavailable_called_tools: Vec<ToolName>,
    discoverable_tools: Option<Vec<DiscoverableTool>>,
    extension_tool_bundles: &[ExtensionToolBundle],
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    use crate::tools::handlers::UnavailableToolHandler;
    use crate::tools::handlers::unavailable_tool_message;
    use crate::tools::tool_search_entry::build_tool_search_entries_for_config;

    let mcp_tool_plan_inputs = mcp_tools.as_deref().map(map_mcp_tools_for_plan);
    let deferred_mcp_tool_sources = deferred_mcp_tools.as_ref().map(|tools| {
        tools
            .iter()
            .map(|tool| ToolRegistryBuildDeferredTool {
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
    let deferred_dynamic_tools = dynamic_tools
        .iter()
        .filter(|tool| tool.defer_loading && (config.namespace_tools || tool.namespace.is_none()))
        .cloned()
        .collect::<Vec<_>>();
    let tool_search_entries = build_tool_search_entries_for_config(
        config,
        deferred_mcp_tools.as_deref(),
        &deferred_dynamic_tools,
    );
    let mut builder = build_tool_registry_builder(
        config,
        ToolRegistryBuildParams {
            mcp_tools: mcp_tool_plan_inputs
                .as_ref()
                .map(|inputs| inputs.mcp_tools.as_slice()),
            deferred_mcp_tools: deferred_mcp_tool_sources.as_deref(),
            tool_namespaces: mcp_tool_plan_inputs
                .as_ref()
                .map(|inputs| &inputs.tool_namespaces),
            discoverable_tools: discoverable_tools.as_deref(),
            extension_tool_bundles,
            dynamic_tools,
            default_agent_type_description: &default_agent_type_description,
            wait_agent_timeouts: WaitAgentTimeoutOptions {
                default_timeout_ms: default_wait_timeout_ms,
                min_timeout_ms: min_wait_timeout_ms,
                max_timeout_ms: MAX_WAIT_TIMEOUT_MS,
            },
            tool_search_entries: &tool_search_entries,
        },
    );
    let mut existing_spec_names = builder
        .specs()
        .iter()
        .map(|configured_tool| configured_tool.name().to_string())
        .collect::<HashSet<_>>();

    for unavailable_tool in unavailable_called_tools {
        let tool_name = flat_tool_name(&unavailable_tool).into_owned();
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
            builder.register_handler(Arc::new(UnavailableToolHandler::new(
                unavailable_tool,
                spec,
            )));
        } else {
            builder.register_handler(Arc::new(UnavailableToolHandler::without_spec(
                unavailable_tool,
            )));
        }
    }
    builder
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
