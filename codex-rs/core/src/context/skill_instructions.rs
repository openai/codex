use codex_core_skills::runtime::SkillCatalogEntry;

use super::ContextualUserFragment;

const MAX_SKILL_NAME_BYTES: usize = 256;
const MAX_SKILL_PATH_BYTES: usize = 1_024;
const MAX_SKILL_INSTRUCTIONS_BYTES: usize = 4_000;
const REPLACEMENT_NOTICE: &str = "<replacement_notice>These instructions replace the previously provided instructions for this skill.</replacement_notice>\n";
const UNAVAILABLE_NOTICE: &str = "<unavailable_notice>The previously provided instructions for this skill no longer apply because the skill is unavailable in the current runtime.</unavailable_notice>\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillInstructions {
    name: String,
    path: Option<String>,
    contents: String,
    update: SkillInstructionsUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillInstructionsUpdate {
    Initial,
    Replacement,
    Unavailable,
}

impl SkillInstructions {
    pub(crate) fn from_runtime(
        entry: &SkillCatalogEntry,
        contents: &str,
        max_total_bytes: usize,
        replaces_previous: bool,
    ) -> Option<(Self, bool)> {
        let mut instructions = Self {
            name: truncate_utf8(&entry.name, MAX_SKILL_NAME_BYTES).0,
            path: Some(truncate_utf8(entry.rendered_path(), MAX_SKILL_PATH_BYTES).0),
            contents: String::new(),
            update: if replaces_previous {
                SkillInstructionsUpdate::Replacement
            } else {
                SkillInstructionsUpdate::Initial
            },
        };
        let envelope_bytes = instructions.render().len();
        let max_body_bytes = max_total_bytes
            .min(MAX_SKILL_INSTRUCTIONS_BYTES)
            .checked_sub(envelope_bytes)?;
        let (contents, truncated) = truncate_utf8(contents, max_body_bytes);
        instructions.contents = contents;
        Some((instructions, truncated))
    }

    pub(crate) fn unavailable(name: &str) -> Self {
        Self {
            name: truncate_utf8(name, MAX_SKILL_NAME_BYTES).0,
            path: None,
            contents: String::new(),
            update: SkillInstructionsUpdate::Unavailable,
        }
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> (String, bool) {
    let mut end = value.len().min(max_bytes);
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    (value[..end].to_string(), end < value.len())
}

impl ContextualUserFragment for SkillInstructions {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<skill>", "</skill>")
    }

    fn body(&self) -> String {
        let path = self
            .path
            .as_ref()
            .map(|path| format!("<path>{path}</path>\n"))
            .unwrap_or_default();
        let notice = match self.update {
            SkillInstructionsUpdate::Initial => "",
            SkillInstructionsUpdate::Replacement => REPLACEMENT_NOTICE,
            SkillInstructionsUpdate::Unavailable => UNAVAILABLE_NOTICE,
        };
        format!(
            "\n<name>{}</name>\n{path}{notice}{}\n",
            self.name, self.contents
        )
    }
}
