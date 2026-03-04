use serde::Deserialize;
use serde::Serialize;

use crate::contextual_user_message::ContextualUserFragment;
use codex_protocol::models::ResponseItem;

use crate::contextual_user_message::AGENTS_MD_FRAGMENT;
use crate::contextual_user_message::SKILL_FRAGMENT;

pub const USER_INSTRUCTIONS_PREFIX: &str = "# AGENTS.md instructions for ";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "user_instructions", rename_all = "snake_case")]
pub(crate) struct UserInstructions {
    pub directory: String,
    pub text: String,
}

impl UserInstructions {
    pub(crate) fn serialize_to_text(&self) -> String {
        format!(
            "{prefix}{directory}\n\n<INSTRUCTIONS>\n{contents}\n{suffix}",
            prefix = AGENTS_MD_FRAGMENT.start_marker(),
            directory = self.directory,
            contents = self.text,
            suffix = AGENTS_MD_FRAGMENT.end_marker(),
        )
    }
}

impl ContextualUserFragment for UserInstructions {
    fn definition(&self) -> crate::contextual_user_message::ContextualUserFragmentDefinition {
        AGENTS_MD_FRAGMENT
    }

    fn serialize_to_text(&self) -> String {
        Self::serialize_to_text(self)
    }
}

impl From<UserInstructions> for ResponseItem {
    fn from(ui: UserInstructions) -> Self {
        ui.into_response_item()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "skill_instructions", rename_all = "snake_case")]
pub(crate) struct SkillInstructions {
    pub name: String,
    pub path: String,
    pub contents: String,
}

impl SkillInstructions {
    pub(crate) fn serialize_to_text(&self) -> String {
        SKILL_FRAGMENT.wrap_body(format!(
            "<name>{}</name>\n<path>{}</path>\n{}",
            self.name, self.path, self.contents
        ))
    }
}

impl ContextualUserFragment for SkillInstructions {
    fn definition(&self) -> crate::contextual_user_message::ContextualUserFragmentDefinition {
        SKILL_FRAGMENT
    }

    fn serialize_to_text(&self) -> String {
        Self::serialize_to_text(self)
    }
}

impl From<SkillInstructions> for ResponseItem {
    fn from(si: SkillInstructions) -> Self {
        si.into_response_item()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;

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
}
