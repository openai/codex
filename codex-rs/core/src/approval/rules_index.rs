use super::command_rules::COMMAND_RULES;
use super::command_rules::CommandMatcher;
use super::command_rules::CommandRule;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::LazyLock;

pub(crate) fn build_index() -> HashMap<&'static str, Vec<&'static CommandRule>> {
    let mut map: HashMap<&'static str, Vec<&'static CommandRule>> = HashMap::new();
    for rule in COMMAND_RULES {
        map.entry(rule.tool).or_default().push(rule);
    }
    map
}

pub(crate) static RULE_INDEX: LazyLock<HashMap<&'static str, Vec<&'static CommandRule>>> =
    LazyLock::new(build_index);

pub(crate) static TOOLS_WITH_SUBCOMMANDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut tools: HashSet<&'static str> = HashSet::new();
    for rule in COMMAND_RULES {
        let uses_subcommand = rule
            .has_subcommand
            .unwrap_or(matches!(rule.matcher, CommandMatcher::WithSubcommands(_)));
        if uses_subcommand {
            tools.insert(rule.tool);
        }
    }
    // Git classification bypasses COMMAND_RULES but still exposes subcommands.
    tools.insert("git");
    tools
});

pub(crate) fn tool_uses_subcommand(tool: &str) -> bool {
    TOOLS_WITH_SUBCOMMANDS.contains(tool)
}
