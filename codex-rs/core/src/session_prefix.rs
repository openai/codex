use codex_protocol::models::ContentItem;

use crate::instructions::SkillInstructions;
use crate::instructions::UserInstructions;
use crate::user_shell_command::is_user_shell_command_text;

/// Helpers for identifying model-visible "session prefix" messages.
///
/// A session prefix is a user-role message that carries configuration or state needed by
/// follow-up turns (e.g. `<environment_context>`, `<turn_aborted>`). These items are persisted in
/// history so the model can see them, but they are not user intent and must not create user-turn
/// boundaries.
pub(crate) const ENVIRONMENT_CONTEXT_OPEN_TAG: &str = "<environment_context>";
pub(crate) const TURN_ABORTED_OPEN_TAG: &str = "<turn_aborted>";
pub(crate) const CONTEXT_UPDATE_OPEN_TAG: &str = "<context_update>";

/// Returns true if `text` starts with a session prefix marker (case-insensitive).
pub(crate) fn is_session_prefix(text: &str) -> bool {
    let trimmed = text.trim_start();
    let lowered = trimmed.to_ascii_lowercase();
    lowered.starts_with(ENVIRONMENT_CONTEXT_OPEN_TAG)
        || lowered.starts_with(TURN_ABORTED_OPEN_TAG)
        || lowered.starts_with(CONTEXT_UPDATE_OPEN_TAG)
}

pub(crate) fn is_contextual_user_message(content: &[ContentItem]) -> bool {
    if UserInstructions::is_user_instructions(content)
        || SkillInstructions::is_skill_instructions(content)
    {
        return true;
    }

    content.iter().any(|content_item| match content_item {
        ContentItem::InputText { text } => {
            is_session_prefix(text) || is_user_shell_command_text(text)
        }
        ContentItem::OutputText { text } => is_session_prefix(text),
        ContentItem::InputImage { .. } => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;

    #[test]
    fn recognizes_context_update_session_prefix() {
        assert!(is_session_prefix(
            "<context_update>\nfoo\n</context_update>"
        ));
        assert!(is_session_prefix(
            "   <context_update>\nfoo\n</context_update>"
        ));
        assert!(is_session_prefix(
            "<CONTEXT_UPDATE>\nfoo\n</CONTEXT_UPDATE>"
        ));
    }

    #[test]
    fn recognizes_legacy_session_prefixes() {
        assert!(is_session_prefix(
            "<environment_context>foo</environment_context>"
        ));
        assert!(is_session_prefix("<turn_aborted>foo</turn_aborted>"));
    }

    #[test]
    fn does_not_treat_plain_text_as_session_prefix() {
        assert_eq!(is_session_prefix("normal user message"), false);
    }

    #[test]
    fn contextual_user_message_detects_context_markers_and_wrappers() {
        let shell = [ContentItem::InputText {
            text: "<user_shell_command>echo hi</user_shell_command>".to_string(),
        }];
        let context_update = [ContentItem::InputText {
            text: "<context_update>\nfoo\n</context_update>".to_string(),
        }];
        let user_instructions = [ContentItem::InputText {
            text: "# AGENTS.md instructions for test\n\n<INSTRUCTIONS>\nfoo\n</INSTRUCTIONS>"
                .to_string(),
        }];
        let skill = [ContentItem::InputText {
            text: "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>"
                .to_string(),
        }];

        assert!(is_contextual_user_message(&shell));
        assert!(is_contextual_user_message(&context_update));
        assert!(is_contextual_user_message(&user_instructions));
        assert!(is_contextual_user_message(&skill));
    }

    #[test]
    fn contextual_user_message_keeps_real_user_content() {
        let real_message = [
            ContentItem::InputText {
                text: "normal user message".to_string(),
            },
            ContentItem::InputImage {
                image_url: "https://example.com/img.png".to_string(),
            },
        ];

        assert_eq!(is_contextual_user_message(&real_message), false);
    }
}
