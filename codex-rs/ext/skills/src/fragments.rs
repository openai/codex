use codex_core_skills::AvailableSkills;
use codex_core_skills::render::SkillCatalogMode;
use codex_core_skills::render::render_available_skills_body_for_mode;
use codex_extension_api::ContextualUserFragment;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AvailableSkillsInstructions {
    skill_root_lines: Vec<String>,
    skill_lines: Vec<String>,
    mode: SkillCatalogMode,
}

impl AvailableSkillsInstructions {
    pub(crate) fn from_skill_lines(skill_lines: Vec<String>, mode: SkillCatalogMode) -> Self {
        Self {
            skill_root_lines: Vec::new(),
            skill_lines,
            mode,
        }
    }

    pub(crate) fn from_available_skills_with_additional_lines(
        mut available: AvailableSkills,
        additional_lines: Vec<String>,
    ) -> Self {
        let mode = if additional_lines.is_empty() {
            SkillCatalogMode::HostOnly
        } else {
            SkillCatalogMode::Mixed
        };
        available.skill_lines.extend(additional_lines);
        Self {
            skill_root_lines: available.skill_root_lines,
            skill_lines: available.skill_lines,
            mode,
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
        render_available_skills_body_for_mode(&self.skill_root_lines, &self.skill_lines, self.mode)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SkillInstructions {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) contents: String,
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
        let name = &self.name;
        let path = &self.path;
        let contents = &self.contents;
        format!("\n<name>{name}</name>\n<path>{path}</path>\n{contents}\n")
    }
}
