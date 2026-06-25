use codex_core_skills::AvailableSkills;
use codex_core_skills::render_available_skills_body;
use codex_extension_api::ContextualUserFragment;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AvailableSkillsInstructions {
    skill_root_lines: Vec<String>,
    skill_lines: Vec<String>,
}

impl From<AvailableSkills> for AvailableSkillsInstructions {
    fn from(available: AvailableSkills) -> Self {
        Self {
            skill_root_lines: available.skill_root_lines,
            skill_lines: available.skill_lines,
        }
    }
}

impl ContextualUserFragment for AvailableSkillsInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG)
    }

    fn body(&self) -> String {
        render_available_skills_body(&self.skill_root_lines, &self.skill_lines)
    }
}
