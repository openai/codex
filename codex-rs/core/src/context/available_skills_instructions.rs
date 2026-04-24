use codex_core_skills::AvailableSkills;
use codex_core_skills::render_available_skills_body;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AvailableSkillsInstructions {
    skill_root_lines: Vec<String>,
    skill_lines: Vec<String>,
}

impl From<AvailableSkills> for AvailableSkillsInstructions {
    fn from(available_skills: AvailableSkills) -> Self {
        Self {
            skill_root_lines: available_skills.skill_root_lines,
            skill_lines: available_skills.skill_lines,
        }
    }
}

impl ContextualUserFragment for AvailableSkillsInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = SKILLS_INSTRUCTIONS_OPEN_TAG;
    const END_MARKER: &'static str = SKILLS_INSTRUCTIONS_CLOSE_TAG;

    fn body(&self) -> String {
        render_available_skills_body(&self.skill_root_lines, &self.skill_lines)
    }
}
