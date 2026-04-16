use serde::Deserialize;
use serde::Serialize;

use codex_protocol::models::ResponseItem;

use crate::fragment::AGENTS_MD_FRAGMENT;
use crate::fragment::AGENTS_MD_START_MARKER;
use crate::fragment::SKILL_FRAGMENT;

pub const USER_INSTRUCTIONS_PREFIX: &str = AGENTS_MD_START_MARKER;
const INSTRUCTIONS_CLOSE_TAG: &str = "</INSTRUCTIONS>";
const ESCAPED_INSTRUCTIONS_CLOSE_TAG: &str = "<\\/INSTRUCTIONS>";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "user_instructions", rename_all = "snake_case")]
pub struct UserInstructions {
    pub directory: String,
    pub text: String,
}

impl UserInstructions {
    pub fn serialize_to_text(&self) -> String {
        let contents = escape_reserved_instruction_delimiters(&self.text);
        format!(
            "{prefix}{directory}\n\n<INSTRUCTIONS>\n{contents}\n{suffix}",
            prefix = AGENTS_MD_FRAGMENT.start_marker(),
            directory = self.directory,
            contents = contents,
            suffix = AGENTS_MD_FRAGMENT.end_marker(),
        )
    }
}

fn escape_reserved_instruction_delimiters(text: &str) -> String {
    let Some(index) = find_ascii_case_insensitive(text, INSTRUCTIONS_CLOSE_TAG) else {
        return text.to_string();
    };

    let mut output = String::with_capacity(text.len());
    let mut remaining = text;
    let mut next_index = index;
    loop {
        output.push_str(&remaining[..next_index]);
        output.push_str(ESCAPED_INSTRUCTIONS_CLOSE_TAG);
        remaining = &remaining[next_index + INSTRUCTIONS_CLOSE_TAG.len()..];

        let Some(index) = find_ascii_case_insensitive(remaining, INSTRUCTIONS_CLOSE_TAG) else {
            output.push_str(remaining);
            return output;
        };
        next_index = index;
    }
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
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
