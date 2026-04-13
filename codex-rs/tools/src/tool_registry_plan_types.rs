use crate::ConfiguredToolSpec;
use crate::DiscoverableTool;
use crate::ToolName;
use crate::ToolSpec;
use crate::ToolsConfig;
use crate::WaitAgentTimeoutOptions;
use crate::augment_tool_spec_for_code_mode;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use rmcp::model::Tool as McpTool;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolHandlerKind {
    AgentJobs,
    ApplyPatch,
    CloseAgentV1,
    CloseAgentV2,
    CodeModeExecute,
    CodeModeWait,
    DynamicTool,
    FollowupTaskV2,
    JsRepl,
    JsReplReset,
    ListAgentsV2,
    ListDir,
    Mcp,
    McpResource,
    Plan,
    RequestPermissions,
    RequestUserInput,
    ResumeAgentV1,
    SendInputV1,
    SendMessageV2,
    Shell,
    ShellCommand,
    SpawnAgentV1,
    SpawnAgentV2,
    TestSync,
    ToolSearch,
    ToolSuggest,
    UnifiedExec,
    ViewImage,
    WaitAgentV1,
    WaitAgentV2,
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
    pub mcp_tools: Option<&'a HashMap<String, McpTool>>,
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

#[derive(Debug, Clone, Copy)]
pub struct ToolRegistryPlanDeferredTool<'a> {
    pub tool_name: &'a str,
    pub tool_namespace: &'a str,
    pub server_name: &'a str,
    pub connector_name: Option<&'a str>,
    pub connector_description: Option<&'a str>,
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
