use crate::tools::handlers::multi_agents_spec::WaitAgentTimeoutOptions;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tools::ConfiguredToolSpec;
use codex_tools::DiscoverableTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_tools::ToolsConfig;
use codex_tools::augment_tool_spec_for_code_mode;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolHandlerKind {
    ApplyPatch,
    CloseAgentV1,
    CloseAgentV2,
    CodeModeExecute,
    CodeModeWait,
    ContainerExec,
    CreateGoal,
    DynamicTool,
    ExecCommand,
    FollowupTaskV2,
    GetGoal,
    ListAgentsV2,
    ListMcpResourceTemplates,
    ListMcpResources,
    LocalShell,
    Mcp,
    Plan,
    ReadMcpResource,
    ReportAgentJobResult,
    RequestPluginInstall,
    RequestPermissions,
    RequestUserInput,
    ResumeAgentV1,
    SendInputV1,
    SendMessageV2,
    Shell,
    ShellCommand,
    SpawnAgentsOnCsv,
    SpawnAgentV1,
    SpawnAgentV2,
    TestSync,
    ToolSearch,
    UpdateGoal,
    ViewImage,
    WaitAgentV1,
    WaitAgentV2,
    WriteStdin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolHandlerSpec {
    pub name: ToolName,
    pub kind: ToolHandlerKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolRegistryPlan {
    pub specs: Vec<ConfiguredToolSpec>,
    pub handlers: Vec<ToolHandlerSpec>,
}

#[derive(Debug, Clone, Copy)]
pub struct ToolRegistryPlanParams<'a> {
    pub mcp_tools: Option<&'a [ToolRegistryPlanMcpTool<'a>]>,
    pub deferred_mcp_tools: Option<&'a [ToolRegistryPlanDeferredTool<'a>]>,
    pub tool_namespaces: Option<&'a HashMap<String, ToolNamespace>>,
    pub discoverable_tools: Option<&'a [DiscoverableTool]>,
    pub dynamic_tools: &'a [DynamicToolSpec],
    pub default_agent_type_description: &'a str,
    pub wait_agent_timeouts: WaitAgentTimeoutOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolNamespace {
    pub name: String,
    pub description: Option<String>,
}

/// Direct MCP tool metadata needed to expose the Responses API namespace tool
/// while registering its runtime handler with the canonical namespace/name
/// identity.
#[derive(Debug, Clone)]
pub struct ToolRegistryPlanMcpTool<'a> {
    pub name: ToolName,
    pub tool: &'a rmcp::model::Tool,
}

#[derive(Debug, Clone)]
pub struct ToolRegistryPlanDeferredTool<'a> {
    pub name: ToolName,
    pub server_name: &'a str,
    pub connector_name: Option<&'a str>,
    pub description: Option<&'a str>,
}

impl ToolRegistryPlan {
    pub(crate) fn new() -> Self {
        Self {
            specs: Vec::new(),
            handlers: Vec::new(),
        }
    }

    pub(crate) fn push_spec(
        &mut self,
        spec: ToolSpec,
        supports_parallel_tool_calls: bool,
        code_mode_enabled: bool,
    ) {
        let spec = if code_mode_enabled {
            augment_tool_spec_for_code_mode(spec)
        } else {
            spec
        };
        self.specs
            .push(ConfiguredToolSpec::new(spec, supports_parallel_tool_calls));
    }

    pub(crate) fn register_handler(&mut self, name: impl Into<ToolName>, kind: ToolHandlerKind) {
        self.handlers.push(ToolHandlerSpec {
            name: name.into(),
            kind,
        });
    }
}

pub(crate) fn agent_type_description(
    config: &ToolsConfig,
    default_agent_type_description: &str,
) -> String {
    if config.agent_type_description.is_empty() {
        default_agent_type_description.to_string()
    } else {
        config.agent_type_description.clone()
    }
}
