//! Projects executor-owned discovery metadata into skill catalogs without reopening host paths.

use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_exec_server::ExecutorSkill;
use codex_protocol::protocol::SkillScope;
use codex_skills::EnvironmentSkillMetadata;
use codex_utils_path_uri::PathUri;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillSourceKind;

use super::ExecutorSkillProvider;
use super::catalog_entry_from_skill;

impl ExecutorSkillProvider {
    pub(super) fn list_from_discovery_v2(
        &self,
        snapshots: &[ExecutorCapabilityDiscoveryV2Snapshot],
    ) -> SkillCatalog {
        let mut catalog = SkillCatalog::default();
        for snapshot in snapshots {
            let discovered = match &snapshot.result {
                Ok(discovered) => discovered,
                Err(error) => {
                    catalog.is_partial = true;
                    catalog.warnings.push(format!(
                        "Environment `{}` discovery failed: {error}",
                        snapshot.environment_id
                    ));
                    continue;
                }
            };
            catalog.warnings.extend(discovered.warnings.clone());
            for plugin in &discovered.plugins {
                for skill in &plugin.skills {
                    let Some(mut entry) = self.catalog_entry_from_discovered_skill(
                        skill,
                        &plugin.id,
                        &plugin.root,
                        &snapshot.environment_id,
                    ) else {
                        continue;
                    };
                    entry.remote_plugin_id = plugin.remote_plugin_id.clone();
                    entry.plugin_id = Some(plugin.id.clone());
                    entry.analytics_scope = Some(SkillScope::User);
                    catalog.push_entry(entry);
                }
            }
            for skill in &discovered.skills {
                // Use the skills namespace when discovery-root metadata is absent.
                let fallback_root;
                let (root_id, root_path) = if let Some(root) = &skill.discovery_root {
                    (root.id.as_str(), &root.path)
                } else {
                    let Some(parent) = skill.path.parent() else {
                        continue;
                    };
                    fallback_root = parent;
                    ("skills", &fallback_root)
                };
                if let Some(entry) = self.catalog_entry_from_discovered_skill(
                    skill,
                    root_id,
                    root_path,
                    &snapshot.environment_id,
                ) {
                    catalog.push_entry(entry);
                }
            }
        }
        catalog
    }

    fn catalog_entry_from_discovered_skill(
        &self,
        skill: &ExecutorSkill,
        root_id: &str,
        root: &PathUri,
        environment_id: &str,
    ) -> Option<SkillCatalogEntry> {
        let metadata = EnvironmentSkillMetadata {
            path_to_skills_md: skill.path.clone(),
            name: skill.name.clone(),
            description: skill.description.clone(),
            short_description: skill.short_description.clone(),
            dependencies: skill.dependencies.clone(),
            policy: skill.policy.clone(),
        };
        if !metadata.matches_product_restriction(self.restriction_product) {
            return None;
        }
        Some(catalog_entry_from_skill(
            &metadata,
            SkillAuthority::new(SkillSourceKind::Executor, root_id),
            root_id,
            root,
            environment_id,
            /*instructions*/ None,
            &self.disabled_skill_paths,
        ))
    }
}
