mod bash;
mod explore;
mod general;
mod guide;
mod plan;
mod statusline;

use crate::definition::AgentDefinition;

/// Returns the complete set of built-in agent definitions.
///
/// These agents cover the most common subagent use-cases: shell execution,
/// file exploration, planning, general-purpose coding, guided reading, and
/// status-line updates.
pub fn builtin_agents() -> Vec<AgentDefinition> {
    vec![
        bash::bash_agent(),
        general::general_agent(),
        explore::explore_agent(),
        plan::plan_agent(),
        guide::guide_agent(),
        statusline::statusline_agent(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_agents_count() {
        let agents = builtin_agents();
        assert_eq!(agents.len(), 6);
    }

    #[test]
    fn test_builtin_agents_unique_names() {
        let agents = builtin_agents();
        let mut names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 6, "All agent names should be unique");
    }

    #[test]
    fn test_builtin_agent_types() {
        let agents = builtin_agents();
        let types: Vec<&str> = agents.iter().map(|a| a.agent_type.as_str()).collect();
        assert!(types.contains(&"bash"));
        assert!(types.contains(&"general"));
        assert!(types.contains(&"explore"));
        assert!(types.contains(&"plan"));
        assert!(types.contains(&"guide"));
        assert!(types.contains(&"statusline"));
    }
}
