use std::collections::HashMap;

use codex_exec_server::CapabilityRootDiscovery;
use codex_protocol::protocol::Product;
use codex_skills::EnvironmentSkillMetadata;
use codex_skills::ParsedSkillFrontmatter;
use codex_skills::parse_skill_frontmatter_metadata;
use codex_utils_path_uri::PathUri;

use super::MAX_QUALIFIED_NAME_LEN;
use super::metadata::SkillMetadataFile;
use super::metadata::resolve_dependencies;
use super::metadata::resolve_policy;
use super::metadata::sanitize_single_line;
use super::metadata::validate_len;

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
        } = match parse_skill_frontmatter_metadata(&skill.instructions.contents, || {
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
