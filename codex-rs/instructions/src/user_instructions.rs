use serde::Deserialize;
use serde::Serialize;

use codex_protocol::models::ResponseItem;

use crate::fragment::AGENTS_MD_FRAGMENT;
use crate::fragment::AGENTS_MD_START_MARKER;
use crate::fragment::SKILL_FRAGMENT;

pub const USER_INSTRUCTIONS_PREFIX: &str = AGENTS_MD_START_MARKER;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "user_instructions", rename_all = "snake_case")]
pub struct UserInstructions {
    pub directory: String,
    pub text: String,
}

impl UserInstructions {
    pub fn serialize_to_text(&self) -> String {
        format!(
            "{prefix}{directory}\n\n<INSTRUCTIONS>\n{contents}\n{suffix}",
            prefix = AGENTS_MD_FRAGMENT.start_marker(),
            directory = self.directory,
            contents = self.text,
            suffix = AGENTS_MD_FRAGMENT.end_marker(),
        )
    }
}

impl From<UserInstructions> for ResponseItem {
    fn from(ui: UserInstructions) -> Self {
        AGENTS_MD_FRAGMENT.into_message(ui.serialize_to_text())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "skill_instructions", rename_all = "snake_case")]
pub struct SkillInstructions {
    pub name: String,
    pub path: String,
    pub contents: String,
}

impl From<SkillInstructions> for ResponseItem {
    fn from(si: SkillInstructions) -> Self {
        SKILL_FRAGMENT.into_message(SKILL_FRAGMENT.wrap(format!(
            "<name>{}</name>\n<path>{}</path>\n{}",
            si.name, si.path, si.contents
        )))
    }
}

#[cfg(test)]
#[path = "user_instructions_tests.rs"]
mod tests;
