use codex_context_fragments::ContextualUserFragment;

use crate::injection::SkillInjection;
use crate::runtime::SkillCatalogEntry;

const MAX_SKILL_NAME_BYTES: usize = 256;
const MAX_SKILL_PATH_BYTES: usize = 1_024;
const MAX_SKILL_BODY_BYTES: usize = 8_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInstructions {
    name: String,
    path: String,
    contents: String,
}

impl From<&SkillInjection> for SkillInstructions {
    fn from(skill: &SkillInjection) -> Self {
        Self {
            name: skill.name.clone(),
            path: skill.path.clone(),
            contents: skill.contents.clone(),
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
            },
            truncated,
        )
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
        format!(
            "\n<name>{}</name>\n<path>{}</path>\n{}\n",
            self.name, self.path, self.contents
        )
    }
}
