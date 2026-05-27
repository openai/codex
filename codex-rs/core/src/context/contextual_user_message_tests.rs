use super::*;
use crate::context::ContextualUserFragment;
use crate::context::ExtensionContextTag;
use crate::context::SubagentNotification;
use codex_protocol::items::HookPromptFragment;
use codex_protocol::items::build_hook_prompt_message;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

#[test]
fn detects_environment_context_fragment() {
    assert!(is_contextual_user_fragment(&ContentItem::InputText {
        text: "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>".to_string(),
    }));
}

#[test]
fn detects_extension_context_fragment() {
    assert!(is_contextual_user_fragment(&ContentItem::InputText {
        text: "<extension_context>\n<example>data</example>\n</extension_context>".to_string(),
    }));
}

#[test]
fn detects_goal_extension_context_fragment() {
    let text = ExtensionContext::with_tag(ExtensionContextTag::GOAL, "hidden goal prompt").render();

    assert!(is_contextual_user_fragment(&ContentItem::InputText {
        text
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
