use super::*;

#[test]
fn detects_environment_context_fragment() {
    assert!(is_contextual_user_fragment(&ContentItem::InputText {
        text: "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>".to_string(),
    }));
}

#[test]
fn detects_agents_instructions_fragment() {
    assert!(is_contextual_user_fragment(&ContentItem::InputText {
        text: "# AGENTS.md instructions for /tmp\n\n<INSTRUCTIONS>\nbody\n</INSTRUCTIONS>"
            .to_string(),
    }));
}

#[test]
fn detects_subagent_notification_fragment_case_insensitively() {
    assert!(
        SUBAGENT_NOTIFICATION_FRAGMENT
            .matches_text("<SUBAGENT_NOTIFICATION>{}</subagent_notification>")
    );
}

#[test]
fn ignores_regular_user_text() {
    assert!(!is_contextual_user_fragment(&ContentItem::InputText {
        text: "hello".to_string(),
    }));
}
