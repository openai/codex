//! Configuration builder for subagent sessions.
//!
//! Transforms AgentDefinition into Config suitable for Codex::spawn().

use std::collections::HashSet;
use std::sync::Arc;

use crate::config::Config;
use crate::subagent::AgentDefinition;
use crate::subagent::ApprovalMode;
use crate::subagent::ToolAccess;
use codex_protocol::protocol::AskForApproval;

/// Builds a Config suitable for subagent execution from AgentDefinition.
///
/// This handles:
/// - Tool filtering: ToolAccess → allowed_tools set
/// - Approval mode mapping: ApprovalMode → AskForApproval
/// - Prompt configuration: system_prompt → developer_instructions
pub struct SubagentConfigBuilder {
    base_config: Arc<Config>,
    definition: Arc<AgentDefinition>,
}

impl SubagentConfigBuilder {
    /// Create a new config builder.
    pub fn new(base_config: Arc<Config>, definition: Arc<AgentDefinition>) -> Self {
        Self {
            base_config,
            definition,
        }
    }

    /// Build the subagent Config.
    pub fn build(self) -> SubagentConfig {
        let allowed_tools = self.build_allowed_tools();
        let blocked_tools = self.build_blocked_tools();
        let approval_policy = self.map_approval_mode();
        let developer_instructions = self.build_developer_instructions();

        SubagentConfig {
            base_config: self.base_config,
            definition: self.definition,
            allowed_tools,
            blocked_tools,
            approval_policy,
            developer_instructions,
        }
    }

    /// Build the set of allowed tools based on AgentDefinition.tools.
    fn build_allowed_tools(&self) -> Option<HashSet<String>> {
        match &self.definition.tools {
            ToolAccess::All => None, // None means all tools allowed
            ToolAccess::List(tools) => Some(tools.iter().cloned().collect()),
        }
    }

    /// Build the set of blocked tools.
    fn build_blocked_tools(&self) -> HashSet<String> {
        self.definition.disallowed_tools.iter().cloned().collect()
    }

    /// Map ApprovalMode to AskForApproval.
    fn map_approval_mode(&self) -> AskForApproval {
        match self.definition.approval_mode {
            ApprovalMode::RouteToParent => {
                // Use parent's approval policy
                self.base_config.approval_policy.value().clone()
            }
            ApprovalMode::AutoApprove => AskForApproval::Never,
            ApprovalMode::DontAsk => AskForApproval::Never,
        }
    }

    /// Build developer instructions from prompt config.
    fn build_developer_instructions(&self) -> Option<String> {
        let mut instructions = Vec::new();

        // Add system prompt from agent definition
        if let Some(system_prompt) = &self.definition.prompt_config.system_prompt {
            instructions.push(system_prompt.clone());
        }

        // Add critical system reminder if present
        if let Some(reminder) = &self.definition.critical_system_reminder {
            instructions.push(format!(
                "\n<critical-reminder>\n{reminder}\n</critical-reminder>"
            ));
        }

        if instructions.is_empty() {
            None
        } else {
            Some(instructions.join("\n"))
        }
    }
}

/// Configuration for a subagent session.
#[derive(Debug, Clone)]
pub struct SubagentConfig {
    /// Base config from parent session.
    pub base_config: Arc<Config>,

    /// Agent definition.
    pub definition: Arc<AgentDefinition>,

    /// Allowed tools (None means all tools).
    /// When Some, only tools in this set will be included.
    pub allowed_tools: Option<HashSet<String>>,

    /// Blocked tools - these are always excluded.
    pub blocked_tools: HashSet<String>,

    /// Approval policy for this subagent.
    pub approval_policy: AskForApproval,

    /// Developer instructions to prepend.
    pub developer_instructions: Option<String>,
}

impl SubagentConfig {
    /// Check if a tool is allowed for this subagent.
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        // Check blocked list first
        if self.blocked_tools.contains(tool_name) {
            return false;
        }

        // Check allowed list if present
        match &self.allowed_tools {
            None => true,
            Some(allowed) => allowed.contains(tool_name),
        }
    }

    /// Get the maximum execution time in seconds.
    pub fn max_time_seconds(&self) -> i32 {
        self.definition.run_config.max_time_seconds
    }

    /// Get the maximum number of turns.
    pub fn max_turns(&self) -> i32 {
        self.definition.run_config.max_turns
    }

    /// Get the grace period in seconds.
    pub fn grace_period_seconds(&self) -> i32 {
        self.definition.run_config.grace_period_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_config;
    use crate::subagent::AgentRunConfig;
    use crate::subagent::AgentSource;
    use crate::subagent::ModelConfig;
    use crate::subagent::PromptConfig;

    fn make_test_definition(tools: ToolAccess, approval: ApprovalMode) -> AgentDefinition {
        AgentDefinition {
            agent_type: "Test".to_string(),
            display_name: None,
            when_to_use: None,
            tools,
            disallowed_tools: vec!["dangerous_tool".to_string()],
            source: AgentSource::Builtin,
            model_config: ModelConfig::default(),
            fork_context: false,
            prompt_config: PromptConfig {
                system_prompt: Some("You are a test agent.".to_string()),
                query: None,
            },
            run_config: AgentRunConfig::default(),
            input_config: None,
            output_config: None,
            approval_mode: approval,
            critical_system_reminder: Some("Be careful!".to_string()),
        }
    }

    #[test]
    fn test_tool_access_all() {
        let definition = Arc::new(make_test_definition(
            ToolAccess::All,
            ApprovalMode::AutoApprove,
        ));
        let config = Arc::new(test_config());
        let builder = SubagentConfigBuilder::new(config, definition);
        let subagent_config = builder.build();

        assert!(subagent_config.allowed_tools.is_none());
        assert!(subagent_config.is_tool_allowed("read_file"));
        assert!(subagent_config.is_tool_allowed("shell"));
        // But blocked tool should still be blocked
        assert!(!subagent_config.is_tool_allowed("dangerous_tool"));
    }

    #[test]
    fn test_tool_access_list() {
        let definition = Arc::new(make_test_definition(
            ToolAccess::List(vec!["Read".to_string(), "Glob".to_string()]),
            ApprovalMode::AutoApprove,
        ));
        let config = Arc::new(test_config());
        let builder = SubagentConfigBuilder::new(config, definition);
        let subagent_config = builder.build();

        assert!(subagent_config.allowed_tools.is_some());
        assert!(subagent_config.is_tool_allowed("Read"));
        assert!(subagent_config.is_tool_allowed("Glob"));
        assert!(!subagent_config.is_tool_allowed("shell"));
    }

    #[test]
    fn test_approval_mode_auto_approve() {
        let definition = Arc::new(make_test_definition(
            ToolAccess::All,
            ApprovalMode::AutoApprove,
        ));
        let config = Arc::new(test_config());
        let builder = SubagentConfigBuilder::new(config, definition);
        let subagent_config = builder.build();

        assert_eq!(subagent_config.approval_policy, AskForApproval::Never);
    }

    #[test]
    fn test_developer_instructions() {
        let definition = Arc::new(make_test_definition(
            ToolAccess::All,
            ApprovalMode::AutoApprove,
        ));
        let config = Arc::new(test_config());
        let builder = SubagentConfigBuilder::new(config, definition);
        let subagent_config = builder.build();

        let instructions = subagent_config.developer_instructions.unwrap();
        assert!(instructions.contains("You are a test agent."));
        assert!(instructions.contains("<critical-reminder>"));
        assert!(instructions.contains("Be careful!"));
    }
}
