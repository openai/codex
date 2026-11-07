#![cfg(test)]

//! Checks managed feature constraints while applying isolated reviewer settings.

use super::reviewer_config::build_reviewer_config;
use codex_config::FeatureRequirementsToml;
use codex_config::RequirementSource;
use codex_config::Sourced;
use codex_core::config::ManagedFeatures;
use codex_features::Feature;
use std::collections::BTreeMap;

#[tokio::test]
async fn guardian_review_session_config_allows_pinned_disabled_feature() {
    let home = tempfile::tempdir().expect("create temp dir");
    let mut parent_config = core_test_support::load_default_config_for_test(&home).await;
    parent_config.features = ManagedFeatures::from_configured_with_warnings(
        parent_config.features.get().clone(),
        Some(Sourced {
            value: FeatureRequirementsToml {
                entries: BTreeMap::from([("multi_agent".to_string(), true)]),
                ..Default::default()
            },
            source: RequirementSource::Unknown,
        }),
        &mut parent_config.startup_warnings,
    )
    .expect("managed features");

    let guardian_config = build_reviewer_config(&parent_config)
        .expect("guardian config should continue when a disabled feature is pinned on");

    assert!(guardian_config.features.enabled(Feature::Collab));
    assert!(guardian_config.mcp_servers.get().is_empty());
    assert!(!guardian_config.include_apps_instructions);
}
