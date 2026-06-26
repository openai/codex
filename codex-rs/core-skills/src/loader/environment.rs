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
use super::SkillMetadataFile;
use super::parse_skill_frontmatter_metadata_inner;
use super::resolve_dependencies;
use super::resolve_policy;
use super::sanitize_single_line;
use super::validate_len;

const MAX_SKILLS_ENTRIES_PER_ROOT: usize = 20_000;
const MAX_CONCURRENT_SKILL_LOADS: usize = 64;

struct EnvironmentSkillDiscovery {
    skills: Vec<DiscoveredEnvironmentSkill>,
    plugin_roots: HashSet<PathUri>,
    namespace_roots: HashSet<PathUri>,
    warnings: Vec<String>,
}

struct DiscoveredEnvironmentSkill {
    path: PathUri,
    metadata: SkillMetadataDiscovery,
}

enum SkillMetadataDiscovery {
    Present(PathUri),
    Absent,
    Probe(PathUri),
}

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
        skill: &DiscoveredEnvironmentSkill,
        plugin_namespace: Option<&str>,
    ) -> Result<Self, String> {
        let (contents, discovered_metadata) = match &skill.metadata {
            SkillMetadataDiscovery::Present(metadata_path) => {
                let (contents, metadata) = tokio::join!(
                    read_skill_contents(file_system, &skill.path),
                    read_skill_metadata(file_system, metadata_path),
                );
                (contents?, metadata)
            }
            SkillMetadataDiscovery::Absent | SkillMetadataDiscovery::Probe(_) => (
                read_skill_contents(file_system, &skill.path).await?,
                (None, None),
            ),
        };
        let ParsedSkillFrontmatter {
            name: base_name,
            description,
            short_description,
        } = parse_skill_frontmatter_metadata_inner(&contents, || default_skill_name(&skill.path))
            .map_err(|err| err.to_string())?;
        let name = plugin_namespace
            .map(|namespace| format!("{namespace}:{base_name}"))
            .unwrap_or(base_name);
        validate_len(&name, MAX_QUALIFIED_NAME_LEN, "qualified name")
            .map_err(|err| err.to_string())?;
        let (dependencies, policy) = match &skill.metadata {
            SkillMetadataDiscovery::Present(_) | SkillMetadataDiscovery::Absent => {
                discovered_metadata
            }
            SkillMetadataDiscovery::Probe(metadata_path) => {
                probe_skill_metadata(file_system, metadata_path).await
            }
        };

        Ok(Self {
            path_to_skills_md: skill.path.clone(),
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
            let inventory_complete = !walk.truncated && walk.errors.is_empty();
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
            let mut file_paths = HashSet::new();
            let mut directory_paths = HashSet::new();
            let mut plugin_roots = HashSet::new();
            for entry in walk.entries {
                match entry.kind {
                    WalkEntryKind::Directory => {
                        directory_paths.insert(entry.path.clone());
                        if DISCOVERABLE_PLUGIN_MANIFEST_PATHS
                            .iter()
                            .any(|path| path.split('/').next() == entry.path.basename().as_deref())
                            && let Some(plugin_root) = entry.path.parent()
                        {
                            plugin_roots.insert(plugin_root);
                        }
                    }
                    WalkEntryKind::File => {
                        file_paths.insert(entry.path.clone());
                        if entry.path.basename().as_deref() == Some(SKILLS_FILENAME) {
                            skill_files.push(entry.path);
                        }
                    }
                }
            }
            let skills = skill_files
                .into_iter()
                .map(|path| DiscoveredEnvironmentSkill {
                    metadata: discover_skill_metadata(
                        &path,
                        &file_paths,
                        &directory_paths,
                        inventory_complete,
                    ),
                    path,
                })
                .collect();
            EnvironmentSkillDiscovery {
                skills,
                plugin_roots,
                namespace_roots: HashSet::from([root.clone()]),
                warnings,
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => EnvironmentSkillDiscovery {
            skills: Vec::new(),
            plugin_roots: HashSet::new(),
            namespace_roots: HashSet::new(),
            warnings: Vec::new(),
        },
        Err(error) => EnvironmentSkillDiscovery {
            skills: Vec::new(),
            plugin_roots: HashSet::new(),
            namespace_roots: HashSet::new(),
            warnings: vec![format!("failed to walk skills root {root}: {error:#}")],
        },
    };
    outcome.warnings.extend(discovery.warnings);
    if discovery.skills.is_empty() {
        return outcome;
    }

    let mut skill_ancestors = HashSet::new();
    for skill in &discovery.skills {
        let mut ancestor = skill.path.parent();
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
    let skill_results = futures::stream::iter(discovery.skills)
        .map(|skill| {
            let mut ancestor = skill.path.parent();
            let plugin_namespace = loop {
                let Some(current) = ancestor else {
                    break None;
                };
                if let Some(namespace) = plugin_namespaces.get(&current) {
                    break Some(namespace.as_str());
                }
                ancestor = current.parent();
            };
            let path = skill.path.clone();
            async move {
                let result =
                    EnvironmentSkillMetadata::parse(file_system, &skill, plugin_namespace).await;
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

fn discover_skill_metadata(
    skill_path: &PathUri,
    file_paths: &HashSet<PathUri>,
    directory_paths: &HashSet<PathUri>,
    inventory_complete: bool,
) -> SkillMetadataDiscovery {
    let Some(skill_dir) = skill_path.parent() else {
        return SkillMetadataDiscovery::Absent;
    };
    let Ok(metadata_dir) = skill_dir.join(SKILLS_METADATA_DIR) else {
        return SkillMetadataDiscovery::Absent;
    };
    let Ok(metadata_path) = metadata_dir.join(SKILLS_METADATA_FILENAME) else {
        return SkillMetadataDiscovery::Absent;
    };
    if file_paths.contains(&metadata_path) {
        SkillMetadataDiscovery::Present(metadata_path)
    } else if inventory_complete && !directory_paths.contains(&metadata_dir) {
        SkillMetadataDiscovery::Absent
    } else {
        // The walk can omit entries after an error or traversal limit. It also omits file
        // symlinks, so keep the existing probe when the metadata directory itself was observed.
        SkillMetadataDiscovery::Probe(metadata_path)
    }
}

async fn read_skill_contents(
    file_system: &dyn ExecutorFileSystem,
    skill_path: &PathUri,
) -> Result<String, String> {
    file_system
        .read_file_text(skill_path, /*sandbox*/ None)
        .await
        .map_err(|err| format!("failed to read file: {err}"))
}

async fn probe_skill_metadata(
    file_system: &dyn ExecutorFileSystem,
    metadata_path: &PathUri,
) -> (Option<SkillDependencies>, Option<SkillPolicy>) {
    match file_system
        .get_metadata(metadata_path, /*sandbox*/ None)
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
    read_skill_metadata(file_system, metadata_path).await
}

async fn read_skill_metadata(
    file_system: &dyn ExecutorFileSystem,
    metadata_path: &PathUri,
) -> (Option<SkillDependencies>, Option<SkillPolicy>) {
    let contents = match file_system
        .read_file_text(metadata_path, /*sandbox*/ None)
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
