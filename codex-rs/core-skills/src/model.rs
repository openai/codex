use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use codex_exec_server::EnvironmentPathRef;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub interface: Option<SkillInterface>,
    pub dependencies: Option<SkillDependencies>,
    pub policy: Option<SkillPolicy>,
    /// Best-effort resolved raw path used for display, wire payloads, and config selectors.
    pub path_to_skills_md: AbsolutePathBuf,
    /// Same resolved path bound to the filesystem that loaded this skill.
    pub source_path: EnvironmentPathRef,
    pub scope: SkillScope,
    pub plugin_id: Option<String>,
}

impl SkillMetadata {
    fn allow_implicit_invocation(&self) -> bool {
        self.policy
            .as_ref()
            .and_then(|policy| policy.allow_implicit_invocation)
            .unwrap_or(true)
    }

    pub fn matches_product_restriction_for_product(
        &self,
        restriction_product: Option<Product>,
    ) -> bool {
        match &self.policy {
            Some(policy) => {
                policy.products.is_empty()
                    || restriction_product.is_some_and(|product| {
                        product.matches_product_restriction(&policy.products)
                    })
            }
            None => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillPolicy {
    pub allow_implicit_invocation: Option<bool>,
    // TODO: Enforce product gating in Codex skill selection/injection instead of only parsing and
    // storing this metadata.
    pub products: Vec<Product>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInterface {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub icon_small: Option<AbsolutePathBuf>,
    pub icon_large: Option<AbsolutePathBuf>,
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
    pub path: AbsolutePathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct SkillLoadOutcome {
    pub skills: Vec<SkillMetadata>,
    pub errors: Vec<SkillError>,
    pub disabled_paths: HashSet<EnvironmentPathRef>,
    pub(crate) skill_roots: Vec<EnvironmentPathRef>,
    pub(crate) skill_root_by_path: Arc<HashMap<EnvironmentPathRef, EnvironmentPathRef>>,
    pub(crate) implicit_skills_by_scripts_dir: Arc<HashMap<EnvironmentPathRef, SkillMetadata>>,
    pub(crate) implicit_skills_by_doc_path: Arc<HashMap<EnvironmentPathRef, SkillMetadata>>,
}

impl SkillLoadOutcome {
    pub fn is_skill_enabled(&self, skill: &SkillMetadata) -> bool {
        !self.disabled_paths.contains(&skill.source_path)
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

    pub fn disabled_path_values(&self) -> HashSet<AbsolutePathBuf> {
        self.disabled_paths
            .iter()
            .map(|path| path.path().clone())
            .collect()
    }
}

pub fn filter_skill_load_outcome_for_product(
    mut outcome: SkillLoadOutcome,
    restriction_product: Option<Product>,
) -> SkillLoadOutcome {
    outcome
        .skills
        .retain(|skill| skill.matches_product_restriction_for_product(restriction_product));
    let retained_paths: HashSet<EnvironmentPathRef> = outcome
        .skills
        .iter()
        .map(|skill| skill.source_path.clone())
        .collect();
    outcome.skill_root_by_path = Arc::new(
        outcome
            .skill_root_by_path
            .iter()
            .filter(|(path, _)| retained_paths.contains(*path))
            .map(|(path, root)| (path.clone(), root.clone()))
            .collect(),
    );
    let retained_roots: HashSet<EnvironmentPathRef> =
        outcome.skill_root_by_path.values().cloned().collect();
    outcome
        .skill_roots
        .retain(|root| retained_roots.contains(root));
    outcome.implicit_skills_by_scripts_dir = Arc::new(
        outcome
            .implicit_skills_by_scripts_dir
            .iter()
            .filter(|(_, skill)| skill.matches_product_restriction_for_product(restriction_product))
            .map(|(path, skill)| (path.clone(), skill.clone()))
            .collect(),
    );
    outcome.implicit_skills_by_doc_path = Arc::new(
        outcome
            .implicit_skills_by_doc_path
            .iter()
            .filter(|(_, skill)| skill.matches_product_restriction_for_product(restriction_product))
            .map(|(path, skill)| (path.clone(), skill.clone()))
            .collect(),
    );
    outcome
}
