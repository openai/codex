use super::*;
use crate::manifest::load_plugin_manifest;
use codex_plugin::PluginId;
use pretty_assertions::assert_eq;

#[test]
fn plugin_mcp_file_supports_mcp_servers_object_format() {
    let parsed = serde_json::from_str::<PluginMcpFile>(
        r#"{
  "mcpServers": {
    "sample": {
      "command": "sample-mcp"
    }
  }
}"#,
    )
    .expect("parse wrapped plugin mcp config")
    .into_mcp_servers();

    assert_eq!(
        parsed,
        HashMap::from([(
            "sample".to_string(),
            serde_json::json!({
                "command": "sample-mcp"
            }),
        )])
    );
}

#[test]
fn plugin_mcp_file_supports_mcp_servers_object_format_with_metadata() {
    let parsed = serde_json::from_str::<PluginMcpFile>(
        r#"{
  "$schema": "https://example.com/plugin-mcp.schema.json",
  "mcpServers": {
    "sample": {
      "command": "sample-mcp"
    }
  }
}"#,
    )
    .expect("parse plugin mcp config with metadata")
    .into_mcp_servers();

    assert_eq!(
        parsed,
        HashMap::from([(
            "sample".to_string(),
            serde_json::json!({
                "command": "sample-mcp"
            }),
        )])
    );
}

#[test]
fn plugin_mcp_file_supports_top_level_server_map_format() {
    let parsed = serde_json::from_str::<PluginMcpFile>(
        r#"{
  "linear": {
    "type": "http",
    "url": "https://mcp.linear.app/mcp"
  }
}"#,
    )
    .expect("parse flat plugin mcp config")
    .into_mcp_servers();

    assert_eq!(
        parsed,
        HashMap::from([(
            "linear".to_string(),
            serde_json::json!({
                "type": "http",
                "url": "https://mcp.linear.app/mcp"
            }),
        )])
    );
}

#[test]
fn curated_plugin_cache_version_shortens_full_git_sha() {
    assert_eq!(
        curated_plugin_cache_version("0123456789abcdef0123456789abcdef01234567"),
        "01234567"
    );
}

#[test]
fn curated_plugin_cache_version_preserves_non_git_sha_versions() {
    assert_eq!(
        curated_plugin_cache_version("export-backup"),
        "export-backup"
    );
    assert_eq!(curated_plugin_cache_version("0123456"), "0123456");
}

fn plugin_id() -> PluginId {
    PluginId::parse("demo-plugin@test-marketplace").expect("plugin id")
}

fn plugin_root() -> (tempfile::TempDir, AbsolutePathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plugin_root =
        AbsolutePathBuf::try_from(tmp.path().join("demo-plugin")).expect("plugin root");
    fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create manifest dir");
    fs::create_dir_all(plugin_root.join("hooks")).expect("create hooks dir");
    (tmp, plugin_root)
}

fn write_manifest(plugin_root: &AbsolutePathBuf, manifest: &str) {
    fs::write(plugin_root.join(".codex-plugin/plugin.json"), manifest).expect("write manifest");
}

fn write_hook_file(plugin_root: &AbsolutePathBuf, relative_path: &str, event: &str, command: &str) {
    fs::write(
        plugin_root.join(relative_path),
        format!(
            r#"{{
  "hooks": {{
    "{event}": [
      {{
        "hooks": [{{ "type": "command", "command": "{command}" }}]
      }}
    ]
  }}
}}"#
        ),
    )
    .expect("write hooks");
}

fn load_sources(plugin_root: &AbsolutePathBuf) -> (Vec<PluginHookSource>, Vec<String>) {
    let manifest = load_plugin_manifest(plugin_root.as_path()).expect("manifest");
    let plugin_data_root = AbsolutePathBuf::try_from(
        plugin_root
            .as_path()
            .parent()
            .expect("plugin root parent")
            .join("plugin-data"),
    )
    .expect("plugin data root");
    load_plugin_hooks(
        plugin_root,
        &plugin_id(),
        &plugin_data_root,
        &manifest.paths,
    )
}

fn assert_sources(sources: &[PluginHookSource], expected_relative_paths: &[&str]) {
    assert_eq!(
        sources
            .iter()
            .map(|source| source.plugin_id.clone())
            .collect::<Vec<_>>(),
        vec![plugin_id(); expected_relative_paths.len()]
    );
    assert_eq!(
        sources
            .iter()
            .map(|source| source.source_relative_path.as_str())
            .collect::<Vec<_>>(),
        expected_relative_paths
    );
    assert_eq!(
        sources
            .iter()
            .map(|source| source.hooks.handler_count())
            .collect::<Vec<_>>(),
        vec![1; expected_relative_paths.len()]
    );
}

#[test]
fn load_plugin_hooks_discovers_default_hooks_file() {
    let (_tmp, plugin_root) = plugin_root();
    write_manifest(&plugin_root, r#"{ "name": "demo-plugin" }"#);
    fs::write(
        plugin_root.join("hooks/hooks.json"),
        r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{ "type": "command", "command": "echo default" }]
      }
    ]
  }
}"#,
    )
    .expect("write hooks");

    let (sources, warnings) = load_sources(&plugin_root);

    assert_eq!(warnings, Vec::<String>::new());
    assert_sources(&sources, &["hooks/hooks.json"]);
}

#[test]
fn load_plugin_hooks_supports_manifest_hook_path() {
    let (_tmp, plugin_root) = plugin_root();
    write_manifest(
        &plugin_root,
        r#"{
  "name": "demo-plugin",
  "hooks": "./hooks/one.json"
}"#,
    );
    write_hook_file(&plugin_root, "hooks/one.json", "PreToolUse", "echo one");

    let (sources, warnings) = load_sources(&plugin_root);

    assert_eq!(warnings, Vec::<String>::new());
    assert_sources(&sources, &["hooks/one.json"]);
}

#[test]
fn load_plugin_hooks_manifest_paths_replace_default_hooks_file() {
    let (_tmp, plugin_root) = plugin_root();
    write_manifest(
        &plugin_root,
        r#"{
  "name": "demo-plugin",
  "hooks": ["./hooks/one.json", "./hooks/two.json"]
}"#,
    );
    write_hook_file(
        &plugin_root,
        "hooks/hooks.json",
        "PreToolUse",
        "echo ignored",
    );
    write_hook_file(&plugin_root, "hooks/one.json", "PreToolUse", "echo one");
    write_hook_file(&plugin_root, "hooks/two.json", "PostToolUse", "echo two");

    let (sources, warnings) = load_sources(&plugin_root);

    assert_eq!(warnings, Vec::<String>::new());
    assert_sources(&sources, &["hooks/one.json", "hooks/two.json"]);
}

#[test]
fn load_plugin_hooks_supports_inline_manifest_hooks() {
    let (_tmp, plugin_root) = plugin_root();
    write_manifest(
        &plugin_root,
        r#"{
  "name": "demo-plugin",
  "hooks": {
    "hooks": {
      "SessionStart": [
        {
          "matcher": "startup",
          "hooks": [{ "type": "command", "command": "echo inline" }]
        }
      ]
    }
  }
}"#,
    );

    let (sources, warnings) = load_sources(&plugin_root);

    assert_eq!(warnings, Vec::<String>::new());
    assert_sources(&sources, &["plugin.json#hooks[0]"]);
}

#[test]
fn load_plugin_hooks_reports_invalid_hook_file() {
    let (_tmp, plugin_root) = plugin_root();
    write_manifest(&plugin_root, r#"{ "name": "demo-plugin" }"#);
    fs::write(plugin_root.join("hooks/hooks.json"), "{ not-json").expect("write invalid hooks");

    let (sources, warnings) = load_sources(&plugin_root);

    assert_eq!(sources, Vec::<PluginHookSource>::new());
    assert_eq!(
        warnings,
        vec![format!(
            "failed to parse plugin hooks config {}: key must be a string at line 1 column 3",
            plugin_root.join("hooks/hooks.json").display()
        )]
    );
}

#[test]
fn load_plugin_hooks_supports_inline_manifest_hook_list() {
    let (_tmp, plugin_root) = plugin_root();
    write_manifest(
        &plugin_root,
        r#"{
  "name": "demo-plugin",
  "hooks": [
    {
      "hooks": {
        "SessionStart": [
          {
            "hooks": [{ "type": "command", "command": "echo inline one" }]
          }
        ]
      }
    },
    {
      "hooks": {
        "Stop": [
          {
            "hooks": [{ "type": "command", "command": "echo inline two" }]
          }
        ]
      }
    }
  ]
}"#,
    );

    let (sources, warnings) = load_sources(&plugin_root);

    assert_eq!(warnings, Vec::<String>::new());
    assert_sources(&sources, &["plugin.json#hooks[0]", "plugin.json#hooks[1]"]);
}

#[test]
fn materialize_git_subdir_uses_sparse_checkout() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let repo = tempfile::tempdir().expect("create git repo");
    let plugin_dir = repo.path().join("plugins/toolkit");
    fs::create_dir_all(&plugin_dir).expect("create plugin directory");
    fs::create_dir_all(repo.path().join("plugins/other")).expect("create other plugin");
    fs::write(plugin_dir.join("marker.txt"), "toolkit").expect("write plugin marker");
    fs::write(repo.path().join("plugins/other/marker.txt"), "other").expect("write other marker");
    fs::write(repo.path().join("root.txt"), "root").expect("write root marker");

    run_git(&["init"], Some(repo.path())).expect("init git repo");
    run_git(
        &["config", "user.email", "test@example.com"],
        Some(repo.path()),
    )
    .expect("configure git email");
    run_git(&["config", "user.name", "Test User"], Some(repo.path())).expect("configure git name");
    run_git(&["add", "."], Some(repo.path())).expect("stage git repo");
    run_git(&["commit", "-m", "init"], Some(repo.path())).expect("commit git repo");

    let materialized = materialize_marketplace_plugin_source(
        codex_home.path(),
        &MarketplacePluginSource::Git {
            url: repo.path().display().to_string(),
            path: Some("plugins/toolkit".to_string()),
            ref_name: None,
            sha: None,
        },
    )
    .expect("materialize git source");

    assert_eq!(
        plugin_dir.file_name(),
        materialized.path.as_path().file_name()
    );
    assert!(materialized.path.as_path().join("marker.txt").is_file());
    let checkout_root = materialized
        .path
        .as_path()
        .parent()
        .and_then(Path::parent)
        .expect("materialized path should be nested under checkout root");
    assert!(!checkout_root.join("root.txt").exists());
    assert!(!checkout_root.join("plugins/other/marker.txt").exists());
}
