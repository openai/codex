mod bash;
mod explore;
mod general;
mod guide;
mod plan;
mod statusline;

use crate::definition::AgentDefinition;
use cocode_config::BuiltinAgentOverride;
use cocode_config::BuiltinAgentsConfig;
use cocode_protocol::execution::ExecutionIdentity;
use cocode_protocol::model::ModelRole;

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

/// Returns builtin agents with config overrides applied.
///
/// Loads configuration from `~/.cocode/builtin-agents.json` and applies
/// any overrides to the hardcoded agent definitions.
///
/// # Example
///
/// ```ignore
/// use cocode_subagent::definitions::builtin_agents_with_overrides;
///
/// let agents = builtin_agents_with_overrides();
/// // Agents now have any user-configured overrides applied
/// ```
pub fn builtin_agents_with_overrides() -> Vec<AgentDefinition> {
    let config = cocode_config::load_builtin_agents_config();
    builtin_agents_with_config(&config)
}

/// Returns builtin agents with the given config overrides applied.
///
/// This is the lower-level function that takes an explicit config,
/// useful for testing or when config is already loaded.
pub fn builtin_agents_with_config(config: &BuiltinAgentsConfig) -> Vec<AgentDefinition> {
    builtin_agents()
        .into_iter()
        .map(|mut def| {
            if let Some(override_cfg) = config.get(&def.agent_type) {
                apply_override(&mut def, override_cfg);
            }
            def
        })
        .collect()
}

/// Apply override configuration to an agent definition.
fn apply_override(def: &mut AgentDefinition, cfg: &BuiltinAgentOverride) {
    if let Some(max_turns) = cfg.max_turns {
        def.max_turns = Some(max_turns);
    }
    if let Some(ref identity) = cfg.identity {
        def.identity = Some(parse_identity(identity));
    }
    if let Some(ref tools) = cfg.tools {
        def.tools = tools.clone();
    }
    if let Some(ref disallowed) = cfg.disallowed_tools {
        def.disallowed_tools = disallowed.clone();
    }
}

/// Parse an identity string into an ExecutionIdentity.
///
/// Supported values:
/// - "main", "fast", "explore", "plan", "vision", "review", "compact" -> Role(ModelRole::*)
/// - "inherit" or unknown -> Inherit
fn parse_identity(s: &str) -> ExecutionIdentity {
    match s.to_lowercase().as_str() {
        "main" => ExecutionIdentity::Role(ModelRole::Main),
        "fast" => ExecutionIdentity::Role(ModelRole::Fast),
        "explore" => ExecutionIdentity::Role(ModelRole::Explore),
        "plan" => ExecutionIdentity::Role(ModelRole::Plan),
        "vision" => ExecutionIdentity::Role(ModelRole::Vision),
        "review" => ExecutionIdentity::Role(ModelRole::Review),
        "compact" => ExecutionIdentity::Role(ModelRole::Compact),
        "inherit" | _ => ExecutionIdentity::Inherit,
    }
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

    #[test]
    fn test_builtin_agents_with_empty_config() {
        let config = BuiltinAgentsConfig::new();
        let agents = builtin_agents_with_config(&config);
        assert_eq!(agents.len(), 6);

        // Should be unchanged from defaults
        let explore = agents.iter().find(|a| a.agent_type == "explore").unwrap();
        assert_eq!(explore.max_turns, Some(20));
    }

    #[test]
    fn test_builtin_agents_with_max_turns_override() {
        let mut config = BuiltinAgentsConfig::new();
        config.insert(
            "explore".to_string(),
            BuiltinAgentOverride {
                max_turns: Some(50),
                identity: None,
                tools: None,
                disallowed_tools: None,
            },
        );

        let agents = builtin_agents_with_config(&config);
        let explore = agents.iter().find(|a| a.agent_type == "explore").unwrap();
        assert_eq!(explore.max_turns, Some(50));
    }

    #[test]
    fn test_builtin_agents_with_identity_override() {
        let mut config = BuiltinAgentsConfig::new();
        config.insert(
            "explore".to_string(),
            BuiltinAgentOverride {
                max_turns: None,
                identity: Some("fast".to_string()),
                tools: None,
                disallowed_tools: None,
            },
        );

        let agents = builtin_agents_with_config(&config);
        let explore = agents.iter().find(|a| a.agent_type == "explore").unwrap();
        assert!(matches!(
            explore.identity,
            Some(ExecutionIdentity::Role(ModelRole::Fast))
        ));
    }

    #[test]
    fn test_builtin_agents_with_tools_override() {
        let mut config = BuiltinAgentsConfig::new();
        config.insert(
            "explore".to_string(),
            BuiltinAgentOverride {
                max_turns: None,
                identity: None,
                tools: Some(vec!["Read".to_string(), "Bash".to_string()]),
                disallowed_tools: None,
            },
        );

        let agents = builtin_agents_with_config(&config);
        let explore = agents.iter().find(|a| a.agent_type == "explore").unwrap();
        assert_eq!(explore.tools, vec!["Read", "Bash"]);
    }

    #[test]
    fn test_builtin_agents_unknown_agent_ignored() {
        let mut config = BuiltinAgentsConfig::new();
        config.insert(
            "unknown_agent".to_string(),
            BuiltinAgentOverride {
                max_turns: Some(999),
                identity: None,
                tools: None,
                disallowed_tools: None,
            },
        );

        let agents = builtin_agents_with_config(&config);
        // Should still have 6 agents, unknown config is ignored
        assert_eq!(agents.len(), 6);
    }

    #[test]
    fn test_parse_identity_roles() {
        assert!(matches!(
            parse_identity("main"),
            ExecutionIdentity::Role(ModelRole::Main)
        ));
        assert!(matches!(
            parse_identity("fast"),
            ExecutionIdentity::Role(ModelRole::Fast)
        ));
        assert!(matches!(
            parse_identity("explore"),
            ExecutionIdentity::Role(ModelRole::Explore)
        ));
        assert!(matches!(
            parse_identity("plan"),
            ExecutionIdentity::Role(ModelRole::Plan)
        ));
        assert!(matches!(
            parse_identity("vision"),
            ExecutionIdentity::Role(ModelRole::Vision)
        ));
        assert!(matches!(
            parse_identity("review"),
            ExecutionIdentity::Role(ModelRole::Review)
        ));
        assert!(matches!(
            parse_identity("compact"),
            ExecutionIdentity::Role(ModelRole::Compact)
        ));
    }

    #[test]
    fn test_parse_identity_inherit() {
        assert!(matches!(
            parse_identity("inherit"),
            ExecutionIdentity::Inherit
        ));
        assert!(matches!(
            parse_identity("unknown"),
            ExecutionIdentity::Inherit
        ));
        assert!(matches!(parse_identity(""), ExecutionIdentity::Inherit));
    }

    #[test]
    fn test_parse_identity_case_insensitive() {
        assert!(matches!(
            parse_identity("MAIN"),
            ExecutionIdentity::Role(ModelRole::Main)
        ));
        assert!(matches!(
            parse_identity("Fast"),
            ExecutionIdentity::Role(ModelRole::Fast)
        ));
        assert!(matches!(
            parse_identity("EXPLORE"),
            ExecutionIdentity::Role(ModelRole::Explore)
        ));
    }
}
