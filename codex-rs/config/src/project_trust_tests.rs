//! Exercises project-key precedence and map matching across path conventions.

use super::*;
use codex_protocol::config_types::TrustLevel;
use pretty_assertions::assert_eq;

fn config_with_projects(entries: &[(&str, Option<TrustLevel>)]) -> ConfigToml {
    ConfigToml {
        projects: Some(
            entries
                .iter()
                .map(|(key, trust_level)| {
                    (
                        key.to_string(),
                        ProjectConfig {
                            trust_level: *trust_level,
                        },
                    )
                })
                .collect(),
        ),
        ..Default::default()
    }
}

fn assert_lookup_order(lookup: &ProjectTrustLookup, entries: &[(&str, Option<TrustLevel>)]) {
    let mut config = config_with_projects(entries);
    for (key, trust_level) in entries {
        assert_eq!(
            config.get_active_project_for_lookup(lookup),
            Some(ProjectConfig {
                trust_level: *trust_level
            }),
        );
        config.projects.as_mut().unwrap().remove(*key);
    }
    assert_eq!(config.get_active_project_for_lookup(lookup), None);
    assert_eq!(
        ConfigToml::default().get_active_project_for_lookup(lookup),
        None
    );
}

#[test]
fn posix_lookup_preserves_cwd_root_alias_order_and_case() {
    let lookup = ProjectTrustLookup::from_paths(
        PathConvention::Posix,
        ProjectTrustPath {
            original: "/alias/src".into(),
            canonical: Some("/repo/src".into()),
        },
        Some(ProjectTrustPath {
            original: "/alias".into(),
            canonical: Some("/repo".into()),
        }),
    );
    assert_lookup_order(
        &lookup,
        &[
            ("/repo/src", None),
            ("/alias/src", Some(TrustLevel::Trusted)),
            ("/repo", Some(TrustLevel::Untrusted)),
            ("/alias", Some(TrustLevel::Trusted)),
        ],
    );
    assert_eq!(
        config_with_projects(&[("/Repo/src", Some(TrustLevel::Trusted))])
            .get_active_project_for_lookup(&lookup),
        None,
    );
}

#[test]
fn windows_prefers_exact_normalized_key_then_sorted_aliases() {
    let lookup = ProjectTrustLookup::from_paths(
        PathConvention::Windows,
        ProjectTrustPath {
            original: r"C:\Repo".into(),
            canonical: None,
        },
        /*repo_root*/ None,
    );
    assert_lookup_order(
        &lookup,
        &[
            (r"c:\repo", Some(TrustLevel::Trusted)),
            (r"C:\REPO", Some(TrustLevel::Untrusted)),
            (r"C:\Repo", Some(TrustLevel::Trusted)),
        ],
    );
}
