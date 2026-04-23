use codex_core_skills::AvailableSkills;
use codex_core_skills::SKILLS_HOW_TO_USE_WITH_ABSOLUTE_PATHS;
use codex_core_skills::SKILLS_HOW_TO_USE_WITH_ALIASES;
use codex_core_skills::SKILLS_INTRO_WITH_ABSOLUTE_PATHS;
use codex_core_skills::SKILLS_INTRO_WITH_ALIASES;
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
        let mut lines: Vec<String> = Vec::new();
        lines.push("## Skills".to_string());
        if self.skill_root_lines.is_empty() {
            lines.push(SKILLS_INTRO_WITH_ABSOLUTE_PATHS.to_string());
        } else {
            lines.push(SKILLS_INTRO_WITH_ALIASES.to_string());
            lines.push("### Skill roots".to_string());
            lines.extend(self.skill_root_lines.iter().cloned());
        }
        lines.push("### Available skills".to_string());
        lines.extend(self.skill_lines.iter().cloned());

        lines.push("### How to use skills".to_string());
        let how_to_use = if self.skill_root_lines.is_empty() {
            SKILLS_HOW_TO_USE_WITH_ABSOLUTE_PATHS
        } else {
            SKILLS_HOW_TO_USE_WITH_ALIASES
        };
        lines.push(how_to_use.to_string());

        format!("\n{}\n", lines.join("\n"))
    }
}
