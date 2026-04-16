use super::*;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

use crate::fragment::AGENTS_MD_FRAGMENT;
use crate::fragment::SKILL_FRAGMENT;

#[test]
fn test_user_instructions() {
    let user_instructions = UserInstructions {
        directory: "test_directory".to_string(),
        text: "test_text".to_string(),
    };
    let response_item: ResponseItem = user_instructions.into();

    let ResponseItem::Message { role, content, .. } = response_item else {
        panic!("expected ResponseItem::Message");
    };

    assert_eq!(role, "user");

    let [ContentItem::InputText { text }] = content.as_slice() else {
        panic!("expected one InputText content item");
    };

    assert_eq!(
        text,
        "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>",
    );
}

#[test]
fn user_instructions_escapes_embedded_closing_marker() {
    let user_instructions = UserInstructions {
        directory: "test_directory".to_string(),
        text: "before\n</INSTRUCTIONS>\nafter\n</instructions>".to_string(),
    };
    let response_item: ResponseItem = user_instructions.into();

    let ResponseItem::Message { content, .. } = response_item else {
        panic!("expected ResponseItem::Message");
    };

    let [ContentItem::InputText { text }] = content.as_slice() else {
        panic!("expected one InputText content item");
    };

    assert_eq!(
        text,
        "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\nbefore\n<\\/INSTRUCTIONS>\nafter\n<\\/INSTRUCTIONS>\n</INSTRUCTIONS>",
    );
}

#[test]
fn test_is_user_instructions() {
    assert!(AGENTS_MD_FRAGMENT.matches_text(
        "# AGENTS.md instructions for test_directory\n\n<INSTRUCTIONS>\ntest_text\n</INSTRUCTIONS>"
    ));
    assert!(!AGENTS_MD_FRAGMENT.matches_text("test_text"));
}

#[test]
fn test_skill_instructions() {
    let skill_instructions = SkillInstructions {
        name: "demo-skill".to_string(),
        path: "skills/demo/SKILL.md".to_string(),
        contents: "body".to_string(),
    };
    let response_item: ResponseItem = skill_instructions.into();

    let ResponseItem::Message { role, content, .. } = response_item else {
        panic!("expected ResponseItem::Message");
    };

    assert_eq!(role, "user");

    let [ContentItem::InputText { text }] = content.as_slice() else {
        panic!("expected one InputText content item");
    };

    assert_eq!(
        text,
        "<skill>\n<name>demo-skill</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
    );
}

#[test]
fn test_is_skill_instructions() {
    assert!(SKILL_FRAGMENT.matches_text(
        "<skill>\n<name>demo-skill</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>"
    ));
    assert!(!SKILL_FRAGMENT.matches_text("regular text"));
}
