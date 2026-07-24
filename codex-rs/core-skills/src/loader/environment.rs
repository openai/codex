use std::collections::HashMap;
use std::io;

use codex_exec_server::CapabilityRootDiscovery;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::protocol::Product;
pub use codex_skills::EnvironmentSkillMetadata;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;

use crate::model::SkillDependencies;
use crate::model::SkillPolicy;

use super::MAX_QUALIFIED_NAME_LEN;
use super::ParsedSkillFrontmatter;
use super::SkillMetadataFile;
use super::discovery::DirectorySymlinkPolicy;
use super::discovery::DiscoveredSkill;
use super::discovery::HiddenDirectoryPolicy;
use super::discovery::MAX_CONCURRENT_SKILL_LOADS;
use super::discovery::SkillDiscoveryOptions;
use super::discovery::SkillMetadataDiscovery;
use super::discovery::discover_skills;
use super::namespace::SkillNamespaceResolver;
use super::parse_skill_frontmatter_metadata_inner;
use super::resolve_dependencies;
use super::resolve_policy;
use super::sanitize_single_line;
use super::validate_len;

struct ParsedEnvironmentSkill {
    path_to_skills_md: PathUri,
    base_name: String,
    description: String,
    short_description: Option<String>,
    dependencies: Option<SkillDependencies>,
    policy: Option<SkillPolicy>,
}

/// Parsed executor skill plus the instructions already materialized by capability discovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentSkillSnapshot {
    pub metadata: EnvironmentSkillMetadata,
    pub instructions: String,
}

#[derive(Debug, Default)]
pub struct EnvironmentSkillSnapshotOutcome {
    pub skills: Vec<EnvironmentSkillSnapshot>,
    pub warnings: Vec<String>,
}

impl ParsedEnvironmentSkill {
    async fn load(
        file_system: &dyn ExecutorFileSystem,
        skill: &DiscoveredSkill,
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
            base_name,
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
#[tracing::instrument(
    name = "skills.environment.load",
    level = "info",
    skip_all,
    fields(skill_count = tracing::field::Empty)
)]
pub async fn load_environment_skills_from_root(
    file_system: &dyn ExecutorFileSystem,
    root: &PathUri,
    restriction_product: Option<Product>,
) -> EnvironmentSkillLoadOutcome {
    let mut outcome = EnvironmentSkillLoadOutcome::default();
    let discovery = discover_skills(
        file_system,
        root,
        // Preserve environment discovery behavior by following directory aliases and including
        // hidden directories exposed by the executor.
        SkillDiscoveryOptions {
            directory_symlinks: DirectorySymlinkPolicy::Follow,
            hidden_directories: HiddenDirectoryPolicy::Include,
            mode: codex_utils_plugins::SkillDiscoveryMode::Recursive,
        },
    )
    .await;
    tracing::Span::current().record("skill_count", discovery.skills.len());
    outcome.warnings.extend(discovery.warnings);
    if discovery.skills.is_empty() {
        return outcome;
    }

    let skill_paths = discovery
        .skills
        .iter()
        .map(|skill| skill.path.clone())
        .collect::<Vec<_>>();
    let namespace_resolver = SkillNamespaceResolver::discover(
        file_system,
        root,
        &skill_paths,
        discovery.plugin_roots,
        discovery.namespace_roots,
    );

    // Remote executors can multiplex these independent per-skill reads, so polling a bounded
    // number together allows the I/O for each skill and its metadata to happen concurrently.
    let skill_results = futures::stream::iter(discovery.skills)
        .map(|skill| {
            let path = skill.path.clone();
            async move {
                (
                    path,
                    ParsedEnvironmentSkill::load(file_system, &skill).await,
                )
            }
        })
        .buffered(MAX_CONCURRENT_SKILL_LOADS)
        .collect::<Vec<_>>();
    let (namespace_resolver, skill_results) = tokio::join!(namespace_resolver, skill_results);

    for (path, result) in skill_results {
        let result = result.and_then(|skill| {
            let name = namespace_resolver
                .for_skill(root, &skill.path_to_skills_md)
                .qualify(&skill.base_name);
            validate_len(&name, MAX_QUALIFIED_NAME_LEN, "qualified name")
                .map_err(|err| err.to_string())?;

            Ok(EnvironmentSkillMetadata {
                path_to_skills_md: skill.path_to_skills_md,
                name,
                description: skill.description,
                short_description: skill.short_description,
                dependencies: skill.dependencies,
                policy: skill.policy,
            })
        });
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

/// Parses an executor-produced manifest bundle without issuing additional filesystem requests.
pub fn load_environment_skills_from_discovery(
    discovery: &CapabilityRootDiscovery,
    restriction_product: Option<Product>,
) -> EnvironmentSkillSnapshotOutcome {
    let mut outcome = EnvironmentSkillSnapshotOutcome {
        warnings: discovery.warnings.clone(),
        ..Default::default()
    };
    if let Some(error) = &discovery.error {
        outcome.warnings.push(error.clone());
        return outcome;
    }

    let mut plugin_namespaces = HashMap::new();
    for (plugin_root, name) in discovery.namespace_manifests.iter().filter_map(|manifest| {
        #[derive(serde::Deserialize)]
        struct ManifestName {
            #[serde(default)]
            name: String,
        }

        let plugin_root = manifest.path.parent()?.parent()?;
        let parsed = serde_json::from_str::<ManifestName>(&manifest.contents).ok()?;
        let name = if parsed.name.trim().is_empty() {
            plugin_root.basename()?
        } else {
            parsed.name
        };
        Some((plugin_root, name))
    }) {
        // Exec-server orders manifests by the same precedence as local discovery. Preserve the
        // first manifest if an older or alternate server returns duplicates for one plugin root.
        plugin_namespaces.entry(plugin_root).or_insert(name);
    }

    for skill in &discovery.skills {
        let plugin_namespace =
            nearest_plugin_namespace(&skill.instructions.path, &plugin_namespaces);
        let ParsedSkillFrontmatter {
            name: base_name,
            description,
            short_description,
        } = match parse_skill_frontmatter_metadata_inner(&skill.instructions.contents, || {
            default_skill_name(&skill.instructions.path)
        }) {
            Ok(frontmatter) => frontmatter,
            Err(error) => {
                outcome.warnings.push(format!(
                    "Failed to load environment skill at {}: {error}",
                    skill.instructions.path
                ));
                continue;
            }
        };
        let name = plugin_namespace
            .map(|namespace| format!("{namespace}:{base_name}"))
            .unwrap_or(base_name);
        if let Err(error) = validate_len(&name, MAX_QUALIFIED_NAME_LEN, "qualified name") {
            outcome.warnings.push(format!(
                "Failed to load environment skill at {}: {error}",
                skill.instructions.path
            ));
            continue;
        }
        let (dependencies, policy) = skill
            .metadata
            .as_ref()
            .and_then(|metadata| {
                serde_yaml::from_str::<SkillMetadataFile>(&metadata.contents)
                    .map_err(|error| {
                        tracing::warn!(
                            path = %metadata.path,
                            "ignoring invalid discovered skill metadata: {error}"
                        );
                    })
                    .ok()
            })
            .map(|metadata| {
                (
                    resolve_dependencies(metadata.dependencies),
                    resolve_policy(metadata.policy),
                )
            })
            .unwrap_or((None, None));
        let metadata = EnvironmentSkillMetadata {
            path_to_skills_md: skill.instructions.path.clone(),
            name,
            description,
            short_description,
            dependencies,
            policy,
        };
        if metadata.matches_product_restriction(restriction_product) {
            outcome.skills.push(EnvironmentSkillSnapshot {
                metadata,
                instructions: skill.instructions.contents.clone(),
            });
        }
    }
    outcome.skills.sort_by(|left, right| {
        left.metadata.name.cmp(&right.metadata.name).then_with(|| {
            left.metadata
                .path_to_skills_md
                .to_string()
                .cmp(&right.metadata.path_to_skills_md.to_string())
        })
    });
    outcome
}

fn nearest_plugin_namespace<'a>(
    skill_path: &PathUri,
    plugin_namespaces: &'a HashMap<PathUri, String>,
) -> Option<&'a str> {
    let mut ancestor = skill_path.parent();
    while let Some(path) = ancestor {
        if let Some(namespace) = plugin_namespaces.get(&path) {
            return Some(namespace);
        }
        ancestor = path.parent();
    }
    None
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
