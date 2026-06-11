use super::ExecutorPluginProvider;
use super::ExecutorPluginProviderError;
use crate::manifest::parse_plugin_manifest;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_plugin::PluginProvider;
use codex_plugin::ResolvedPlugin;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;

const MANIFEST_CONTENTS: &str = r#"{
  "name": "demo-plugin",
  "version": " 1.2.3 ",
  "description": "Demo plugin",
  "skills": "./skills",
  "mcpServers": "./.mcp.json",
  "apps": "./.app.json",
  "interface": {
    "displayName": "Demo Plugin",
    "composerIcon": "./assets/icon.svg"
  }
}"#;

fn write_manifest(plugin_root: &Path, relative_path: &str, contents: &str) {
    let manifest_path = plugin_root.join(relative_path);
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create manifest parent");
    fs::write(manifest_path, contents).expect("write manifest");
}

fn selected_root(id: &str, environment_id: &str, path: &Path) -> SelectedCapabilityRoot {
    SelectedCapabilityRoot {
        id: id.to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: environment_id.to_string(),
            path: path.to_string_lossy().into_owned(),
        },
    }
}

#[tokio::test]
async fn executor_resolves_a_complete_authority_bound_descriptor() {
    let temp_dir = tempdir().expect("tempdir");
    let plugin_root = temp_dir.path().join("demo-plugin");
    write_manifest(&plugin_root, ".codex-plugin/plugin.json", MANIFEST_CONTENTS);
    let manifest_path = plugin_root.join(".codex-plugin/plugin.json");
    let parsed_manifest = parse_plugin_manifest(&plugin_root, &manifest_path, MANIFEST_CONTENTS)
        .expect("parse manifest");
    let provider = ExecutorPluginProvider::new(Arc::new(EnvironmentManager::default_for_tests()));

    let resolved = provider
        .resolve(&selected_root(
            "selected-demo",
            LOCAL_ENVIRONMENT_ID,
            &plugin_root,
        ))
        .await
        .expect("resolve executor plugin");

    assert_eq!(
        resolved,
        Some(ResolvedPlugin::from_environment(
            "selected-demo".to_string(),
            LOCAL_ENVIRONMENT_ID.to_string(),
            AbsolutePathBuf::from_absolute_path_checked(plugin_root.clone())
                .expect("absolute plugin root"),
            AbsolutePathBuf::from_absolute_path_checked(manifest_path)
                .expect("absolute manifest path"),
            parsed_manifest,
        ))
    );
}

#[tokio::test]
async fn standalone_capability_root_is_not_a_plugin() {
    let temp_dir = tempdir().expect("tempdir");
    let standalone_root = temp_dir.path().join("standalone-skill");
    fs::create_dir_all(&standalone_root).expect("create standalone root");
    let provider = ExecutorPluginProvider::new(Arc::new(EnvironmentManager::default_for_tests()));

    let resolved = provider
        .resolve(&selected_root(
            "standalone",
            LOCAL_ENVIRONMENT_ID,
            &standalone_root,
        ))
        .await
        .expect("resolve standalone root");

    assert_eq!(resolved, None);
}

#[tokio::test]
async fn unavailable_environment_does_not_fall_back_to_host_filesystem() {
    let temp_dir = tempdir().expect("tempdir");
    let plugin_root = temp_dir.path().join("host-plugin");
    write_manifest(&plugin_root, ".codex-plugin/plugin.json", MANIFEST_CONTENTS);
    let provider =
        ExecutorPluginProvider::new(Arc::new(EnvironmentManager::without_environments()));

    let err = provider
        .resolve(&selected_root("host-plugin", "missing", &plugin_root))
        .await
        .expect_err("missing environment should fail");

    assert_eq!(
        err.to_string(),
        "selected capability root `host-plugin` references unavailable environment `missing`"
    );
}

#[tokio::test]
async fn malformed_preferred_manifest_does_not_fall_through_to_alternate() {
    let temp_dir = tempdir().expect("tempdir");
    let plugin_root = temp_dir.path().join("demo-plugin");
    write_manifest(&plugin_root, ".codex-plugin/plugin.json", "{not-json");
    write_manifest(
        &plugin_root,
        ".claude-plugin/plugin.json",
        MANIFEST_CONTENTS,
    );
    let expected_path =
        AbsolutePathBuf::from_absolute_path_checked(plugin_root.join(".codex-plugin/plugin.json"))
            .expect("absolute manifest path");
    let provider = ExecutorPluginProvider::new(Arc::new(EnvironmentManager::default_for_tests()));

    let err = provider
        .resolve(&selected_root(
            "selected-demo",
            LOCAL_ENVIRONMENT_ID,
            &plugin_root,
        ))
        .await
        .expect_err("malformed preferred manifest should fail");

    let ExecutorPluginProviderError::ParseManifest {
        root_id,
        path,
        source: _,
    } = err
    else {
        panic!("expected parse error");
    };
    assert_eq!(
        (root_id, path),
        ("selected-demo".to_string(), expected_path)
    );
}

#[tokio::test]
async fn executor_root_must_be_an_explicit_absolute_path() {
    let provider = ExecutorPluginProvider::new(Arc::new(EnvironmentManager::default_for_tests()));
    let selected_root = SelectedCapabilityRoot {
        id: "selected-demo".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            path: "~/plugins/demo".to_string(),
        },
    };

    let err = provider
        .resolve(&selected_root)
        .await
        .expect_err("home-relative executor path should fail");

    assert_eq!(
        err.to_string(),
        "selected capability root `selected-demo` has invalid path `~/plugins/demo`: executor path must be absolute"
    );
}
