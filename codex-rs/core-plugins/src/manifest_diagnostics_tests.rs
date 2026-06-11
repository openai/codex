use super::diagnose_plugin_manifest;
use std::fs;
use tempfile::tempdir;

#[test]
fn diagnostics_explain_ignored_capability_paths() {
    let tmp = tempdir().expect("tempdir");
    let plugin_root = tmp.path().join("plugin");
    let manifest_path = plugin_root.join(".claude-plugin/plugin.json");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create manifest directory");
    fs::write(
        manifest_path,
        r#"{
  "name": "plugin",
  "skills": "../outside",
  "hooks": 7,
  "commands": "./commands"
}"#,
    )
    .expect("write manifest");

    let diagnostics = diagnose_plugin_manifest(&plugin_root);
    let manifest = diagnostics.manifest.expect("partially valid manifest");

    assert!(manifest.paths.skills.is_none());
    assert!(manifest.paths.hooks.is_none());
    assert!(
        diagnostics
            .issues
            .iter()
            .any(|issue| issue.contains("skills") && issue.contains("start with `./`"))
    );
    assert_eq!(
        diagnostics.unsupported_capability_fields,
        vec!["commands".to_string()]
    );
    assert!(
        diagnostics
            .issues
            .iter()
            .any(|issue| issue.contains("hooks") && issue.contains("found number"))
    );
}

#[test]
fn diagnostics_explain_missing_manifest() {
    let tmp = tempdir().expect("tempdir");
    let diagnostics = diagnose_plugin_manifest(tmp.path());

    assert!(diagnostics.manifest.is_none());
    assert_eq!(diagnostics.issues.len(), 1);
    assert!(diagnostics.issues[0].contains("does not contain"));
}
