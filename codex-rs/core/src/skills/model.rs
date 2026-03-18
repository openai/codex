use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct SkillManagedNetworkOverride {
    pub allowed_domains: Option<Vec<String>>,
    pub denied_domains: Option<Vec<String>>,
}

impl SkillManagedNetworkOverride {
    pub fn has_domain_overrides(&self) -> bool {
        self.allowed_domains.is_some() || self.denied_domains.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub interface: Option<SkillInterface>,
    pub dependencies: Option<SkillDependencies>,
    pub policy: Option<SkillPolicy>,
    pub permission_profile: Option<PermissionProfile>,
    pub managed_network_override: Option<SkillManagedNetworkOverride>,
    /// Path to the SKILLS.md file that declares this skill.
    pub path_to_skills_md: PathBuf,
    pub scope: SkillScope,
}

impl SkillMetadata {
    fn allow_implicit_invocation(&self) -> bool {
        self.policy
            .as_ref()
            .and_then(|policy| policy.allow_implicit_invocation)
            .unwrap_or(true)
    }

    pub fn matches_product_restriction(&self, session_source: &SessionSource) -> bool {
        match &self.policy {
            Some(policy) => session_source.matches_product_restriction(&policy.products),
            None => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillPolicy {
    pub allow_implicit_invocation: Option<bool>,
    pub products: Vec<Product>,
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
    pub(crate) implicit_skills_by_scripts_dir: Arc<HashMap<PathBuf, SkillMetadata>>,
    pub(crate) implicit_skills_by_doc_path: Arc<HashMap<PathBuf, SkillMetadata>>,
}

impl SkillLoadOutcome {
    pub fn is_skill_enabled(&self, skill: &SkillMetadata) -> bool {
        !self.disabled_paths.contains(&skill.path_to_skills_md)
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

pub fn filter_skill_load_outcome_for_session_source(
    mut outcome: SkillLoadOutcome,
    session_source: &SessionSource,
) -> SkillLoadOutcome {
    outcome
        .skills
        .retain(|skill| skill.matches_product_restriction(session_source));
    outcome.implicit_skills_by_scripts_dir = Arc::new(
        outcome
            .implicit_skills_by_scripts_dir
            .iter()
            .filter(|(_, skill)| skill.matches_product_restriction(session_source))
            .map(|(path, skill)| (path.clone(), skill.clone()))
            .collect(),
    );
    outcome.implicit_skills_by_doc_path = Arc::new(
        outcome
            .implicit_skills_by_doc_path
            .iter()
            .filter(|(_, skill)| skill.matches_product_restriction(session_source))
            .map(|(path, skill)| (path.clone(), skill.clone()))
            .collect(),
    );
    outcome
}

pub fn filter_skills_for_session_source(
    skills: Vec<SkillMetadata>,
    session_source: &SessionSource,
) -> Vec<SkillMetadata> {
    skills
        .into_iter()
        .filter(|skill| skill.matches_product_restriction(session_source))
        .collect()
}
