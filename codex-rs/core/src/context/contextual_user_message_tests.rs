use super::*;
use crate::context::ContextualUserFragment;
use crate::context::InternalContextSource;
use crate::context::InternalModelContextFragment;
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
fn detects_internal_model_context_fragment() {
    let text = InternalModelContextFragment::new(
        InternalContextSource::from_static("goal"),
        "Continue working toward the active thread goal.",
    )
    .render();

    assert_eq!(
        text,
        "<codex_internal_context source=\"goal\">\nContinue working toward the active thread goal.\n</codex_internal_context>"
    );
    assert!(is_contextual_user_fragment(&ContentItem::InputText {
        text
    }));
}

#[test]
fn detects_legacy_goal_context_fragment() {
    assert!(is_contextual_user_fragment(&ContentItem::InputText {
        text: "<goal_context>\nContinue working toward the active thread goal.\n</goal_context>"
            .to_string(),
    }));
}

#[test]
fn does_not_hide_arbitrary_context_tags() {
    assert!(!is_contextual_user_fragment(&ContentItem::InputText {
        text: "<project_context>\nbody\n</project_context>".to_string(),
    }));
}

#[test]
fn rejects_invalid_internal_model_context_source() {
    assert!(!is_contextual_user_fragment(&ContentItem::InputText {
        text: "<codex_internal_context source=\"Goal\">\nbody\n</codex_internal_context>"
            .to_string(),
    }));
}

#[test]
fn contextual_user_fragment_is_dyn_compatible() {
    let fragment: Box<dyn ContextualUserFragment> = Box::new(InternalModelContextFragment::new(
        InternalContextSource::from_static("goal"),
        "Continue working toward the active thread goal.",
    ));

    assert_eq!(
        fragment.render(),
        "<codex_internal_context source=\"goal\">\nContinue working toward the active thread goal.\n</codex_internal_context>"
    );
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
