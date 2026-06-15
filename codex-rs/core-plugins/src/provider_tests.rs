use super::ExecutorPluginProvider;
use super::ExecutorPluginProviderError;
use super::resolve_plugin_root;
use crate::manifest::parse_plugin_manifest;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemResult;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_plugin::PluginProvider;
use codex_plugin::ResolvedPlugin;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
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

#[derive(Debug, PartialEq, Eq)]
enum FileSystemCall {
    Metadata(AbsolutePathBuf),
    Read(AbsolutePathBuf),
}

struct SyntheticPluginFileSystem {
    plugin_root: AbsolutePathBuf,
    manifest_path: AbsolutePathBuf,
    calls: Mutex<Vec<FileSystemCall>>,
}

impl SyntheticPluginFileSystem {
    fn unsupported<T>() -> FileSystemResult<T> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "operation is not used by plugin resolution",
        ))
    }
}

impl ExecutorFileSystem for SyntheticPluginFileSystem {
    fn canonicalize<'a>(
        &'a self,
        _path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, PathUri> {
        Box::pin(async { Self::unsupported() })
    }

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let path = path.to_abs_path()?;
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(FileSystemCall::Read(path.clone()));
            if path == self.manifest_path {
                Ok(MANIFEST_CONTENTS.as_bytes().to_vec())
            } else {
                Err(io::Error::new(io::ErrorKind::NotFound, "not found"))
            }
        })
    }

    fn write_file<'a>(
        &'a self,
        _path: &'a PathUri,
        _contents: Vec<u8>,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async { Self::unsupported() })
    }

    fn create_directory<'a>(
        &'a self,
        _path: &'a PathUri,
        _options: CreateDirectoryOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async { Self::unsupported() })
    }

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        Box::pin(async move {
            let path = path.to_abs_path()?;
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(FileSystemCall::Metadata(path.clone()));
            let (is_directory, is_file) = if path == self.plugin_root {
                (true, false)
            } else if path == self.manifest_path {
                (false, true)
            } else {
                return Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
            };
            Ok(FileMetadata {
                is_directory,
                is_file,
                is_symlink: false,
                size: 0,
                created_at_ms: 0,
                modified_at_ms: 0,
            })
        })
    }

    fn read_directory<'a>(
        &'a self,
        _path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        Box::pin(async { Self::unsupported() })
    }

    fn remove<'a>(
        &'a self,
        _path: &'a PathUri,
        _options: RemoveOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async { Self::unsupported() })
    }

    fn copy<'a>(
        &'a self,
        _source_path: &'a PathUri,
        _destination_path: &'a PathUri,
        _options: CopyOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async { Self::unsupported() })
    }
}

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
async fn plugin_root_resolution_uses_supplied_executor_file_system() {
    let temp_dir = tempdir().expect("tempdir");
    let plugin_root = temp_dir.path().join("executor-only-plugin");
    assert!(!plugin_root.exists());
    let plugin_root =
        AbsolutePathBuf::from_absolute_path_checked(plugin_root).expect("absolute plugin root");
    let manifest_path = plugin_root.join(".codex-plugin/plugin.json");
    let parsed_manifest = parse_plugin_manifest(
        plugin_root.as_path(),
        manifest_path.as_path(),
        MANIFEST_CONTENTS,
    )
    .expect("parse manifest");
    let file_system = SyntheticPluginFileSystem {
        plugin_root: plugin_root.clone(),
        manifest_path: manifest_path.clone(),
        calls: Mutex::new(Vec::new()),
    };
    let resolved = resolve_plugin_root(
        &selected_root("selected-demo", "executor-test", plugin_root.as_path()),
        plugin_root.clone(),
        &file_system,
    )
    .await
    .expect("resolve executor plugin");

    assert_eq!(
        resolved,
        Some(
            ResolvedPlugin::from_environment(
                "selected-demo".to_string(),
                "executor-test".to_string(),
                plugin_root.clone(),
                manifest_path.clone(),
                parsed_manifest,
            )
            .expect("valid expected descriptor")
        )
    );
    assert_eq!(
        *file_system
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![
            FileSystemCall::Metadata(plugin_root),
            FileSystemCall::Metadata(manifest_path.clone()),
            FileSystemCall::Read(manifest_path),
        ]
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
