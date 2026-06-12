use super::*;
use crate::manifest::PluginManifestHooks;
use crate::manifest::PluginManifestPaths;
use codex_config::HooksFile;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn absolute_path(path: impl AsRef<Path>) -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(path.as_ref().to_path_buf()).unwrap()
}

fn empty_manifest_paths() -> PluginManifestPaths {
    PluginManifestPaths {
        skills: None,
        mcp_servers: None,
        apps: None,
        hooks: None,
    }
}

fn sorted_skill_paths(capabilities: &DeclaredPluginCapabilities) -> Vec<String> {
    let mut paths = capabilities
        .skills
        .iter()
        .map(|skill| skill.path.display().to_string())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn sorted_app_names(capabilities: &DeclaredPluginCapabilities) -> Vec<String> {
    let mut apps = capabilities
        .apps
        .iter()
        .map(|app| app.name.clone())
        .collect::<Vec<_>>();
    apps.sort();
    apps
}

fn sorted_mcp_server_names(capabilities: &DeclaredPluginCapabilities) -> Vec<String> {
    let mut names = capabilities
        .mcp
        .iter()
        .map(|mcp| mcp.name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn sorted_hook_names(capabilities: &DeclaredPluginCapabilities) -> Vec<String> {
    let mut names = capabilities
        .hooks
        .iter()
        .map(|hook| hook.name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn loads_default_declared_capability_paths() {
    let tmp = TempDir::new().unwrap();
    let plugin_root = tmp.path().join("plugin");
    fs::create_dir_all(plugin_root.join("skills/example")).unwrap();
    fs::create_dir_all(plugin_root.join("hooks")).unwrap();
    fs::write(
        plugin_root.join(".app.json"),
        r#"{
  "apps": {
    "linear": {
      "id": "connector_linear",
      "category": "Productivity"
    },
    "missing_id": {
      "id": ""
    }
  }
}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "linear": {
      "command": "linear-mcp"
    },
    "docs": {
      "command": "docs-mcp"
    }
  }
}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join("hooks/hooks.json"),
        r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [{ "type": "command", "command": "echo session" }]
      }
    ]
  }
}"#,
    )
    .unwrap();

    let capabilities =
        load_declared_plugin_capabilities(&absolute_path(&plugin_root), &empty_manifest_paths());

    assert_eq!(
        sorted_skill_paths(&capabilities),
        vec![plugin_root.join("skills").display().to_string()]
    );
    assert_eq!(sorted_app_names(&capabilities), vec!["linear".to_string()]);
    assert_eq!(
        sorted_mcp_server_names(&capabilities),
        vec!["docs".to_string(), "linear".to_string()]
    );
    assert_eq!(
        sorted_hook_names(&capabilities),
        vec!["SessionStart".to_string()]
    );
}

#[test]
fn manifest_paths_replace_default_declared_capability_paths() {
    let tmp = TempDir::new().unwrap();
    let plugin_root = tmp.path().join("plugin");
    fs::create_dir_all(plugin_root.join("configured")).unwrap();
    fs::create_dir_all(plugin_root.join("hooks")).unwrap();
    fs::write(
        plugin_root.join(".app.json"),
        r#"{"apps":{"default_app":{"id":"connector_default"}}}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join(".mcp.json"),
        r#"{"mcpServers":{"default_mcp":{"command":"default-mcp"}}}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join("hooks/hooks.json"),
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo default"}]}]}}"#,
    )
    .unwrap();
    let configured_apps = plugin_root.join("configured/apps.json");
    let configured_mcp = plugin_root.join("configured/mcp.json");
    let configured_hooks = plugin_root.join("configured/hooks.json");
    fs::write(
        &configured_apps,
        r#"{"apps":{"configured_app":{"id":"connector_configured"}}}"#,
    )
    .unwrap();
    fs::write(
        &configured_mcp,
        r#"{"configured_mcp":{"command":"configured-mcp"}}"#,
    )
    .unwrap();
    fs::write(
        &configured_hooks,
        r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"echo configured"}]}]}}"#,
    )
    .unwrap();

    let manifest_paths = PluginManifestPaths {
        skills: Some(absolute_path(plugin_root.join("configured").join("skills"))),
        apps: Some(absolute_path(configured_apps)),
        mcp_servers: Some(absolute_path(configured_mcp)),
        hooks: Some(PluginManifestHooks::Paths(vec![absolute_path(
            configured_hooks,
        )])),
    };

    let capabilities =
        load_declared_plugin_capabilities(&absolute_path(&plugin_root), &manifest_paths);

    assert_eq!(
        sorted_skill_paths(&capabilities),
        vec![
            plugin_root
                .join("configured")
                .join("skills")
                .display()
                .to_string()
        ]
    );
    assert_eq!(
        sorted_app_names(&capabilities),
        vec!["configured_app".to_string()]
    );
    assert_eq!(
        sorted_mcp_server_names(&capabilities),
        vec!["configured_mcp".to_string()]
    );
    assert_eq!(
        sorted_hook_names(&capabilities),
        vec!["PostToolUse".to_string()]
    );
}

#[test]
fn loads_inline_declared_hooks() {
    let tmp = TempDir::new().unwrap();
    let plugin_root = tmp.path().join("plugin");
    let hooks_file = serde_json::from_str::<HooksFile>(
        r#"{
  "hooks": {
    "Stop": [
      {
        "hooks": [{ "type": "command", "command": "echo stop" }]
      }
    ]
  }
}"#,
    )
    .unwrap();

    let manifest_paths = PluginManifestPaths {
        skills: None,
        apps: None,
        mcp_servers: None,
        hooks: Some(PluginManifestHooks::Inline(vec![hooks_file])),
    };

    let capabilities =
        load_declared_plugin_capabilities(&absolute_path(&plugin_root), &manifest_paths);

    assert_eq!(sorted_hook_names(&capabilities), vec!["Stop".to_string()]);
}
