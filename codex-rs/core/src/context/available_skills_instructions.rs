use codex_core_skills::AvailableSkills;
use codex_core_skills::SKILLS_HOW_TO_USE_WITH_ABSOLUTE_PATHS;
use codex_core_skills::SKILLS_HOW_TO_USE_WITH_ALIASES;
use codex_core_skills::render_available_skills_body;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;

use super::ContextualUserFragment;

/// Model-context fragment describing the skills available to Codex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableSkillsInstructions {
    skill_root_lines: Vec<String>,
    skill_lines: Vec<String>,
}

impl AvailableSkillsInstructions {
    /// Creates a skills context fragment from pre-rendered catalog lines.
    pub fn from_skill_lines(skill_lines: Vec<String>) -> Self {
        Self {
            skill_root_lines: Vec::new(),
            skill_lines,
        }
    }

    pub fn from_available_skills(
        available_skills: &AvailableSkills,
        include_skills_usage_instructions: bool,
    ) -> Self {
        let mut skill_lines = available_skills.skill_lines.clone();
        if include_skills_usage_instructions {
            skill_lines.push("### How to use skills".to_string());
            let instructions = if available_skills.skill_root_lines.is_empty() {
                SKILLS_HOW_TO_USE_WITH_ABSOLUTE_PATHS
            } else {
                SKILLS_HOW_TO_USE_WITH_ALIASES
            };
            skill_lines.push(instructions.to_string());
        }
        Self {
            skill_root_lines: available_skills.skill_root_lines.clone(),
            skill_lines,
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
