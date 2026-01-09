use crate::features::Feature;
use crate::features::Features;

pub(crate) const HIERARCHICAL_AGENTS_MESSAGE: &str = "Files called AGENTS.md commonly appear in many places inside a container - at \"/\", in \"~\", deep within git repositories, or in any other directory; their location is not limited to version-controlled folders.\n\nTheir purpose is to pass along human guidance to you, the agent. Such guidance can include coding standards, explanations of the project layout, steps for building or testing, and even wording that must accompany a GitHub pull-request description produced by the agent; all of it is to be followed.\n\nEach AGENTS.md governs the entire directory that contains it and every child directory beneath that point. Whenever you change a file, you have to comply with every AGENTS.md whose scope covers that file. Naming conventions, stylistic rules and similar directives are restricted to the code that falls inside that scope unless the document explicitly states otherwise.\n\nWhen two AGENTS.md files disagree, the one located deeper in the directory structure overrides the higher-level file, while instructions given directly in the prompt by the system, developer, or user outrank any AGENTS.md content.";

pub(crate) fn maybe_append_hierarchical_agents_user_instructions(
    user_instructions: Option<String>,
    features: &Features,
) -> Option<String> {
    if !features.enabled(Feature::HierarchicalAgents) {
        return user_instructions;
    }

    match user_instructions {
        Some(text) => Some(append_hierarchical_agents_message(&text).unwrap_or(text)),
        None => Some(HIERARCHICAL_AGENTS_MESSAGE.to_string()),
    }
}

fn append_hierarchical_agents_message(base: &str) -> Option<String> {
    if base.contains(HIERARCHICAL_AGENTS_MESSAGE) {
        return None;
    }

    let mut updated = String::with_capacity(base.len() + HIERARCHICAL_AGENTS_MESSAGE.len() + 2);
    updated.push_str(base);

    if !base.trim().is_empty() && !base.ends_with("\n\n") {
        if base.ends_with('\n') {
            updated.push('\n');
        } else {
            updated.push_str("\n\n");
        }
    }

    updated.push_str(HIERARCHICAL_AGENTS_MESSAGE);
    Some(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn append_adds_message_with_blank_line() {
        let base = "Intro\n\nNotes";
        let updated = append_hierarchical_agents_message(base).expect("should insert");
        let expected = format!("{base}\n\n{HIERARCHICAL_AGENTS_MESSAGE}");
        assert_eq!(updated, expected);
    }

    #[test]
    fn append_handles_trailing_newline() {
        let base = "Intro\n";
        let updated = append_hierarchical_agents_message(base).expect("should insert");
        let expected = format!("{base}\n{HIERARCHICAL_AGENTS_MESSAGE}");
        assert_eq!(updated, expected);
    }

    #[test]
    fn append_handles_empty_base() {
        let base = "";
        let updated = append_hierarchical_agents_message(base).expect("should insert");
        let expected = HIERARCHICAL_AGENTS_MESSAGE.to_string();
        assert_eq!(updated, expected);
    }

    #[test]
    fn append_is_idempotent() {
        let base = format!("Intro\n\n{HIERARCHICAL_AGENTS_MESSAGE}\n\n# Next\n");
        assert!(append_hierarchical_agents_message(&base).is_none());
    }

    #[test]
    fn maybe_append_returns_message_when_missing() {
        let mut features = Features::with_defaults();
        features.enable(Feature::HierarchicalAgents);
        let updated = maybe_append_hierarchical_agents_user_instructions(None, &features)
            .expect("message expected");
        assert_eq!(updated, HIERARCHICAL_AGENTS_MESSAGE);
    }
}
