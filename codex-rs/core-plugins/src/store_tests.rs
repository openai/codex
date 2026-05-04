use super::*;
use codex_plugin::PluginId;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

fn write_plugin_with_version(
    root: &Path,
    dir_name: &str,
    manifest_name: &str,
    manifest_version: Option<&str>,
) {
    let plugin_root = root.join(dir_name);
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::create_dir_all(plugin_root.join("skills")).unwrap();
    let version = manifest_version
        .map(|manifest_version| format!(r#","version":"{manifest_version}""#))
        .unwrap_or_default();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(r#"{{"name":"{manifest_name}"{version}}}"#),
    )
    .unwrap();
    fs::write(plugin_root.join("skills/SKILL.md"), "skill").unwrap();
    fs::write(plugin_root.join(".mcp.json"), r#"{"mcpServers":{}}"#).unwrap();
}

fn write_plugin(root: &Path, dir_name: &str, manifest_name: &str) {
    write_plugin_with_version(
        root,
        dir_name,
        manifest_name,
        /*manifest_version*/ None,
    );
}

#[test]
fn try_new_rejects_relative_codex_home() {
    let err = PluginStore::try_new(PathBuf::from("relative"))
        .expect_err("relative codex home should fail");
    let err = err.to_string().replace('\\', "/");

    assert_eq!(
        err,
        "failed to resolve plugin cache root: path is not absolute: relative/plugins/cache"
    );
}

#[test]
fn install_copies_plugin_into_default_marketplace() {
    let tmp = tempdir().unwrap();
    write_plugin(tmp.path(), "sample-plugin", "sample-plugin");
    let plugin_id = PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap();

    let result = PluginStore::new(tmp.path().to_path_buf())
        .install(
            AbsolutePathBuf::try_from(tmp.path().join("sample-plugin")).unwrap(),
            plugin_id.clone(),
        )
        .unwrap();

    let installed_path = tmp.path().join("plugins/cache/debug/sample-plugin/local");
    assert_eq!(
        result,
        PluginInstallResult {
            plugin_id,
            plugin_version: "local".to_string(),
            installed_path: AbsolutePathBuf::try_from(installed_path.clone()).unwrap(),
        }
    );
    assert!(installed_path.join(".codex-plugin/plugin.json").is_file());
    assert!(installed_path.join("skills/SKILL.md").is_file());
}

#[test]
fn install_uses_manifest_name_for_destination_and_key() {
    let tmp = tempdir().unwrap();
    write_plugin(tmp.path(), "source-dir", "manifest-name");
    let plugin_id = PluginId::new("manifest-name".to_string(), "market".to_string()).unwrap();

    let result = PluginStore::new(tmp.path().to_path_buf())
        .install(
            AbsolutePathBuf::try_from(tmp.path().join("source-dir")).unwrap(),
            plugin_id.clone(),
        )
        .unwrap();

    assert_eq!(
        result,
        PluginInstallResult {
            plugin_id,
            plugin_version: "local".to_string(),
            installed_path: AbsolutePathBuf::try_from(
                tmp.path().join("plugins/cache/market/manifest-name/local"),
            )
            .unwrap(),
        }
    );
}

#[test]
fn plugin_root_derives_path_from_key_and_version() {
    let tmp = tempdir().unwrap();
    let store = PluginStore::new(tmp.path().to_path_buf());
    let plugin_id = PluginId::new("sample".to_string(), "debug".to_string()).unwrap();

    assert_eq!(
        store.plugin_root(&plugin_id, "local").as_path(),
        tmp.path().join("plugins/cache/debug/sample/local")
    );
}

#[test]
fn plugin_data_root_derives_path_from_key() {
    let tmp = tempdir().unwrap();
    let store = PluginStore::new(tmp.path().to_path_buf());
    let plugin_id = PluginId::new("sample".to_string(), "debug".to_string()).unwrap();

    assert_eq!(
        store.plugin_data_root(&plugin_id).as_path(),
        tmp.path().join("plugins/data/sample-debug")
    );
}

#[test]
fn install_with_version_uses_requested_cache_version() {
    let tmp = tempdir().unwrap();
    write_plugin(tmp.path(), "sample-plugin", "sample-plugin");
    let plugin_id =
        PluginId::new("sample-plugin".to_string(), "openai-curated".to_string()).unwrap();
    let plugin_version = "0123456789abcdef".to_string();

    let result = PluginStore::new(tmp.path().to_path_buf())
        .install_with_version(
            AbsolutePathBuf::try_from(tmp.path().join("sample-plugin")).unwrap(),
            plugin_id.clone(),
            plugin_version.clone(),
        )
        .unwrap();

    let installed_path = tmp.path().join(format!(
        "plugins/cache/openai-curated/sample-plugin/{plugin_version}"
    ));
    assert_eq!(
        result,
        PluginInstallResult {
            plugin_id,
            plugin_version: plugin_version.clone(),
            installed_path: AbsolutePathBuf::try_from(installed_path.clone()).unwrap(),
        }
    );
    assert!(installed_path.join(".codex-plugin/plugin.json").is_file());
    assert_eq!(
        std::fs::read_to_string(
            tmp.path()
                .join("plugins/cache/openai-curated/sample-plugin/.active-version"),
        )
        .unwrap(),
        format!("{plugin_version}\n")
    );
    assert!(
        tmp.path()
            .join("plugins/cache/openai-curated/sample-plugin/.active-version.lock")
            .is_file()
    );
}

#[test]
fn install_with_existing_version_reuses_cache_directory() {
    let tmp = tempdir().unwrap();
    write_plugin(tmp.path(), "source-one", "sample-plugin");
    write_plugin(tmp.path(), "source-two", "sample-plugin");
    fs::write(tmp.path().join("source-two/new-file"), "new").unwrap();
    let plugin_id =
        PluginId::new("sample-plugin".to_string(), "openai-curated".to_string()).unwrap();
    let plugin_version = "0123456789abcdef".to_string();
    let store = PluginStore::new(tmp.path().to_path_buf());

    let result = store
        .install_with_version(
            AbsolutePathBuf::try_from(tmp.path().join("source-one")).unwrap(),
            plugin_id.clone(),
            plugin_version.clone(),
        )
        .unwrap();
    fs::write(result.installed_path.as_path().join("sentinel"), "old").unwrap();

    let reinstall_result = store
        .install_with_version(
            AbsolutePathBuf::try_from(tmp.path().join("source-two")).unwrap(),
            plugin_id.clone(),
            plugin_version.clone(),
        )
        .unwrap();

    assert_eq!(reinstall_result, result);
    assert_eq!(
        fs::read_to_string(result.installed_path.as_path().join("sentinel")).unwrap(),
        "old"
    );
    assert!(!result.installed_path.as_path().join("new-file").exists());
    assert_eq!(
        store.active_plugin_version(&plugin_id),
        Some(plugin_version)
    );
}

#[test]
fn install_with_version_adds_new_version_without_deleting_old_version() {
    let tmp = tempdir().unwrap();
    write_plugin(tmp.path(), "source-one", "sample-plugin");
    write_plugin(tmp.path(), "source-two", "sample-plugin");
    let plugin_id =
        PluginId::new("sample-plugin".to_string(), "openai-curated".to_string()).unwrap();
    let store = PluginStore::new(tmp.path().to_path_buf());

    store
        .install_with_version(
            AbsolutePathBuf::try_from(tmp.path().join("source-one")).unwrap(),
            plugin_id.clone(),
            "1.0.0".to_string(),
        )
        .unwrap();
    store
        .install_with_version(
            AbsolutePathBuf::try_from(tmp.path().join("source-two")).unwrap(),
            plugin_id.clone(),
            "2.0.0".to_string(),
        )
        .unwrap();

    assert!(
        tmp.path()
            .join("plugins/cache/openai-curated/sample-plugin/1.0.0")
            .is_dir()
    );
    assert!(
        tmp.path()
            .join("plugins/cache/openai-curated/sample-plugin/2.0.0")
            .is_dir()
    );
    assert_eq!(
        std::fs::read_to_string(
            tmp.path()
                .join("plugins/cache/openai-curated/sample-plugin/.active-version"),
        )
        .unwrap(),
        "2.0.0\n"
    );
    assert_eq!(
        store.active_plugin_version(&plugin_id),
        Some("2.0.0".to_string())
    );
}

#[test]
fn install_uses_manifest_version_when_present() {
    let tmp = tempdir().unwrap();
    write_plugin_with_version(
        tmp.path(),
        "sample-plugin",
        "sample-plugin",
        Some("1.2.3-beta+7"),
    );
    let plugin_id = PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap();

    let result = PluginStore::new(tmp.path().to_path_buf())
        .install(
            AbsolutePathBuf::try_from(tmp.path().join("sample-plugin")).unwrap(),
            plugin_id.clone(),
        )
        .unwrap();

    let installed_path = tmp
        .path()
        .join("plugins/cache/debug/sample-plugin/1.2.3-beta+7");
    assert_eq!(
        result,
        PluginInstallResult {
            plugin_id,
            plugin_version: "1.2.3-beta+7".to_string(),
            installed_path: AbsolutePathBuf::try_from(installed_path.clone()).unwrap(),
        }
    );
    assert!(installed_path.join(".codex-plugin/plugin.json").is_file());
}

#[test]
fn install_rejects_blank_manifest_version() {
    let tmp = tempdir().unwrap();
    write_plugin_with_version(tmp.path(), "sample-plugin", "sample-plugin", Some("   "));
    let plugin_id = PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap();

    let err = PluginStore::new(tmp.path().to_path_buf())
        .install(
            AbsolutePathBuf::try_from(tmp.path().join("sample-plugin")).unwrap(),
            plugin_id,
        )
        .expect_err("blank manifest version should be rejected");
    let err = err.to_string().replace('\\', "/");

    assert_eq!(
        err,
        "invalid plugin version in plugin.json: must not be blank"
    );
}

#[test]
fn active_plugin_version_prefers_active_marker_over_sorted_versions() {
    let tmp = tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/1.0.0",
        "sample-plugin",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/9.0.0",
        "sample-plugin",
    );
    fs::write(
        tmp.path()
            .join("plugins/cache/debug/sample-plugin/.active-version"),
        "1.0.0\n",
    )
    .unwrap();
    let store = PluginStore::new(tmp.path().to_path_buf());
    let plugin_id = PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap();

    assert_eq!(
        store.active_plugin_version(&plugin_id),
        Some("1.0.0".to_string())
    );
}

#[test]
fn active_plugin_version_reads_version_directory_name() {
    let tmp = tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/local",
        "sample-plugin",
    );
    let store = PluginStore::new(tmp.path().to_path_buf());
    let plugin_id = PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap();

    assert_eq!(
        store.active_plugin_version(&plugin_id),
        Some("local".to_string())
    );
    assert_eq!(
        store.active_plugin_root(&plugin_id).unwrap().as_path(),
        tmp.path().join("plugins/cache/debug/sample-plugin/local")
    );
}

#[test]
fn cleanup_inactive_versions_removes_versions_except_active_marker_version() {
    let tmp = tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/1.0.0",
        "sample-plugin",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/2.0.0",
        "sample-plugin",
    );
    fs::write(
        tmp.path()
            .join("plugins/cache/debug/sample-plugin/.active-version"),
        "2.0.0\n",
    )
    .unwrap();
    let store = PluginStore::new(tmp.path().to_path_buf());
    let plugin_id = PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap();

    store.cleanup_inactive_versions(&plugin_id);

    assert!(
        !tmp.path()
            .join("plugins/cache/debug/sample-plugin/1.0.0")
            .exists()
    );
    assert!(
        tmp.path()
            .join("plugins/cache/debug/sample-plugin/2.0.0")
            .is_dir()
    );
}

#[test]
fn cleanup_inactive_versions_logs_and_continues_after_remove_failures() {
    let tmp = tempdir().unwrap();
    let plugin_base_root = tmp.path().join("plugins/cache/debug/sample-plugin");
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/1.0.0",
        "sample-plugin",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/2.0.0",
        "sample-plugin",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/3.0.0",
        "sample-plugin",
    );
    fs::write(plugin_base_root.join(".active-version"), "3.0.0\n").unwrap();
    let plugin_id = PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap();

    cleanup_inactive_versions_with_remover(&plugin_id, &plugin_base_root, |path| {
        if path.ends_with("1.0.0") {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "held open"))
        } else {
            fs::remove_dir_all(path)
        }
    });

    assert!(plugin_base_root.join("1.0.0").is_dir());
    assert!(!plugin_base_root.join("2.0.0").exists());
    assert!(plugin_base_root.join("3.0.0").is_dir());
}

#[test]
fn active_plugin_version_prefers_default_local_version_when_multiple_versions_exist() {
    let tmp = tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/0123456789abcdef",
        "sample-plugin",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/local",
        "sample-plugin",
    );
    let store = PluginStore::new(tmp.path().to_path_buf());
    let plugin_id = PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap();

    assert_eq!(
        store.active_plugin_version(&plugin_id),
        Some("local".to_string())
    );
}

#[test]
fn active_plugin_version_returns_last_sorted_version_when_default_is_missing() {
    let tmp = tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/0123456789abcdef",
        "sample-plugin",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/fedcba9876543210",
        "sample-plugin",
    );
    let store = PluginStore::new(tmp.path().to_path_buf());
    let plugin_id = PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap();

    assert_eq!(
        store.active_plugin_version(&plugin_id),
        Some("fedcba9876543210".to_string())
    );
}

#[test]
fn plugin_root_rejects_path_separators_in_key_segments() {
    let err = PluginId::parse("../../etc@debug").unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid plugin name: only ASCII letters, digits, `_`, and `-` are allowed in `../../etc@debug`"
    );

    let err = PluginId::parse("sample@../../etc").unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid marketplace name: only ASCII letters, digits, `_`, and `-` are allowed in `sample@../../etc`"
    );
}

#[test]
fn install_rejects_manifest_names_with_path_separators() {
    let tmp = tempdir().unwrap();
    write_plugin(tmp.path(), "source-dir", "../../etc");

    let err = PluginStore::new(tmp.path().to_path_buf())
        .install(
            AbsolutePathBuf::try_from(tmp.path().join("source-dir")).unwrap(),
            PluginId::new("source-dir".to_string(), "debug".to_string()).unwrap(),
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "invalid plugin name: only ASCII letters, digits, `_`, and `-` are allowed"
    );
}

#[test]
fn install_rejects_marketplace_names_with_path_separators() {
    let err = PluginId::new("sample-plugin".to_string(), "../../etc".to_string()).unwrap_err();

    assert_eq!(
        err.to_string(),
        "invalid marketplace name: only ASCII letters, digits, `_`, and `-` are allowed"
    );
}

#[test]
fn install_rejects_manifest_names_that_do_not_match_marketplace_plugin_name() {
    let tmp = tempdir().unwrap();
    write_plugin(tmp.path(), "source-dir", "manifest-name");

    let err = PluginStore::new(tmp.path().to_path_buf())
        .install(
            AbsolutePathBuf::try_from(tmp.path().join("source-dir")).unwrap(),
            PluginId::new("different-name".to_string(), "debug".to_string()).unwrap(),
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "plugin.json name `manifest-name` does not match marketplace plugin name `different-name`"
    );
}
