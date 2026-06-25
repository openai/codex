use std::collections::HashMap;
use std::collections::HashSet;
use std::io;

use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::WalkEntryKind;
use codex_exec_server::WalkOptions;
use codex_protocol::protocol::Product;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::DISCOVERABLE_PLUGIN_MANIFEST_PATHS;
use codex_utils_plugins::plugin_namespace_for_root_uri;
use codex_utils_plugins::plugin_namespace_for_skill_uri;
use futures::StreamExt;
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
use super::SkillFileDiscovery;
use super::SkillMetadataFile;
use super::parse_skill_frontmatter_metadata_inner;
use super::resolve_dependencies;
use super::resolve_policy;
use super::sanitize_single_line;
use super::validate_len;

const MAX_SKILLS_ENTRIES_PER_ROOT: usize = 20_000;
const MAX_CONCURRENT_SKILL_LOADS: usize = 64;

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

    async fn parse(
        file_system: &dyn ExecutorFileSystem,
        path: &PathUri,
        plugin_namespace: Option<&str>,
    ) -> Result<Self, String> {
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
        let name = plugin_namespace
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
    let discovery = match file_system
        .walk(
            root,
            WalkOptions {
                max_depth: MAX_SCAN_DEPTH,
                max_directories: MAX_SKILLS_DIRS_PER_ROOT,
                max_entries: MAX_SKILLS_ENTRIES_PER_ROOT,
                follow_directory_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await
    {
        Ok(walk) => {
            let mut warnings = walk
                .errors
                .into_iter()
                .map(|error| {
                    format!(
                        "failed to scan skill path {}: {}",
                        error.path, error.message
                    )
                })
                .collect::<Vec<_>>();
            if walk.truncated {
                warnings.push(format!(
                    "skills scan reached its traversal limit (root: {root})"
                ));
            }
            let mut skill_files = Vec::new();
            let mut plugin_roots = HashSet::new();
            for entry in walk.entries {
                match entry.kind {
                    WalkEntryKind::Directory => {
                        if DISCOVERABLE_PLUGIN_MANIFEST_PATHS
                            .iter()
                            .any(|path| path.split('/').next() == entry.path.basename().as_deref())
                            && let Some(plugin_root) = entry.path.parent()
                        {
                            plugin_roots.insert(plugin_root);
                        }
                    }
                    WalkEntryKind::File => {
                        if entry.path.basename().as_deref() == Some(SKILLS_FILENAME) {
                            skill_files.push(entry.path);
                        }
                    }
                }
            }
            SkillFileDiscovery {
                skill_files,
                plugin_roots,
                namespace_roots: HashSet::from([root.clone()]),
                warnings,
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => SkillFileDiscovery {
            skill_files: Vec::new(),
            plugin_roots: HashSet::new(),
            namespace_roots: HashSet::new(),
            warnings: Vec::new(),
        },
        Err(error) => SkillFileDiscovery {
            skill_files: Vec::new(),
            plugin_roots: HashSet::new(),
            namespace_roots: HashSet::new(),
            warnings: vec![format!("failed to walk skills root {root}: {error:#}")],
        },
    };
    outcome.warnings.extend(discovery.warnings);
    if discovery.skill_files.is_empty() {
        return outcome;
    }

    let mut skill_ancestors = HashSet::new();
    for skill_path in &discovery.skill_files {
        let mut ancestor = skill_path.parent();
        while let Some(path) = ancestor {
            skill_ancestors.insert(path.clone());
            ancestor = path.parent();
        }
    }

    let namespace_roots = discovery.namespace_roots;
    let namespace_lookups = join_all(namespace_roots.iter().map(|namespace_root| async {
        (
            namespace_root.clone(),
            plugin_namespace_for_skill_uri(file_system, namespace_root).await,
        )
    }))
    .await;
    let plugin_lookups = join_all(
        discovery
            .plugin_roots
            .iter()
            .filter(|plugin_root| skill_ancestors.contains(*plugin_root))
            .filter(|plugin_root| !namespace_roots.contains(*plugin_root))
            .map(|plugin_root| async {
                (
                    plugin_root.clone(),
                    plugin_namespace_for_root_uri(file_system, plugin_root).await,
                )
            }),
    )
    .await;
    let plugin_namespaces = namespace_lookups
        .into_iter()
        .chain(plugin_lookups)
        .filter_map(|(plugin_root, namespace)| namespace.map(|namespace| (plugin_root, namespace)))
        .collect::<HashMap<_, _>>();

    // Remote executors can multiplex these independent per-skill reads, so polling a bounded
    // number together allows the I/O for each skill and its metadata to happen concurrently.
    let skill_results = futures::stream::iter(discovery.skill_files)
        .map(|path| {
            let mut ancestor = path.parent();
            let plugin_namespace = loop {
                let Some(current) = ancestor else {
                    break None;
                };
                if let Some(namespace) = plugin_namespaces.get(&current) {
                    break Some(namespace.as_str());
                }
                ancestor = current.parent();
            };
            async move {
                let result =
                    EnvironmentSkillMetadata::parse(file_system, &path, plugin_namespace).await;
                (path, result)
            }
        })
        .buffered(MAX_CONCURRENT_SKILL_LOADS)
        .collect::<Vec<_>>()
        .await;

    for (path, result) in skill_results {
        match result {
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
