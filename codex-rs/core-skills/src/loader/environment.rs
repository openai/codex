use std::collections::HashSet;
use std::collections::VecDeque;
use std::io;

use codex_exec_server::ExecutorFileSystem;
use codex_protocol::protocol::Product;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::plugin_namespace_for_skill_uri;
use futures::future::join_all;

use crate::model::SkillDependencies;
use crate::model::SkillPolicy;

use super::MAX_QUALIFIED_NAME_LEN;
use super::MAX_SCAN_DEPTH;
use super::MAX_SKILLS_DIRS_PER_ROOT;
use super::ParsedSkillFrontmatter;
use super::SKILLS_FILENAME;
use super::SKILLS_METADATA_DIR;
use super::SKILLS_METADATA_FILENAME;
use super::SkillMetadataFile;
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
    let root = canonicalize_for_skill_identity(file_system, root).await;
    match file_system.get_metadata(&root, /*sandbox*/ None).await {
        Ok(metadata) if metadata.is_directory => {}
        Ok(_) => return outcome,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return outcome,
        Err(err) => {
            outcome.warnings.push(format!(
                "Failed to load environment skills at {root}: {err}"
            ));
            return outcome;
        }
    }

    let mut visited_dirs = HashSet::from([root.clone()]);
    let mut queue = VecDeque::from([(root.clone(), 0)]);
    let mut truncated_by_dir_limit = false;

    while let Some((dir, depth)) = queue.pop_front() {
        let entries = match file_system.read_directory(&dir, /*sandbox*/ None).await {
            Ok(entries) => entries,
            Err(err) => {
                outcome.warnings.push(format!(
                    "Failed to read environment skills dir {dir}: {err}"
                ));
                continue;
            }
        };

        let paths = entries
            .into_iter()
            .filter_map(|entry| {
                let file_name = entry.file_name;
                if file_name.starts_with('.') {
                    return None;
                }
                match dir.join(&file_name) {
                    Ok(path) => Some((file_name, path)),
                    Err(err) => {
                        outcome.warnings.push(format!(
                            "Failed to resolve environment skill path {dir}/{file_name}: {err}"
                        ));
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        let metadata_results = join_all(
            paths
                .iter()
                .map(|(_, path)| file_system.get_metadata(path, /*sandbox*/ None)),
        )
        .await;

        for ((file_name, path), metadata_result) in paths.into_iter().zip(metadata_results) {
            let metadata = match metadata_result {
                Ok(metadata) => metadata,
                Err(err) => {
                    outcome.warnings.push(format!(
                        "Failed to stat environment skill path {path}: {err}"
                    ));
                    continue;
                }
            };

            if metadata.is_directory {
                enqueue_dir(
                    file_system,
                    &mut queue,
                    &mut visited_dirs,
                    &mut truncated_by_dir_limit,
                    path,
                    depth + 1,
                )
                .await;
            } else if metadata.is_file && file_name == SKILLS_FILENAME {
                match parse_environment_skill_file(file_system, &path).await {
                    Ok(skill) if skill.matches_product_restriction(restriction_product) => {
                        outcome.skills.push(skill);
                    }
                    Ok(_) => {}
                    Err(message) => outcome.warnings.push(format!(
                        "Failed to load environment skill at {path}: {message}"
                    )),
                }
            }
        }
    }

    if truncated_by_dir_limit {
        tracing::warn!(
            "environment skills scan truncated after {} directories (root: {})",
            MAX_SKILLS_DIRS_PER_ROOT,
            root
        );
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

async fn enqueue_dir(
    file_system: &dyn ExecutorFileSystem,
    queue: &mut VecDeque<(PathUri, usize)>,
    visited_dirs: &mut HashSet<PathUri>,
    truncated_by_dir_limit: &mut bool,
    path: PathUri,
    depth: usize,
) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    if visited_dirs.len() >= MAX_SKILLS_DIRS_PER_ROOT {
        *truncated_by_dir_limit = true;
        return;
    }
    let path = canonicalize_for_skill_identity(file_system, &path).await;
    if visited_dirs.insert(path.clone()) {
        queue.push_back((path, depth));
    }
}

async fn parse_environment_skill_file(
    file_system: &dyn ExecutorFileSystem,
    path: &PathUri,
) -> Result<EnvironmentSkillMetadata, String> {
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
    validate_len(&name, MAX_QUALIFIED_NAME_LEN, "qualified name").map_err(|err| err.to_string())?;
    let (dependencies, policy) = load_skill_metadata(file_system, path).await;
    let path_to_skills_md = canonicalize_for_skill_identity(file_system, path).await;

    Ok(EnvironmentSkillMetadata {
        path_to_skills_md,
        name,
        description,
        short_description,
        dependencies,
        policy,
    })
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

async fn canonicalize_for_skill_identity(
    file_system: &dyn ExecutorFileSystem,
    path: &PathUri,
) -> PathUri {
    file_system
        .canonicalize(path, /*sandbox*/ None)
        .await
        .unwrap_or_else(|_| path.clone())
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
