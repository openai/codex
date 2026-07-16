use super::*;
use crate::InstructionSourceGroup;
use crate::detect::plugins::detect_cur_plugins;
use crate::migration_source::PluginDetectionContext;
use crate::model::MigrationDetails;
use crate::model::PluginsMigration;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use tempfile::TempDir;
use toml::Value as TomlValue;

#[test]
fn effective_settings_merge_sandbox_configuration() {
    let root = TempDir::new().expect("tempdir");
    let source_dir = root.path().join(CurSource::CONFIG_DIR);
    let source_settings = source_dir.join(CurSource::HOME_CONFIG_FILE);
    fs::create_dir_all(&source_dir).expect("source directory");
    fs::write(&source_settings, r#"{"env":{"FOO":"bar"}}"#).expect("source settings");
    fs::write(
        source_dir.join(CurSource::SANDBOX_CONFIG_FILE),
        r#"{"type":"read_only"}"#,
    )
    .expect("sandbox settings");

    assert_eq!(
        CurSource::effective_settings(&source_dir, &source_settings).expect("effective settings"),
        Some(serde_json::json!({
            "env": {"FOO": "bar"},
            (CurSource::SANDBOX_SETTINGS_KEY): {"type": "read_only"}
        }))
    );
}

#[test]
fn append_config_maps_workspace_permissions() {
    let root = TempDir::new().expect("tempdir");
    let writable_root = root.path().join("generated");
    let settings = serde_json::json!({
        (CurSource::SANDBOX_SETTINGS_KEY): {
            "type": "workspace_readwrite",
            "additionalReadwritePaths": [writable_root.display().to_string(), "relative/path"],
            "disableTmpWrite": true,
            "networkPolicy": {"default": "allow"}
        }
    });
    let mut config = toml::map::Map::new();

    CurSource::append_config(&mut config, settings.as_object().expect("settings object"));

    let mut workspace_write = toml::map::Map::new();
    workspace_write.insert(
        "writable_roots".to_string(),
        TomlValue::Array(vec![TomlValue::String(
            writable_root.to_string_lossy().into_owned(),
        )]),
    );
    workspace_write.insert("exclude_slash_tmp".to_string(), TomlValue::Boolean(true));
    workspace_write.insert(
        "exclude_tmpdir_env_var".to_string(),
        TomlValue::Boolean(true),
    );
    workspace_write.insert("network_access".to_string(), TomlValue::Boolean(true));
    let mut expected = toml::map::Map::new();
    expected.insert(
        "sandbox_mode".to_string(),
        TomlValue::String("workspace-write".to_string()),
    );
    expected.insert(
        "sandbox_workspace_write".to_string(),
        TomlValue::Table(workspace_write),
    );

    assert_eq!(TomlValue::Table(config), TomlValue::Table(expected));
}

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
