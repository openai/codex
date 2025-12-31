//! Built-in agent definitions (Explore and Plan).

use super::AgentDefinition;
use super::AgentRunConfig;
use super::AgentSource;
use super::ApprovalMode;
use super::ModelConfig;
use super::PromptConfig;
use super::ToolAccess;
use crate::tools::names;
use std::sync::LazyLock;

/// Built-in Explore agent for fast codebase exploration.
pub static EXPLORE_AGENT: LazyLock<AgentDefinition> = LazyLock::new(|| AgentDefinition {
    agent_type: "Explore".to_string(),
    display_name: Some("Codebase Explorer".to_string()),
    when_to_use: Some(
        "Use this agent to quickly find files by patterns, search code for keywords, \
             or answer questions about the codebase. Specify thoroughness: 'quick', \
             'medium', or 'very thorough'."
            .to_string(),
    ),
    tools: ToolAccess::List(vec![
        names::THINK.to_string(),
        names::READ_FILE.to_string(),
        names::LIST_DIR.to_string(),
        names::GLOB_FILES.to_string(),
        names::GREP_FILES.to_string(),
        names::WEB_FETCH.to_string(),
        names::WEB_SEARCH.to_string(),
    ]),
    disallowed_tools: vec![
        names::SMART_EDIT.to_string(),
        names::APPLY_PATCH.to_string(),
        names::WRITE_FILE.to_string(),
        names::SHELL_COMMAND.to_string(),
        names::TASK.to_string(),
    ],
    source: AgentSource::Builtin,
    model_config: ModelConfig::default(),
    fork_context: false,
    prompt_config: PromptConfig {
        system_prompt: Some(include_str!("../prompts/explore.md").to_string()),
        query: None,
    },
    run_config: AgentRunConfig {
        max_time_seconds: 120,
        max_turns: 100,
        grace_period_seconds: 30,
    },
    input_config: None,
    output_config: None,
    approval_mode: ApprovalMode::DontAsk,
    critical_system_reminder: Some(
        "CRITICAL: This is a READ-ONLY exploration task. You CANNOT edit, write, \
             or create files. Only use read, glob, and grep tools."
            .to_string(),
    ),
});

/// Built-in Plan agent for implementation planning.
pub static PLAN_AGENT: LazyLock<AgentDefinition> = LazyLock::new(|| AgentDefinition {
    agent_type: "Plan".to_string(),
    display_name: Some("Implementation Planner".to_string()),
    when_to_use: Some(
        "Use this agent to design implementation plans for complex tasks. \
             Returns step-by-step plans, identifies critical files, and considers \
             architectural trade-offs."
            .to_string(),
    ),
    tools: ToolAccess::List(vec![
        names::THINK.to_string(),
        names::READ_FILE.to_string(),
        names::LIST_DIR.to_string(),
        names::GLOB_FILES.to_string(),
        names::GREP_FILES.to_string(),
        names::WEB_FETCH.to_string(),
        names::WEB_SEARCH.to_string(),
    ]),
    disallowed_tools: vec![
        names::SMART_EDIT.to_string(),
        names::APPLY_PATCH.to_string(),
        names::WRITE_FILE.to_string(),
        names::SHELL_COMMAND.to_string(),
        names::TASK.to_string(),
    ],
    source: AgentSource::Builtin,
    model_config: ModelConfig::default(),
    fork_context: true,
    prompt_config: PromptConfig {
        system_prompt: Some(include_str!("../prompts/plan.md").to_string()),
        query: None,
    },
    run_config: AgentRunConfig {
        max_time_seconds: 300,
        max_turns: 100,
        grace_period_seconds: 60,
    },
    input_config: None,
    output_config: None,
    approval_mode: ApprovalMode::DontAsk,
    critical_system_reminder: Some(
        "CRITICAL: This is a PLANNING task. You CANNOT edit, write, or create files. \
             Focus on analyzing the codebase and creating a detailed implementation plan."
            .to_string(),
    ),
});

/// Get all built-in agent definitions.
pub fn get_builtin_agents() -> Vec<&'static AgentDefinition> {
    vec![&*EXPLORE_AGENT, &*PLAN_AGENT]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explore_agent() {
        let agent = &*EXPLORE_AGENT;
        assert_eq!(agent.agent_type, "Explore");
        assert!(agent.is_builtin());
        assert!(matches!(agent.approval_mode, ApprovalMode::DontAsk));
    }

    #[test]
    fn test_plan_agent() {
        let agent = &*PLAN_AGENT;
        assert_eq!(agent.agent_type, "Plan");
        assert!(agent.fork_context);
    }

    #[test]
    fn test_get_builtin_agents() {
        let agents = get_builtin_agents();
        assert_eq!(agents.len(), 2);
        assert!(agents.iter().any(|a| a.agent_type == "Explore"));
        assert!(agents.iter().any(|a| a.agent_type == "Plan"));
    }
}
