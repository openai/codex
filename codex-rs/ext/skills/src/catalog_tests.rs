//! Tests partial Cloud reconciliation without changing source-specific enablement.

use super::*;
use pretty_assertions::assert_eq;

fn skill(kind: SkillSourceKind, name: &str) -> SkillCatalogEntry {
    SkillCatalogEntry::new(
        SkillPackageId(format!("skill://{kind}/{name}")),
        SkillAuthority::new(kind.clone(), kind.to_string()),
        name,
        "",
        SkillResourceId::new(format!("skill://{kind}/{name}/SKILL.md")),
    )
    .with_remote_plugin_id(Some("plugin_a".to_string()))
}

#[test]
fn partial_cloud_merges_matches_without_disabling_unmatched_installations() {
    for (cloud_is_partial, cloud_has_match) in [
        (None, false),
        (Some(false), false),
        (Some(true), false),
        (Some(false), true),
        (Some(true), true),
    ] {
        for executor_is_partial in [false, true] {
            let cloud_entry = skill(SkillSourceKind::Cloud, "matched");
            let matched = skill(SkillSourceKind::Executor, "matched").disabled();
            let mut unmatched = skill(SkillSourceKind::Executor, "unmatched");
            let mut denied_source = unmatched.primary_source();
            denied_source.enabled = false;
            unmatched.sources.push(denied_source);
            let denied = skill(SkillSourceKind::Executor, "denied").disabled();
            let mut executor = SkillCatalog::default();
            executor.extend(SkillCatalog {
                entries: vec![matched.clone(), unmatched.clone(), denied.clone()],
                is_partial: executor_is_partial,
                ..Default::default()
            });
            // Appending a complete provider must not clear an earlier partial result.
            executor.extend(SkillCatalog::default());
            let cloud = cloud_is_partial.map(|is_partial| SkillCatalog {
                entries: if cloud_has_match {
                    vec![cloud_entry.clone()]
                } else {
                    Vec::new()
                },
                is_partial,
                // Diagnostic warnings alone do not make membership incomplete.
                warnings: vec!["diagnostic".to_string()],
            });
            let expected_match = if cloud_has_match {
                let mut entry = cloud_entry;
                entry.sources.push(matched.primary_source());
                entry
            } else {
                matched
            };
            if cloud_is_partial == Some(false) {
                unmatched.enabled = false;
            }
            assert_eq!(
                merge_catalogs(cloud, executor),
                SkillCatalog {
                    entries: vec![expected_match, unmatched, denied],
                    warnings: if cloud_is_partial.is_some() {
                        vec!["diagnostic".to_string()]
                    } else {
                        Vec::new()
                    },
                    is_partial: cloud_is_partial.unwrap_or(false) || executor_is_partial,
                },
                "cloud_is_partial={cloud_is_partial:?}, cloud_has_match={cloud_has_match}, executor_is_partial={executor_is_partial}",
            );
        }
    }
}
