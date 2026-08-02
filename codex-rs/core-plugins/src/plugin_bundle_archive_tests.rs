use super::*;
use tempfile::tempdir;

#[test]
fn portable_root_manifest_can_be_packed_and_unpacked() {
    let source = tempdir().expect("source tempdir");
    fs::write(
        source.path().join("plugin.json"),
        r#"{"$schema":"https://agent-plugins.org/schemas/1.0.0/plugin.schema.json","name":"portable"}"#,
    )
    .expect("write portable manifest");
    fs::create_dir_all(source.path().join("skills/demo")).expect("create skill directory");
    fs::write(source.path().join("skills/demo/SKILL.md"), "# Demo\n").expect("write skill");

    let archive =
        pack_plugin_bundle_tar_gz(source.path(), 1024 * 1024).expect("pack portable plugin");
    let destination = tempdir().expect("destination tempdir");
    unpack_plugin_bundle_tar_gz(&archive, destination.path(), 1024 * 1024)
        .expect("unpack portable plugin");

    assert!(destination.path().join("plugin.json").is_file());
    assert!(destination.path().join("skills/demo/SKILL.md").is_file());
}

#[test]
fn invalid_portable_manifest_is_rejected_before_packing() {
    let source = tempdir().expect("source tempdir");
    fs::write(
        source.path().join("plugin.json"),
        r#"{"$schema":"https://agent-plugins.org/schemas/1.0.0/plugin.schema.json","name":"UPPER"}"#,
    )
    .expect("write invalid portable manifest");

    let error = pack_plugin_bundle_tar_gz(source.path(), 1024 * 1024)
        .expect_err("invalid Agent Plugin manifest should fail");

    assert!(matches!(
        error,
        PluginBundlePackError::InvalidPluginPath { .. }
    ));
}
