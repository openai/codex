use codex_context_fragments::ContextualUserFragment;

use crate::injection::SkillInjection;

#[derive(Debug, Clone, PartialEq)]
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

impl ContextualUserFragment for SkillInstructions {
    fn role() -> &'static str {
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
