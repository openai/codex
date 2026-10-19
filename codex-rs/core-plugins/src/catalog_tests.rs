//! Tests Cloud catalog authority, version matching, and installation reconciliation.

use super::PluginCatalog;
use super::PluginCatalogEntry;
use super::PluginIdentity;
use super::PluginSourceLocation;
use super::merge_catalogs;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn cloud_plugin(remote_id: &str) -> PluginCatalogEntry {
    PluginCatalogEntry {
        enabled: true,
        id: PluginIdentity::Remote {
            remote_plugin_id: remote_id.to_string(),
        },
        display_name: "Example".to_string(),
        version: "1.0.0".to_string(),
        mcp_servers: BTreeMap::from([(
            "example".to_string(),
            serde_json::json!({"command":"node","args":["server.js"]}),
        )]),
        connector_ids: vec!["connector_example".to_string()],
        locations: vec![PluginSourceLocation::Cloud {
            resource_uri: format!("plugin://{remote_id}"),
            bundle_uri: None,
        }],
    }
}

fn executor_plugin(remote_id: &str, environment_id: &str) -> anyhow::Result<PluginCatalogEntry> {
    let mut entry = cloud_plugin(remote_id);
    entry.locations = vec![PluginSourceLocation::Executor {
        environment_id: environment_id.to_string(),
        plugin_id: "example@marketplace".to_string(),
        root: PathUri::parse("file:///plugins/example/1.0.0")?,
    }];
    Ok(entry)
}

#[test]
fn merged_catalog_prefers_cloud_and_matches_executor_versions() -> anyhow::Result<()> {
    let definition = cloud_plugin("plugin_a");
    let mut first = executor_plugin("plugin_a", "executor-a")?;
    first.display_name = "Installed display name".to_string();
    first.mcp_servers.clear();
    first.connector_ids.clear();
    let second = executor_plugin("plugin_a", "executor-b")?;
    let mut incompatible = executor_plugin("plugin_a", "executor-c")?;
    incompatible.version = "2.0.0".to_string();
    let mut expected = definition.clone();
    expected.locations.extend(first.locations.clone());
    expected.locations.extend(second.locations.clone());

    let executor = PluginCatalog {
        entries: vec![first.clone(), first, second, incompatible],
        warnings: vec!["executor warning".to_string()],
        ..Default::default()
    };
    let cloud = PluginCatalog {
        entries: vec![definition],
        warnings: vec!["cloud warning".to_string()],
        ..Default::default()
    };
    assert_eq!(
        merge_catalogs(Some(&executor), Some(&cloud)),
        PluginCatalog {
            entries: vec![expected],
            warnings: vec![
                "cloud warning".to_string(),
                "executor warning".to_string(),
                "plugin `plugin_a` has different cloud and executor versions; retaining the cloud definition"
                    .to_string(),
            ],
            ..Default::default()
        }
    );
    Ok(())
}

#[test]
fn cloud_catalog_controls_remote_plugins_but_preserves_local_plugins() -> anyhow::Result<()> {
    let installed = executor_plugin("plugin_a", "executor-a")?;
    let mut local = executor_plugin("plugin_a", "executor-b")?;
    local.id = PluginIdentity::Local {
        plugin_id: "example@marketplace".to_string(),
    };
    let executor = PluginCatalog {
        entries: vec![installed.clone(), local.clone()],
        ..Default::default()
    };
    let local_only = PluginCatalog {
        entries: vec![local.clone()],
        ..Default::default()
    };
    // An available Cloud catalog controls remote entries; local entries are unaffected.
    let mut other_installation = executor_plugin("plugin_a", "executor-c")?;
    other_installation.version = "2.0.0".to_string();
    let unmatched = PluginCatalog {
        entries: vec![installed.clone(), other_installation.clone(), local.clone()],
        ..Default::default()
    };
    let mut disabled = installed.clone();
    disabled.enabled = false;
    other_installation.enabled = false;
    let disabled_entries = vec![disabled, other_installation, local.clone()];
    assert_eq!(merge_catalogs(Some(&unmatched), /*cloud*/ None), unmatched);
    let explicitly_disabled = PluginCatalog {
        entries: disabled_entries.clone(),
        ..Default::default()
    };
    assert_eq!(
        merge_catalogs(Some(&explicitly_disabled), /*cloud*/ None),
        explicitly_disabled,
    );
    for cloud in [
        Some(PluginCatalog::default()),
        Some(PluginCatalog {
            is_partial: true,
            warnings: vec!["partial cloud listing".to_string()],
            ..Default::default()
        }),
    ] {
        assert_eq!(
            merge_catalogs(Some(&unmatched), cloud.as_ref()),
            PluginCatalog {
                entries: if cloud.as_ref().is_some_and(|catalog| catalog.is_partial) {
                    unmatched.entries.clone()
                } else {
                    disabled_entries.clone()
                },
                is_partial: cloud.as_ref().is_some_and(|catalog| catalog.is_partial),
                warnings: cloud.map(|catalog| catalog.warnings).unwrap_or_default(),
            },
        );
    }

    let definition = cloud_plugin("plugin_a");
    let cloud = PluginCatalog {
        entries: vec![definition.clone()],
        ..Default::default()
    };
    assert_eq!(merge_catalogs(/*executor*/ None, Some(&cloud)), cloud);
    assert_eq!(
        merge_catalogs(/*executor*/ None, /*cloud*/ None),
        PluginCatalog::default(),
    );
    let mut authorized = definition.clone();
    authorized.locations.extend(installed.locations);
    assert_eq!(
        merge_catalogs(Some(&executor), Some(&cloud)),
        PluginCatalog {
            entries: vec![authorized, local.clone()],
            ..Default::default()
        }
    );

    // Removing an installation preserves its Cloud definition without the executor route.
    assert_eq!(
        merge_catalogs(Some(&local_only), Some(&cloud)),
        PluginCatalog {
            entries: vec![definition, local],
            ..Default::default()
        }
    );
    Ok(())
}

#[test]
fn partial_cloud_merges_known_plugins_and_preserves_unmatched_enablement() -> anyhow::Result<()> {
    let definition = cloud_plugin("matched");
    let matched = executor_plugin("matched", "executor-a")?;
    let mut denied = executor_plugin("denied", "executor-a")?;
    denied.enabled = false;
    let unmatched = executor_plugin("unmatched", "executor-a")?;
    let executor = PluginCatalog {
        entries: vec![matched.clone(), denied.clone(), unmatched.clone()],
        ..Default::default()
    };
    let cloud = PluginCatalog {
        entries: vec![definition.clone()],
        is_partial: true,
        ..Default::default()
    };
    let mut expected = definition;
    expected.locations.extend(matched.locations);
    assert_eq!(
        merge_catalogs(Some(&executor), Some(&cloud)),
        PluginCatalog {
            entries: vec![expected, denied, unmatched],
            is_partial: true,
            ..Default::default()
        }
    );
    Ok(())
}
