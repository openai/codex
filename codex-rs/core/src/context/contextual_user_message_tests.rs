use super::*;
use crate::context::ContextualUserFragment;
use codex_protocol::items::HookPromptFragment;
use codex_protocol::items::build_hook_prompt_message;
use codex_protocol::models::ResponseItem;

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
    assert!(SubagentNotification::matches_text(
        "<SUBAGENT_NOTIFICATION>{}</subagent_notification>"
    ));
}

#[test]
fn ignores_regular_user_text() {
    assert!(!is_contextual_user_fragment(&ContentItem::InputText {
        text: "hello".to_string(),
    }));
}

#[test]
fn classifies_memory_excluded_fragments() {
    let cases = [
        (
            "# AGENTS.md instructions for /tmp\n\n<INSTRUCTIONS>\nbody\n</INSTRUCTIONS>",
            true,
        ),
        (
            "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
            true,
        ),
        (
            "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>",
            false,
        ),
        (
            "<subagent_notification>{\"agent_id\":\"a\",\"status\":\"completed\"}</subagent_notification>",
            false,
        ),
    ];

    for (text, expected) in cases {
        assert_eq!(
            is_memory_excluded_contextual_user_fragment(&ContentItem::InputText {
                text: text.to_string(),
            }),
            expected,
            "{text}",
        );
    }
}

#[test]
fn detects_hook_prompt_fragment_and_roundtrips_escaping() {
    let message = build_hook_prompt_message(&[HookPromptFragment::from_single_hook(
        r#"Retry with "waves" & <tides>"#,
        "hook-run-1",
    )])
    .expect("hook prompt message");

    let ResponseItem::Message { content, .. } = message else {
        panic!("expected hook prompt response item");
    };

    let [content_item] = content.as_slice() else {
        panic!("expected a single content item");
    };

    assert!(is_contextual_user_fragment(content_item));

    let ContentItem::InputText { text } = content_item else {
        panic!("expected input text content item");
    };
    let parsed = parse_visible_hook_prompt_message(/*id*/ None, content.as_slice())
        .expect("visible hook prompt");
    assert_eq!(
        parsed.fragments,
        vec![HookPromptFragment {
            text: r#"Retry with "waves" & <tides>"#.to_string(),
            hook_run_id: "hook-run-1".to_string(),
        }],
    );
    assert!(!text.contains("&quot;waves&quot; & <tides>"));
}
