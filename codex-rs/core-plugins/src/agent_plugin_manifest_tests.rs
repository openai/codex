use super::PluginManifest;
use super::PluginManifestMcpServers;
use super::parse_resolved_plugin_manifest;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::AGENT_PLUGIN_SCHEMA_URI;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn write_agent_plugin_manifest(plugin_root: &Path, extra_fields: &str) {
    fs::create_dir_all(plugin_root).expect("create plugin root");
    fs::write(
        plugin_root.join("plugin.json"),
        format!(
            r#"{{
  "$schema": "{AGENT_PLUGIN_SCHEMA_URI}",
  "name": "demo-plugin"{extra_fields}
}}"#
        ),
    )
    .expect("write Agent Plugins manifest");
}

fn load_manifest(plugin_root: &Path) -> PluginManifest {
    try_load_manifest(plugin_root).expect("load plugin manifest")
}

fn try_load_manifest(plugin_root: &Path) -> Option<PluginManifest> {
    let manifest_path = plugin_root.join("plugin.json");
    let contents = fs::read_to_string(&manifest_path).ok()?;
    let overlay_path = plugin_root.join(".codex-plugin/plugin.json");
    let overlay_contents = fs::read_to_string(&overlay_path).ok();
    parse_resolved_plugin_manifest(
        plugin_root,
        &manifest_path,
        &contents,
        overlay_contents
            .as_ref()
            .map(|contents| (overlay_path.as_path(), contents.as_str())),
    )
    .ok()
}

#[test]
fn uses_portable_metadata_and_fixed_components() {
    let tmp = tempdir().expect("tempdir");
    let plugin_root = tmp.path().join("demo-plugin");
    write_agent_plugin_manifest(
        &plugin_root,
        r#",
  "version": "release-2026-07",
  "description": "Portable demo",
  "author": {"name": "Portable Author"},
  "homepage": "https://example.com/plugin",
  "keywords": ["portable"]"#,
    );

    let manifest = load_manifest(&plugin_root);

    assert_eq!(manifest.name, "demo-plugin");
    assert_eq!(manifest.version.as_deref(), Some("release-2026-07"));
    assert_eq!(manifest.description.as_deref(), Some("Portable demo"));
    assert_eq!(manifest.keywords, vec!["portable"]);
    assert_eq!(
        manifest.paths.skills,
        vec![
            AbsolutePathBuf::from_absolute_path_checked(plugin_root.join("skills"))
                .expect("skills path")
        ]
    );
    assert_eq!(
        manifest.paths.mcp_servers,
        Some(PluginManifestMcpServers::Path(
            AbsolutePathBuf::from_absolute_path_checked(plugin_root.join("mcp.json"))
                .expect("MCP path")
        ))
    );
    let interface = manifest.interface.expect("default portable interface");
    assert_eq!(interface.display_name.as_deref(), Some("demo-plugin"));
    assert_eq!(
        interface.short_description.as_deref(),
        Some("Portable demo")
    );
    assert_eq!(interface.long_description.as_deref(), Some("Portable demo"));
    assert_eq!(interface.developer_name.as_deref(), Some("Portable Author"));
    assert_eq!(
        interface.website_url.as_deref(),
        Some("https://example.com/plugin")
    );
    assert_eq!(interface.category.as_deref(), Some("Other"));
}

#[test]
fn does_not_invent_optional_metadata() {
    let tmp = tempdir().expect("tempdir");
    let plugin_root = tmp.path().join("demo-plugin");
    write_agent_plugin_manifest(&plugin_root, "");

    let manifest = load_manifest(&plugin_root);

    assert_eq!(manifest.version, None);
    assert_eq!(manifest.description, None);
    let interface = manifest.interface.expect("default portable interface");
    assert_eq!(interface.display_name.as_deref(), Some("demo-plugin"));
    assert_eq!(interface.short_description, None);
    assert_eq!(interface.long_description, None);
    assert_eq!(interface.developer_name, None);
    assert_eq!(interface.website_url, None);
}

#[test]
fn normalizes_empty_optional_metadata() {
    let tmp = tempdir().expect("tempdir");
    let plugin_root = tmp.path().join("demo-plugin");
    write_agent_plugin_manifest(
        &plugin_root,
        r#",
  "version": "",
  "description": " ",
  "author": {"name": " ", "email": ""},
  "homepage": "",
  "keywords": [""]"#,
    );

    let manifest = load_manifest(&plugin_root);

    assert_eq!(manifest.version, None);
    assert_eq!(manifest.description, None);
    assert_eq!(manifest.keywords, vec![""]);
    let interface = manifest.interface.expect("default portable interface");
    assert_eq!(interface.short_description, None);
    assert_eq!(interface.long_description, None);
    assert_eq!(interface.developer_name, None);
    assert_eq!(interface.website_url, None);
}

#[test]
fn accepts_dotted_names_and_ignores_extensions() {
    let tmp = tempdir().expect("tempdir");
    let plugin_root = tmp.path().join("acme-tools");
    fs::create_dir_all(&plugin_root).expect("create plugin root");
    fs::write(
        plugin_root.join("plugin.json"),
        format!(
            r#"{{
  "$schema":"{AGENT_PLUGIN_SCHEMA_URI}",
  "name":"acme.tools",
  "extensions":{{
    "com.example.client":{{"future":true}},
    "com.example.unimplemented":"ignored"
  }}
}}"#
        ),
    )
    .expect("write manifest");

    assert_eq!(load_manifest(&plugin_root).name, "acme.tools");

    fs::write(
        plugin_root.join("plugin.json"),
        format!(
            r#"{{"$schema":"{AGENT_PLUGIN_SCHEMA_URI}","name":"acme.tools","extensions":false}}"#
        ),
    )
    .expect("write manifest with invalid extensions");
    assert_eq!(load_manifest(&plugin_root).name, "acme.tools");
}

#[test]
fn rejects_overlong_name_and_wrong_metadata_types() {
    let tmp = tempdir().expect("tempdir");
    let plugin_root = tmp.path().join("demo-plugin");
    fs::create_dir_all(&plugin_root).expect("create plugin root");
    fs::write(
        plugin_root.join("plugin.json"),
        format!(
            r#"{{"$schema":"{AGENT_PLUGIN_SCHEMA_URI}","name":"{}"}}"#,
            "a".repeat(65)
        ),
    )
    .expect("write manifest");
    assert_eq!(try_load_manifest(&plugin_root), None);

    fs::write(
        plugin_root.join("plugin.json"),
        format!(r#"{{"$schema":"{AGENT_PLUGIN_SCHEMA_URI}","name":"demo-plugin","homepage":42}}"#),
    )
    .expect("write manifest");
    assert_eq!(try_load_manifest(&plugin_root), None);

    fs::write(
        plugin_root.join("plugin.json"),
        format!(r#"{{"$schema":"{AGENT_PLUGIN_SCHEMA_URI}","name":"demo-plugin","version":null}}"#),
    )
    .expect("write manifest");
    assert_eq!(try_load_manifest(&plugin_root), None);

    fs::write(
        plugin_root.join("plugin.json"),
        format!(
            r#"{{"$schema":"{AGENT_PLUGIN_SCHEMA_URI}","name":"demo-plugin","author":{{"name":null}}}}"#
        ),
    )
    .expect("write manifest");
    assert_eq!(try_load_manifest(&plugin_root), None);
}

#[test]
fn legacy_codex_overlay_keeps_portable_components_fixed() {
    let tmp = tempdir().expect("tempdir");
    let plugin_root = tmp.path().join("demo-plugin");
    write_agent_plugin_manifest(
        &plugin_root,
        r#",
  "version": "portable-version",
  "description": "Portable description""#,
    );
    fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create overlay dir");
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "different-name",
  "version": "9.9.9",
  "description": "Codex description",
  "skills": [],
  "mcpServers": null,
  "interface": {"displayName": "Codex Demo"}
}"#,
    )
    .expect("write overlay");

    let manifest = load_manifest(&plugin_root);

    assert_eq!(manifest.name, "demo-plugin");
    assert_eq!(manifest.version.as_deref(), Some("portable-version"));
    assert_eq!(
        manifest.description.as_deref(),
        Some("Portable description")
    );
    assert_eq!(
        manifest.paths.skills,
        vec![
            AbsolutePathBuf::from_absolute_path_checked(plugin_root.join("skills"))
                .expect("skills path")
        ]
    );
    assert_eq!(
        manifest.paths.mcp_servers,
        Some(PluginManifestMcpServers::Path(
            AbsolutePathBuf::from_absolute_path_checked(plugin_root.join("mcp.json"))
                .expect("MCP path")
        ))
    );
    assert_eq!(
        manifest
            .interface
            .and_then(|interface| interface.display_name),
        Some("Codex Demo".to_string())
    );
}

#[test]
fn inline_openai_extension_precedes_legacy_overlay() {
    let tmp = tempdir().expect("tempdir");
    let plugin_root = tmp.path().join("demo-plugin");
    write_agent_plugin_manifest(
        &plugin_root,
        r#",
  "extensions": {
    "com.openai": {
      "interface": {"displayName": "Inline Codex"}
    }
  }"#,
    );
    fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create overlay dir");
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"interface":{"displayName":"Legacy Codex"}}"#,
    )
    .expect("write overlay");

    let manifest = load_manifest(&plugin_root);

    assert_eq!(
        manifest
            .interface
            .and_then(|interface| interface.display_name),
        Some("Inline Codex".to_string())
    );
}
