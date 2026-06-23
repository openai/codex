use std::io;

use codex_exec_server::ExecutorFileSystem;
use codex_protocol::protocol::Product;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::plugin_namespace_for_skill_uri;

use crate::model::SkillDependencies;
use crate::model::SkillPolicy;

use super::MAX_QUALIFIED_NAME_LEN;
use super::ParsedSkillFrontmatter;
use super::SKILLS_METADATA_DIR;
use super::SKILLS_METADATA_FILENAME;
use super::SkillMetadataFile;
use super::SymlinkPolicy;
use super::discover_skills_under_root;
use super::parse_skill_frontmatter_metadata_inner;
use super::resolve_dependencies;
use super::resolve_policy;
use super::sanitize_single_line;
use super::validate_len;

/// URI-native metadata for one skill owned by an execution environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentSkillMetadata {
    pub path_to_skills_md: PathUri,
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub dependencies: Option<SkillDependencies>,
    pub policy: Option<SkillPolicy>,
}

impl EnvironmentSkillMetadata {
    pub fn allows_implicit_invocation(&self) -> bool {
        self.policy
            .as_ref()
            .and_then(|policy| policy.allow_implicit_invocation)
            .unwrap_or(true)
    }

    fn matches_product_restriction(&self, restriction_product: Option<Product>) -> bool {
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

    async fn parse(file_system: &dyn ExecutorFileSystem, path: &PathUri) -> Result<Self, String> {
        let contents = file_system
            .read_file_text(path, /*sandbox*/ None)
            .await
            .map_err(|err| format!("failed to read file: {err}"))?;
        let ParsedSkillFrontmatter {
            name: base_name,
            description,
            short_description,
        } = parse_skill_frontmatter_metadata_inner(&contents, || default_skill_name(path))
            .map_err(|err| err.to_string())?;
        let name = plugin_namespace_for_skill_uri(file_system, path)
            .await
            .map(|namespace| format!("{namespace}:{base_name}"))
            .unwrap_or(base_name);
        validate_len(&name, MAX_QUALIFIED_NAME_LEN, "qualified name")
            .map_err(|err| err.to_string())?;
        let (dependencies, policy) = load_skill_metadata(file_system, path).await;

        Ok(Self {
            path_to_skills_md: path.clone(),
            name,
            description,
            short_description,
            dependencies,
            policy,
        })
    }
}

#[derive(Debug, Default)]
pub struct EnvironmentSkillLoadOutcome {
    pub skills: Vec<EnvironmentSkillMetadata>,
    pub warnings: Vec<String>,
}

/// Discovers skills without converting environment-owned paths to host paths.
pub async fn load_environment_skills_from_root(
    file_system: &dyn ExecutorFileSystem,
    root: &PathUri,
    restriction_product: Option<Product>,
) -> EnvironmentSkillLoadOutcome {
    let mut outcome = EnvironmentSkillLoadOutcome::default();
    let discovery =
        discover_skills_under_root(file_system, root, SymlinkPolicy::FollowDirectories).await;
    outcome.warnings.extend(discovery.warnings);
    for path in discovery.skill_files {
        match EnvironmentSkillMetadata::parse(file_system, &path).await {
            Ok(skill) if skill.matches_product_restriction(restriction_product) => {
                outcome.skills.push(skill);
            }
            Ok(_) => {}
            Err(message) => outcome.warnings.push(format!(
                "Failed to load environment skill at {path}: {message}"
            )),
        }
    }
    outcome.skills.sort_by(|left, right| {
        left.name.cmp(&right.name).then_with(|| {
            left.path_to_skills_md
                .to_string()
                .cmp(&right.path_to_skills_md.to_string())
        })
    });
    outcome
}

async fn load_skill_metadata(
    file_system: &dyn ExecutorFileSystem,
    skill_path: &PathUri,
) -> (Option<SkillDependencies>, Option<SkillPolicy>) {
    let Some(skill_dir) = skill_path.parent() else {
        return (None, None);
    };
    let Ok(metadata_path) =
        skill_dir.join(&format!("{SKILLS_METADATA_DIR}/{SKILLS_METADATA_FILENAME}"))
    else {
        return (None, None);
    };
    match file_system
        .get_metadata(&metadata_path, /*sandbox*/ None)
        .await
    {
        Ok(metadata) if metadata.is_file => {}
        Ok(_) => return (None, None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return (None, None),
        Err(error) => {
            tracing::warn!("ignoring {metadata_path}: failed to stat metadata: {error}");
            return (None, None);
        }
    }
    let contents = match file_system
        .read_file_text(&metadata_path, /*sandbox*/ None)
        .await
    {
        Ok(contents) => contents,
        Err(error) => {
            tracing::warn!("ignoring {metadata_path}: failed to read metadata: {error}");
            return (None, None);
        }
    };
    let parsed: SkillMetadataFile = match serde_yaml::from_str(&contents) {
        Ok(parsed) => parsed,
        Err(error) => {
            tracing::warn!("ignoring {metadata_path}: invalid metadata: {error}");
            return (None, None);
        }
    };

    (
        resolve_dependencies(parsed.dependencies),
        resolve_policy(parsed.policy),
    )
}

fn default_skill_name(path: &PathUri) -> String {
    path.parent()
        .and_then(|parent| parent.basename())
        .map(|name| sanitize_single_line(&name))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "skill".to_string())
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
