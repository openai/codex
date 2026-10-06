use super::ExecutorPluginProvider;
use super::ExecutorPluginProviderError;
use super::PluginRootOwnership;
use super::find_manifest;
use super::resolve_plugin_root;
use crate::manifest::parse_plugin_manifest_uri;
use codex_exec_server::CapabilityRootDiscovery;
use codex_exec_server::CapabilityTextFile;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::FileSystemResult;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::GetMetadataOptions;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::ReadFileOptions;
use codex_exec_server::RemoveOptions;
use codex_exec_server::WalkOptions;
use codex_exec_server::WalkOutcome;
use codex_exec_server::WriteFileOptions;
use codex_exec_server_test_support::environment_manager_without_environments;
use codex_plugin::ResolvedPlugin;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::DISCOVERABLE_PLUGIN_MANIFEST_PATHS;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use tempfile::tempdir;
use tokio::sync::Semaphore;

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
    Metadata(PathUri),
    SandboxedMetadata {
        path: PathUri,
        sandbox: Box<FileSystemSandboxContext>,
    },
    Read(PathUri),
}

struct SyntheticPluginFileSystem {
    plugin_root: PathUri,
    manifest_path: PathUri,
    calls: Mutex<Vec<FileSystemCall>>,
    metadata_gate: Option<Arc<Semaphore>>,
    metadata_delay: Option<(PathUri, Arc<Semaphore>)>,
    metadata_error: Option<PathUri>,
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
        _options: ReadFileOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(FileSystemCall::Read(path.clone()));
            if path == &self.manifest_path {
                Ok(MANIFEST_CONTENTS.as_bytes().to_vec())
            } else {
                Err(io::Error::new(io::ErrorKind::NotFound, "not found"))
            }
        })
    }

    fn read_file_stream<'a>(
        &'a self,
        _path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileSystemReadStream> {
        Box::pin(async { Self::unsupported() })
    }

    fn write_file<'a>(
        &'a self,
        _path: &'a PathUri,
        _contents: Vec<u8>,
        _options: WriteFileOptions,
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
        _options: GetMetadataOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(match sandbox {
                    Some(sandbox) => FileSystemCall::SandboxedMetadata {
                        path: path.clone(),
                        sandbox: Box::new(sandbox.clone()),
                    },
                    None => FileSystemCall::Metadata(path.clone()),
                });
            if let Some(gate) = &self.metadata_gate {
                gate.acquire().await.expect("open metadata gate").forget();
            }
            if let Some((delayed_path, gate)) = &self.metadata_delay
                && delayed_path == path
            {
                gate.acquire().await.expect("open delayed gate").forget();
            }
            if self.metadata_error.as_ref() == Some(path) {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
            }
            let (is_directory, is_file) = if path == &self.plugin_root {
                (true, false)
            } else if path == &self.manifest_path {
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

    fn walk<'a>(
        &'a self,
        _path: &'a PathUri,
        _options: WalkOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, WalkOutcome> {
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
            path: PathUri::from_host_native_path(path).expect("path URI"),
        },
    }
}

fn selected_root_uri(id: &str, environment_id: &str, path: PathUri) -> SelectedCapabilityRoot {
    SelectedCapabilityRoot {
        id: id.to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: environment_id.to_string(),
            path,
        },
    }
}

#[tokio::test]
async fn root_ownership_survives_malformed_ancestor_manifests() {
    let directory = tempdir().expect("tempdir");
    let root = directory.path().join("package/skills");
    fs::create_dir_all(&root).expect("create skills root");
    let provider = ExecutorPluginProvider::new(Arc::new(EnvironmentManager::default_for_tests()));
    let selected = selected_root("example@marketplace", LOCAL_ENVIRONMENT_ID, &root);
    assert_eq!(
        provider
            .root_ownership(&selected, /*discovery*/ None, /*sandbox*/ None)
            .await
            .unwrap(),
        PluginRootOwnership::Standalone
    );
    write_manifest(
        root.parent().unwrap(),
        ".codex-plugin/plugin.json",
        "{broken",
    );
    assert_eq!(
        provider
            .root_ownership(&selected, /*discovery*/ None, /*sandbox*/ None)
            .await
            .unwrap(),
        PluginRootOwnership::Plugin
    );
    let mut discovery = CapabilityRootDiscovery {
        id: selected.id.clone(),
        path: PathUri::from_host_native_path(&root).unwrap(),
        plugin: None,
        skills: Vec::new(),
        namespace_manifests: Vec::new(),
        warnings: vec!["unreadable manifest".into()],
        error: None,
    };
    assert_eq!(
        provider
            .root_ownership(&selected, Some(&discovery), /*sandbox*/ None)
            .await
            .unwrap(),
        PluginRootOwnership::Plugin
    );
    discovery.error = Some("discovery failed".into());
    assert!(matches!(
        provider
            .root_ownership(&selected, Some(&discovery), /*sandbox*/ None)
            .await,
        Err(ExecutorPluginProviderError::DiscoveryFailed { .. })
    ));
}

#[tokio::test]
async fn discovery_ownership_uses_ancestors_not_nested_plugins() {
    let directory = tempdir().expect("tempdir");
    let path = PathUri::from_host_native_path(directory.path()).unwrap();
    let selected = selected_root_uri("example@marketplace", "executor", path.clone());
    let provider =
        ExecutorPluginProvider::new(Arc::new(environment_manager_without_environments()));
    let mut discovery = CapabilityRootDiscovery {
        id: selected.id.clone(),
        path: path.clone(),
        plugin: None,
        skills: Vec::new(),
        namespace_manifests: vec![CapabilityTextFile {
            path: path.join("nested/.codex-plugin/plugin.json").unwrap(),
            contents: "{broken".into(),
        }],
        warnings: Vec::new(),
        error: None,
    };
    assert_eq!(
        provider
            .root_ownership(&selected, Some(&discovery), /*sandbox*/ None)
            .await
            .unwrap(),
        PluginRootOwnership::Standalone
    );
    discovery.namespace_manifests[0].path = path.join(".codex-plugin/plugin.json").unwrap();
    assert_eq!(
        provider
            .root_ownership(&selected, Some(&discovery), /*sandbox*/ None)
            .await
            .unwrap(),
        PluginRootOwnership::Plugin
    );
    assert!(matches!(
        provider
            .root_ownership(&selected, /*discovery*/ None, /*sandbox*/ None)
            .await,
        Err(ExecutorPluginProviderError::UnavailableEnvironment { .. })
    ));
}

#[tokio::test]
async fn ancestor_manifest_probes_pipeline_and_preserve_error_order() {
    let root = PathUri::parse("file:///a/b/c/d/e/f").expect("selected root URI");
    let owner = PathUri::parse("file:///a/b/c").expect("owner URI");
    let manifest = owner.join(".codex-plugin/plugin.json").unwrap();
    let earlier_error = root.join(".codex-plugin/plugin.json").unwrap();
    let later_error = owner.join(".claude-plugin/plugin.json").unwrap();
    let expected_probes = std::iter::successors(Some(root.clone()), PathUri::parent)
        .flat_map(|ancestor| {
            DISCOVERABLE_PLUGIN_MANIFEST_PATHS
                .iter()
                .map(move |marker| ancestor.join(marker).unwrap())
        })
        .collect::<HashSet<_>>();

    for (metadata_error, expected) in [
        (None, Ok(Some(manifest.clone()))),
        (
            Some(earlier_error.clone()),
            Err((earlier_error.clone(), io::ErrorKind::PermissionDenied)),
        ),
        (Some(later_error), Ok(Some(manifest.clone()))),
    ] {
        let gate = Arc::new(Semaphore::new(/*permits*/ 0));
        let earliest_gate = Arc::new(Semaphore::new(/*permits*/ 0));
        let file_system = SyntheticPluginFileSystem {
            // A directory named like a manifest must not count as plugin ownership.
            plugin_root: earlier_error.clone(),
            manifest_path: manifest.clone(),
            calls: Mutex::new(Vec::new()),
            metadata_gate: Some(Arc::clone(&gate)),
            metadata_delay: Some((earlier_error.clone(), Arc::clone(&earliest_gate))),
            metadata_error,
        };
        let selected = selected_root_uri("example@marketplace", "executor", root.clone());
        let mut resolution = Box::pin(find_manifest(
            &selected,
            std::iter::successors(Some(root.clone()), PathUri::parent),
            &file_system,
            /*sandbox*/ None,
        ));
        assert!(futures::poll!(resolution.as_mut()).is_pending());
        let probes = file_system
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|call| match call {
                FileSystemCall::Metadata(path) => path.clone(),
                call => panic!("unexpected filesystem call: {call:?}"),
            })
            .collect::<HashSet<_>>();
        assert_eq!(probes, expected_probes);
        gate.add_permits(probes.len());
        // Later successes/errors cannot outrank the earliest still-pending candidate.
        assert!(futures::poll!(resolution.as_mut()).is_pending());
        earliest_gate.add_permits(/*n*/ 1);
        let result = resolution.await.map_err(|error| {
            let ExecutorPluginProviderError::InspectManifest { path, source, .. } = error else {
                panic!("unexpected manifest resolution error: {error}");
            };
            (path, source.kind())
        });
        assert_eq!(result, expected);
    }
}

#[tokio::test]
async fn manifest_presence_uses_sandboxed_metadata_without_reading_contents() {
    let directory = tempdir().expect("tempdir");
    let plugin_root = PathUri::from_host_native_path(directory.path()).expect("root URI");
    let manifest_path = plugin_root
        .join(".codex-plugin/plugin.json")
        .expect("manifest URI");
    let file_system = SyntheticPluginFileSystem {
        plugin_root: plugin_root.clone(),
        manifest_path: manifest_path.clone(),
        calls: Mutex::new(Vec::new()),
        metadata_gate: None,
        metadata_delay: None,
        metadata_error: None,
    };
    let sandbox =
        FileSystemSandboxContext::from_permission_profile(Default::default(), plugin_root.clone());
    assert_eq!(
        find_manifest(
            &selected_root_uri("selected-demo", "executor-test", plugin_root.clone()),
            std::iter::once(plugin_root.clone()),
            &file_system,
            Some(&sandbox),
        )
        .await
        .expect("probe manifest"),
        Some(manifest_path.clone()),
    );
    assert_eq!(
        *file_system
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![FileSystemCall::SandboxedMetadata {
            path: manifest_path,
            sandbox: Box::new(sandbox)
        }],
    );
}

#[tokio::test]
async fn plugin_root_resolution_uses_supplied_executor_file_system() {
    let temp_dir = tempdir().expect("tempdir");
    let plugin_root = temp_dir.path().join("executor-only-plugin");
    assert!(!plugin_root.exists());
    let plugin_root = PathUri::from_host_native_path(&plugin_root).expect("plugin root URI");
    let manifest_path = plugin_root
        .join(".codex-plugin/plugin.json")
        .expect("manifest URI");
    let parsed_manifest =
        parse_plugin_manifest_uri(&plugin_root, &manifest_path, MANIFEST_CONTENTS)
            .expect("parse manifest");
    let file_system = SyntheticPluginFileSystem {
        plugin_root: plugin_root.clone(),
        manifest_path: manifest_path.clone(),
        calls: Mutex::new(Vec::new()),
        metadata_gate: None,
        metadata_delay: None,
        metadata_error: None,
    };
    let resolved = resolve_plugin_root(
        &selected_root_uri("selected-demo", "executor-test", plugin_root.clone()),
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
async fn plugin_root_resolution_accepts_foreign_executor_file_uri() {
    let plugin_root = PathUri::parse("file:///C:/plugins/foo").expect("Windows plugin root URI");
    let manifest_path = plugin_root
        .join(".codex-plugin/plugin.json")
        .expect("manifest URI");
    let parsed_manifest =
        parse_plugin_manifest_uri(&plugin_root, &manifest_path, MANIFEST_CONTENTS)
            .expect("parse manifest");
    let file_system = SyntheticPluginFileSystem {
        plugin_root: plugin_root.clone(),
        manifest_path: manifest_path.clone(),
        calls: Mutex::new(Vec::new()),
        metadata_gate: None,
        metadata_delay: None,
        metadata_error: None,
    };
    let selected_root = selected_root_uri("selected-demo", "executor-test", plugin_root.clone());
    let resolved = resolve_plugin_root(&selected_root, plugin_root.clone(), &file_system)
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
        .resolve_bound(&selected_root(
            "standalone",
            LOCAL_ENVIRONMENT_ID,
            &standalone_root,
        ))
        .await
        .expect("resolve standalone root");

    assert!(resolved.is_none());
}

#[tokio::test]
async fn root_agent_plugin_manifest_is_not_an_executor_plugin() {
    let temp_dir = tempdir().expect("tempdir");
    let plugin_root = temp_dir.path().join("agent-plugin");
    write_manifest(
        &plugin_root,
        "plugin.json",
        r#"{
          "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
          "name": "agent-plugin",
          "version": "1.0.0"
        }"#,
    );
    let provider = ExecutorPluginProvider::new(Arc::new(EnvironmentManager::default_for_tests()));

    let resolved = provider
        .resolve_bound(&selected_root(
            "agent-plugin",
            LOCAL_ENVIRONMENT_ID,
            &plugin_root,
        ))
        .await
        .expect("resolve selected root");

    assert!(resolved.is_none());
}

#[tokio::test]
async fn unavailable_environment_does_not_fall_back_to_host_filesystem() {
    let temp_dir = tempdir().expect("tempdir");
    let plugin_root = temp_dir.path().join("host-plugin");
    write_manifest(&plugin_root, ".codex-plugin/plugin.json", MANIFEST_CONTENTS);
    let provider =
        ExecutorPluginProvider::new(Arc::new(environment_manager_without_environments()));

    let err = provider
        .resolve_bound(&selected_root("host-plugin", "missing", &plugin_root))
        .await
        .map(|_| ())
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
        PathUri::from_host_native_path(plugin_root.join(".codex-plugin/plugin.json"))
            .expect("manifest URI");
    let provider = ExecutorPluginProvider::new(Arc::new(EnvironmentManager::default_for_tests()));

    let err = provider
        .resolve_bound(&selected_root(
            "selected-demo",
            LOCAL_ENVIRONMENT_ID,
            &plugin_root,
        ))
        .await
        .map(|_| ())
        .expect_err("malformed preferred manifest should fail");

    assert!(
        std::error::Error::source(&err)
            .is_some_and(<dyn std::error::Error>::is::<serde_json::Error>)
    );
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
