use crate::config::DEFAULT_MULTI_AGENT_V2_DEFAULT_WAIT_TIMEOUT_MS;
use crate::config::DEFAULT_MULTI_AGENT_V2_MAX_WAIT_TIMEOUT_MS;
use crate::config::DEFAULT_MULTI_AGENT_V2_MIN_WAIT_TIMEOUT_MS;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::tools::handlers::multi_agents_common::DEFAULT_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MAX_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_common::MIN_WAIT_TIMEOUT_MS;
use crate::tools::handlers::multi_agents_spec::WaitAgentTimeoutOptions;
use crate::tools::registry::ToolRegistryBuilder;
use crate::tools::spec_plan::build_tool_registry_builder;
use crate::tools::spec_plan_types::ToolRegistryBuildParams;
use codex_extension_api::ExtensionToolExecutor;
use codex_mcp::ToolInfo;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tools::DiscoverableTool;
use codex_tools::ToolUserShellType;
use codex_tools::ToolsConfig;
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

pub(crate) fn build_specs_with_discoverable_tools(
    config: &ToolsConfig,
    mcp_tools: Option<Vec<ToolInfo>>,
    deferred_mcp_tools: Option<Vec<ToolInfo>>,
    discoverable_tools: Option<Vec<DiscoverableTool>>,
    extension_tool_executors: &[Arc<dyn ExtensionToolExecutor>],
    dynamic_tools: &[DynamicToolSpec],
) -> ToolRegistryBuilder {
    let default_agent_type_description =
        crate::agent::role::spawn_tool_spec::build(&std::collections::BTreeMap::new());
    let (min_wait_timeout_ms, max_wait_timeout_ms, default_wait_timeout_ms) =
        if config.multi_agent_v2 {
            let min_wait_timeout_ms = config
                .wait_agent_min_timeout_ms
                .unwrap_or(DEFAULT_MULTI_AGENT_V2_MIN_WAIT_TIMEOUT_MS);
            let max_wait_timeout_ms = config
                .wait_agent_max_timeout_ms
                .unwrap_or(DEFAULT_MULTI_AGENT_V2_MAX_WAIT_TIMEOUT_MS);
            let default_wait_timeout_ms = config
                .wait_agent_default_timeout_ms
                .unwrap_or(DEFAULT_MULTI_AGENT_V2_DEFAULT_WAIT_TIMEOUT_MS);
            (
                min_wait_timeout_ms,
                max_wait_timeout_ms,
                default_wait_timeout_ms,
            )
        } else {
            (
                MIN_WAIT_TIMEOUT_MS,
                MAX_WAIT_TIMEOUT_MS,
                DEFAULT_WAIT_TIMEOUT_MS,
            )
        };
    build_tool_registry_builder(
        config,
        ToolRegistryBuildParams {
            mcp_tools: mcp_tools.as_deref(),
            deferred_mcp_tools: deferred_mcp_tools.as_deref(),
            discoverable_tools: discoverable_tools.as_deref(),
            extension_tool_executors,
            dynamic_tools,
            default_agent_type_description: &default_agent_type_description,
            wait_agent_timeouts: WaitAgentTimeoutOptions {
                default_timeout_ms: default_wait_timeout_ms,
                min_timeout_ms: min_wait_timeout_ms,
                max_timeout_ms: max_wait_timeout_ms,
            },
        },
    )
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
