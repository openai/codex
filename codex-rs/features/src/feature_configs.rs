use crate::FeatureConfig;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MultiAgentV2ConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_hint_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_hint_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_spawn_agent_metadata: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_mcp_mode: Option<SubagentMcpMode>,
}

impl FeatureConfig for MultiAgentV2ConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubagentMcpMode {
    /// Sub-agents create their own MCP clients and server processes.
    #[default]
    Fresh,
    /// Sub-agents reuse the parent thread's MCP connection manager without owning refresh or policy state; parentless sub-agents skip MCP startup.
    InheritParent,
    /// Sub-agents skip MCP startup and ignore configured MCP servers.
    Disabled,
}
