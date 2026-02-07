use crate::definition::AgentDefinition;

/// Tools that are never available to any subagent, regardless of configuration.
const SYSTEM_BLOCKED: &[&str] = &["Task", "EnterPlanMode", "ExitPlanMode"];

/// Apply three-layer tool filtering for a subagent.
///
/// Filtering is applied in order:
///
/// 1. **System blocked** - tools in `SYSTEM_BLOCKED` are always removed.
/// 2. **Definition allow-list** - if `definition.tools` is non-empty, only
///    those tools are retained.
/// 3. **Definition deny-list** - tools in `definition.disallowed_tools` are
///    removed.
///
/// When `background` is `true`, additional interactive tools are blocked.
pub fn filter_tools_for_agent(
    all_tools: &[String],
    definition: &AgentDefinition,
    background: bool,
) -> Vec<String> {
    let mut result: Vec<String> = all_tools
        .iter()
        .filter(|t| !SYSTEM_BLOCKED.contains(&t.as_str()))
        .cloned()
        .collect();

    // Layer 2: apply allow-list if provided.
    if !definition.tools.is_empty() {
        result.retain(|t| definition.tools.contains(t));
    }

    // Layer 3: apply deny-list.
    if !definition.disallowed_tools.is_empty() {
        result.retain(|t| !definition.disallowed_tools.contains(t));
    }

    // Background agents cannot use interactive tools.
    if background {
        let interactive_blocked = ["UserInput", "AskUser", "ConfirmAction"];
        result.retain(|t| !interactive_blocked.contains(&t.as_str()));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_tools() -> Vec<String> {
        vec![
            "Bash".to_string(),
            "Read".to_string(),
            "Edit".to_string(),
            "Write".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
            "Task".to_string(),
            "EnterPlanMode".to_string(),
            "ExitPlanMode".to_string(),
            "UserInput".to_string(),
        ]
    }

    fn make_def(tools: Vec<&str>, disallowed: Vec<&str>) -> AgentDefinition {
        AgentDefinition {
            name: "test".to_string(),
            description: "test agent".to_string(),
            agent_type: "test".to_string(),
            tools: tools.into_iter().map(String::from).collect(),
            disallowed_tools: disallowed.into_iter().map(String::from).collect(),
            identity: None,
            max_turns: None,
            permission_mode: None,
        }
    }

    #[test]
    fn test_system_blocked_always_removed() {
        let def = make_def(vec![], vec![]);
        let filtered = filter_tools_for_agent(&all_tools(), &def, false);
        assert!(!filtered.contains(&"Task".to_string()));
        assert!(!filtered.contains(&"EnterPlanMode".to_string()));
        assert!(!filtered.contains(&"ExitPlanMode".to_string()));
    }

    #[test]
    fn test_allow_list_filtering() {
        let def = make_def(vec!["Bash", "Read"], vec![]);
        let filtered = filter_tools_for_agent(&all_tools(), &def, false);
        assert_eq!(filtered, vec!["Bash", "Read"]);
    }

    #[test]
    fn test_deny_list_filtering() {
        let def = make_def(vec![], vec!["Edit", "Write"]);
        let filtered = filter_tools_for_agent(&all_tools(), &def, false);
        assert!(filtered.contains(&"Bash".to_string()));
        assert!(filtered.contains(&"Read".to_string()));
        assert!(!filtered.contains(&"Edit".to_string()));
        assert!(!filtered.contains(&"Write".to_string()));
    }

    #[test]
    fn test_combined_allow_deny() {
        let def = make_def(vec!["Bash", "Read", "Edit"], vec!["Edit"]);
        let filtered = filter_tools_for_agent(&all_tools(), &def, false);
        assert_eq!(filtered, vec!["Bash", "Read"]);
    }

    #[test]
    fn test_background_blocks_interactive() {
        let def = make_def(vec![], vec![]);
        let filtered = filter_tools_for_agent(&all_tools(), &def, true);
        assert!(!filtered.contains(&"UserInput".to_string()));
    }

    #[test]
    fn test_background_false_keeps_interactive() {
        let def = make_def(vec![], vec![]);
        let filtered = filter_tools_for_agent(&all_tools(), &def, false);
        assert!(filtered.contains(&"UserInput".to_string()));
    }

    #[test]
    fn test_empty_tools_in() {
        let def = make_def(vec![], vec![]);
        let filtered = filter_tools_for_agent(&[], &def, false);
        assert!(filtered.is_empty());
    }
}
