use std::fs;

use codex_plugin::PluginHookSource;
use codex_plugin::PluginHookSourceKind;
use codex_plugin::PluginId;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::loader::load_plugin_hooks;
use crate::manifest::load_plugin_manifest;

fn computer_use_plugin_id() -> PluginId {
    PluginId::parse("computer-use@openai-bundled").expect("computer-use plugin id")
}

fn plugin_root(temp: &TempDir, name: &str) -> AbsolutePathBuf {
    let root = AbsolutePathBuf::from_absolute_path(temp.path().join(name))
        .expect("plugin root should be absolute");
    fs::create_dir_all(root.join(".codex-plugin")).expect("create manifest dir");
    fs::create_dir_all(root.join("hooks")).expect("create hooks dir");
    root
}

fn write_manifest(root: &AbsolutePathBuf, manifest: &str) {
    fs::write(root.join(".codex-plugin/plugin.json"), manifest).expect("write manifest");
}

fn write_default_hooks(root: &AbsolutePathBuf, command: &str, command_windows: Option<&str>) {
    let command_windows = command_windows
        .map(|command| format!(r#","commandWindows":"{command}""#))
        .unwrap_or_default();
    fs::write(
        root.join("hooks/hooks.json"),
        format!(
            r#"{{
  "hooks": {{
    "PreToolUse": [
      {{
        "matcher": "Bash",
        "hooks": [
          {{
            "type": "command",
            "command": "{command}",
            "timeout": 12,
            "async": false,
            "statusMessage": "checking"{command_windows}
          }}
        ]
      }}
    ]
  }}
}}"#
        ),
    )
    .expect("write hooks");
}

fn load_sources(
    plugin_root: &AbsolutePathBuf,
    plugin_id: &PluginId,
) -> (Vec<PluginHookSource>, Vec<String>) {
    let manifest = load_plugin_manifest(plugin_root.as_path()).expect("manifest");
    let plugin_data_root = plugin_root
        .as_path()
        .parent()
        .expect("plugin root parent")
        .join("plugin-data");
    let plugin_data_root =
        AbsolutePathBuf::from_absolute_path(plugin_data_root).expect("plugin data root");
    load_plugin_hooks(plugin_root, plugin_id, &plugin_data_root, &manifest.paths)
}

fn apply_authority(
    plugin_id: &PluginId,
    installed_root: &AbsolutePathBuf,
    packaged_root: &AbsolutePathBuf,
    hook_sources: Vec<PluginHookSource>,
    hook_load_warnings: Vec<String>,
) -> (Vec<PluginHookSource>, Vec<String>) {
    let plugin_data_root = installed_root
        .as_path()
        .parent()
        .expect("plugin root parent")
        .join("plugin-data");
    let plugin_data_root =
        AbsolutePathBuf::from_absolute_path(plugin_data_root).expect("plugin data root");
    apply_app_bundled_internal_hook_authority(
        plugin_id,
        installed_root,
        &plugin_data_root,
        hook_sources,
        hook_load_warnings,
        &[AppBundledInternalPlugin {
            plugin_id: plugin_id.clone(),
            plugin_root: packaged_root.clone(),
        }],
    )
}

#[test]
fn matching_allowlisted_declarations_become_app_bundled_internal() {
    let temp = TempDir::new().expect("tempdir");
    let installed_root = plugin_root(&temp, "installed");
    let packaged_root = plugin_root(&temp, "packaged");
    write_manifest(&installed_root, r#"{ "name": "computer-use" }"#);
    write_manifest(&packaged_root, r#"{ "name": "computer-use" }"#);
    write_default_hooks(
        &installed_root,
        "python3 hooks/check.py",
        Some("py hooks/check.py"),
    );
    write_default_hooks(
        &packaged_root,
        "python3 hooks/check.py",
        Some("py hooks/check.py"),
    );
    let plugin_id = computer_use_plugin_id();
    let (hook_sources, hook_load_warnings) = load_sources(&installed_root, &plugin_id);

    let (verified_sources, warnings) = apply_authority(
        &plugin_id,
        &installed_root,
        &packaged_root,
        hook_sources,
        hook_load_warnings,
    );

    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(verified_sources.len(), 1);
    assert_eq!(
        verified_sources[0].kind,
        PluginHookSourceKind::AppBundledInternal
    );
    assert_eq!(
        verified_sources[0].source_path,
        installed_root.join("hooks/hooks.json")
    );
}

#[test]
fn mismatched_relative_source_path_fails_closed() {
    let temp = TempDir::new().expect("tempdir");
    let installed_root = plugin_root(&temp, "installed");
    let packaged_root = plugin_root(&temp, "packaged");
    write_manifest(&installed_root, r#"{ "name": "computer-use" }"#);
    write_manifest(
        &packaged_root,
        r#"{ "name": "computer-use", "hooks": "./hooks/internal.json" }"#,
    );
    write_default_hooks(
        &installed_root,
        "python3 hooks/check.py",
        /*command_windows*/ None,
    );
    fs::write(
        packaged_root.join("hooks/internal.json"),
        fs::read_to_string(installed_root.join("hooks/hooks.json")).expect("read installed hooks"),
    )
    .expect("write packaged hooks");
    let plugin_id = computer_use_plugin_id();
    let (hook_sources, hook_load_warnings) = load_sources(&installed_root, &plugin_id);

    let (verified_sources, warnings) = apply_authority(
        &plugin_id,
        &installed_root,
        &packaged_root,
        hook_sources,
        hook_load_warnings,
    );

    assert_eq!(verified_sources, Vec::<PluginHookSource>::new());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("declaration_mismatch"));
}

#[test]
fn non_allowlisted_authority_fails_closed() {
    let temp = TempDir::new().expect("tempdir");
    let installed_root = plugin_root(&temp, "installed");
    let packaged_root = plugin_root(&temp, "packaged");
    write_manifest(&installed_root, r#"{ "name": "demo-plugin" }"#);
    write_manifest(&packaged_root, r#"{ "name": "demo-plugin" }"#);
    write_default_hooks(
        &installed_root,
        "echo installed",
        /*command_windows*/ None,
    );
    write_default_hooks(
        &packaged_root,
        "echo installed",
        /*command_windows*/ None,
    );
    let plugin_id = PluginId::parse("demo-plugin@openai-bundled").expect("plugin id");
    let (hook_sources, hook_load_warnings) = load_sources(&installed_root, &plugin_id);

    let (verified_sources, warnings) = apply_authority(
        &plugin_id,
        &installed_root,
        &packaged_root,
        hook_sources,
        hook_load_warnings,
    );

    assert_eq!(verified_sources, Vec::<PluginHookSource>::new());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("plugin_not_allowlisted"));
}
