use std::collections::HashSet;
use std::path::PathBuf;

use codex_protocol::protocol::SkillScope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub interface: Option<SkillInterface>,
    pub dependencies: Option<SkillDependencies>,
    pub policy: Option<SkillPolicy>,
    pub path: PathBuf,
    pub scope: SkillScope,
}

impl SkillMetadata {
    fn allow_implicit_invocation(&self) -> bool {
        self.policy
            .as_ref()
            .and_then(|policy| policy.allow_implicit_invocation)
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillPolicy {
    pub allow_implicit_invocation: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInterface {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub icon_small: Option<PathBuf>,
    pub icon_large: Option<PathBuf>,
    pub brand_color: Option<String>,
    pub default_prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDependencies {
    pub tools: Vec<SkillToolDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillToolDependency {
    pub r#type: String,
    pub value: String,
    pub description: Option<String>,
    pub transport: Option<String>,
    pub command: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillError {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct SkillLoadOutcome {
    pub skills: Vec<SkillMetadata>,
    pub errors: Vec<SkillError>,
    pub disabled_paths: HashSet<PathBuf>,
}

impl SkillLoadOutcome {
    pub fn is_skill_enabled(&self, skill: &SkillMetadata) -> bool {
        !self.disabled_paths.contains(&skill.path)
    }

    pub fn is_skill_allowed_for_implicit_invocation(&self, skill: &SkillMetadata) -> bool {
        self.is_skill_enabled(skill) && skill.allow_implicit_invocation()
    }

    pub fn allowed_skills_for_implicit_invocation(&self) -> Vec<SkillMetadata> {
        self.skills
            .iter()
            .filter(|skill| self.is_skill_allowed_for_implicit_invocation(skill))
            .cloned()
            .collect()
    }

    pub fn skills_with_enabled(&self) -> impl Iterator<Item = (&SkillMetadata, bool)> {
        self.skills
            .iter()
            .map(|skill| (skill, self.is_skill_enabled(skill)))
    }
}
