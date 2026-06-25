use codex_context_fragments::ContextualUserFragment;

use crate::injection::SkillInjection;
use crate::runtime::SkillCatalogEntry;

const MAX_SKILL_NAME_BYTES: usize = 256;
const MAX_SKILL_PATH_BYTES: usize = 1_024;
const MAX_SKILL_BODY_BYTES: usize = 8_000;
const REPLACEMENT_NOTICE: &str = "<replacement_notice>These instructions replace the previously provided instructions for this skill.</replacement_notice>\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInstructions {
    name: String,
    path: String,
    contents: String,
    replaces_previous: bool,
}

impl From<&SkillInjection> for SkillInstructions {
    fn from(skill: &SkillInjection) -> Self {
        Self {
            name: skill.name.clone(),
            path: skill.path.clone(),
            contents: skill.contents.clone(),
            replaces_previous: false,
        }
    }
}

impl SkillInstructions {
    /// Builds one hard-bounded instruction fragment from a runtime catalog entry.
    pub fn from_runtime(entry: &SkillCatalogEntry, contents: &str) -> (Self, bool) {
        let (contents, truncated) = truncate_utf8(contents, MAX_SKILL_BODY_BYTES);
        (
            Self {
                name: truncate_utf8(&entry.name, MAX_SKILL_NAME_BYTES).0,
                path: truncate_utf8(entry.rendered_path(), MAX_SKILL_PATH_BYTES).0,
                contents,
                replaces_previous: false,
            },
            truncated,
        )
    }

    pub(crate) fn from_runtime_with_total_limit(
        entry: &SkillCatalogEntry,
        contents: &str,
        max_total_bytes: usize,
        replaces_previous: bool,
    ) -> Option<(Self, bool)> {
        let name = truncate_utf8(&entry.name, MAX_SKILL_NAME_BYTES).0;
        let path = truncate_utf8(entry.rendered_path(), MAX_SKILL_PATH_BYTES).0;
        let mut instructions = Self {
            name,
            path,
            contents: String::new(),
            replaces_previous,
        };
        let envelope_bytes = instructions.render().len();
        let max_body_bytes = max_total_bytes
            .checked_sub(envelope_bytes)?
            .min(MAX_SKILL_BODY_BYTES);
        let (contents, truncated) = truncate_utf8(contents, max_body_bytes);
        instructions.contents = contents;
        Some((instructions, truncated))
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
        let replacement_notice = if self.replaces_previous {
            REPLACEMENT_NOTICE
        } else {
            ""
        };
        format!(
            "\n<name>{}</name>\n<path>{}</path>\n{replacement_notice}{}\n",
            self.name, self.path, self.contents
        )
    }
}
