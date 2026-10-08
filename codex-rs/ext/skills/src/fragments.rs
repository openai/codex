use codex_extension_api::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;

use crate::catalog_prompt::SkillPromptKind;
use crate::catalog_prompt::render_available_skills_body;
use crate::tools::SkillToolAuthority;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AvailableSkillsInstructions {
    prompt_kind: SkillPromptKind,
    skill_root_lines: Vec<String>,
    skill_lines: Vec<String>,
}

impl AvailableSkillsInstructions {
    pub(crate) fn from_skill_lines(
        prompt_kind: SkillPromptKind,
        skill_root_lines: Vec<String>,
        mut skill_lines: Vec<String>,
        include_skills_usage_instructions: bool,
    ) -> Self {
        if include_skills_usage_instructions {
            skill_lines.push("### How to use skills".to_string());
            if let Some(instructions) = prompt_kind.alias_instructions() {
                skill_lines.push(instructions.to_string());
            }
            skill_lines.push(prompt_kind.usage_instructions().to_string());
        }
        Self {
            prompt_kind,
            skill_root_lines,
            skill_lines,
        }
    }
}

impl ContextualUserFragment for AvailableSkillsInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("skills.catalog".to_string())
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (SKILLS_INSTRUCTIONS_OPEN_TAG, SKILLS_INSTRUCTIONS_CLOSE_TAG)
    }

    fn body(&self) -> String {
        render_available_skills_body(self.prompt_kind, &self.skill_root_lines, &self.skill_lines)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SkillInstructions {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) contents: String,
    pub(crate) resource_access: Option<SkillResourceAccess>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SkillResourceAccess {
    pub(crate) authority: SkillToolAuthority,
    pub(crate) package: String,
    pub(crate) main_resource: String,
}

impl ContextualUserFragment for SkillInstructions {
    fn role(&self) -> &'static str {
        "user"
    }

    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("skills.selected_skill_instructions".to_string())
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
        let resource_access = self
            .resource_access
            .as_ref()
            .map(|access| {
                let metadata = serde_json::json!({
                    "authority": access.authority,
                    "package": access.package,
                    "main_resource": access.main_resource,
                });
                format!("\n<resource_access>{metadata}</resource_access>")
            })
            .unwrap_or_default();
        format!("\n<name>{name}</name>\n<path>{path}</path>{resource_access}\n{contents}\n")
    }
}
