use super::*;
use crate::InstructionSourceGroup;
use crate::detect::plugins::detect_cur_plugins;
use crate::migration_source::PluginDetectionContext;
use crate::model::MigrationDetails;
use crate::model::PluginsMigration;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use tempfile::TempDir;

#[test]
fn cached_marketplace_plugins_require_manifest_and_cache_entries() {
    let root = TempDir::new().expect("tempdir");
    let marketplace_root = root.path().join("plugins/marketplaces/acme");
    let cache_root = root.path().join("plugins/cache/acme");
    let manifest_path = marketplace_root.join(PLUGIN_MARKETPLACE_MANIFEST);
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("manifest directory");
    fs::create_dir_all(cache_root.join("sample")).expect("cached plugin");
    fs::create_dir_all(cache_root.join("not-listed")).expect("unlisted cached plugin");
    fs::write(
        &manifest_path,
        r#"{
            "name": "acme",
            "plugins": [{"name": "sample"}, {"name": "not-cached"}]
        }"#,
    )
    .expect("marketplace manifest");

    assert_eq!(
        cached_marketplace_plugins(root.path()).expect("cached marketplace plugins"),
        vec![CachedMarketplacePlugins {
            name: "acme".to_string(),
            source: marketplace_root,
            plugin_names: vec!["sample".to_string()],
        }]
    );
}

#[test]
fn detects_uninstalled_plugin_from_configured_marketplace() {
    let root = TempDir::new().expect("tempdir");
    let marketplace_root = root.path().join("plugins/marketplaces/acme");
    let manifest_path = marketplace_root.join(PLUGIN_MARKETPLACE_MANIFEST);
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("manifest directory");
    fs::create_dir_all(root.path().join("plugins/cache/acme/sample")).expect("cached plugin");
    fs::write(
        &manifest_path,
        r#"{"name":"acme","plugins":[{"name":"sample"}]}"#,
    )
    .expect("marketplace manifest");
    let configured_plugin_ids = HashSet::new();
    let configured_marketplace_plugins =
        BTreeMap::from([("acme".to_string(), HashSet::from(["sample".to_string()]))]);
    let source_settings = root.path().join(CurSource::HOME_CONFIG_FILE);
    let source_root = root.path().join("repo");

    let detected = detect_cur_plugins(&PluginDetectionContext {
        external_agent_home: root.path(),
        source_settings: &source_settings,
        source_root: &source_root,
        repo_root: None,
        settings: None,
        configured_plugin_ids: &configured_plugin_ids,
        configured_marketplace_plugins: &configured_marketplace_plugins,
    })
    .expect("detect plugins")
    .expect("plugin migration");

    assert_eq!(
        detected.details,
        MigrationDetails {
            plugins: vec![PluginsMigration {
                marketplace_name: "acme".to_string(),
                plugin_names: vec!["sample".to_string()],
            }],
            ..Default::default()
        }
    );
}

#[test]
fn detects_legacy_repo_instruction_file() {
    let root = TempDir::new().expect("tempdir");
    let source = root.path().join(CurSource::LEGACY_RULES_FILE);
    fs::write(&source, "Use the source agent carefully.\n").expect("legacy rules");

    assert_eq!(
        CurSource::repo_instruction_source_groups(root.path()).expect("instruction sources"),
        vec![InstructionSourceGroup {
            scope: root.path().to_path_buf(),
            sources: vec![source.clone()],
        }]
    );
    assert_eq!(
        CurSource::read_instruction_source(&source).expect("instruction contents"),
        "Use the source agent carefully.\n"
    );
}
