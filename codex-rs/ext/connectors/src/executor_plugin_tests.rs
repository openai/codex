use super::DEFAULT_APP_CONFIG_FILE;
use super::ExecutorPluginConnectorProviderError;
use super::load_from_file_system;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::FileSystemResult;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_plugin::AppConnectorId;
use codex_plugin::AppDeclaration;
use codex_plugin::ResolvedPlugin;
use codex_plugin::manifest::PluginManifest;
use codex_plugin::manifest::PluginManifestInterface;
use codex_plugin::manifest::PluginManifestPaths;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::io;
use std::sync::Mutex;

const APP_CONFIG_CONTENTS: &str = r#"{
  "apps": {
    "calendar": {"id": "connector_calendar", "category": "productivity"},
    "drive": {"id": "connector_drive"},
    "calendar_alias": {"id": "connector_calendar"},
    "blank": {"id": "  "}
  }
}"#;

#[derive(Clone, Copy)]
enum ReadOutcome {
    Contents(&'static str),
    Error(io::ErrorKind),
}

struct SyntheticExecutorFileSystem {
    config_path: PathUri,
    outcome: ReadOutcome,
    reads: Mutex<Vec<PathUri>>,
}

impl SyntheticExecutorFileSystem {
    fn unsupported<T>() -> FileSystemResult<T> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "operation is not used by executor connector provider tests",
        ))
    }
}

impl ExecutorFileSystem for SyntheticExecutorFileSystem {
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
            self.reads
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(path.clone());
            if path != &self.config_path {
                return Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
            }
            match self.outcome {
                ReadOutcome::Contents(contents) => Ok(contents.as_bytes().to_vec()),
                ReadOutcome::Error(kind) => Err(io::Error::new(kind, "synthetic read error")),
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
        _path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        Box::pin(async { Self::unsupported() })
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

#[tokio::test]
async fn reads_declared_config_only_through_executor_file_system() {
    let plugin_root = path_uri("file:///C:/executor/plugins/calendar");
    let config_path = plugin_root
        .join("config/apps.json")
        .expect("app config URI");
    let plugin = resolved_plugin(&plugin_root, Some(config_path.clone()));
    let file_system = file_system(
        config_path.clone(),
        ReadOutcome::Contents(APP_CONFIG_CONTENTS),
    );

    let declarations = load_from_file_system(&plugin, &plugin_root, &file_system)
        .await
        .expect("load executor app config");

    assert_eq!(
        declarations,
        vec![
            AppDeclaration {
                name: "calendar".to_string(),
                connector_id: AppConnectorId("connector_calendar".to_string()),
                category: Some("productivity".to_string()),
            },
            AppDeclaration {
                name: "drive".to_string(),
                connector_id: AppConnectorId("connector_drive".to_string()),
                category: None,
            },
            AppDeclaration {
                name: "calendar_alias".to_string(),
                connector_id: AppConnectorId("connector_calendar".to_string()),
                category: None,
            },
        ]
    );
    assert_eq!(reads(&file_system), vec![config_path]);
}

#[tokio::test]
async fn missing_default_config_returns_no_declarations() {
    let plugin_root = path_uri("file:///C:/executor/plugins/calendar");
    let config_path = plugin_root
        .join(DEFAULT_APP_CONFIG_FILE)
        .expect("default app config URI");
    let plugin = resolved_plugin(&plugin_root, /*apps*/ None);
    let file_system = file_system(
        config_path.clone(),
        ReadOutcome::Error(io::ErrorKind::NotFound),
    );

    let declarations = load_from_file_system(&plugin, &plugin_root, &file_system)
        .await
        .expect("missing default app config should be ignored");

    assert_eq!(declarations, Vec::new());
    assert_eq!(reads(&file_system), vec![config_path]);
}

#[tokio::test]
async fn missing_declared_config_returns_no_declarations() {
    let plugin_root = path_uri("file:///opt/plugins/calendar");
    let config_path = plugin_root
        .join("config/apps.json")
        .expect("app config URI");
    let plugin = resolved_plugin(&plugin_root, Some(config_path.clone()));
    let file_system = file_system(
        config_path.clone(),
        ReadOutcome::Error(io::ErrorKind::NotFound),
    );

    let declarations = load_from_file_system(&plugin, &plugin_root, &file_system)
        .await
        .expect("missing declared app config should be ignored");

    assert_eq!(declarations, Vec::new());
    assert_eq!(reads(&file_system), vec![config_path]);
}

#[tokio::test]
async fn malformed_config_reports_declared_path_without_fallback() {
    let plugin_root = path_uri("file:///opt/plugins/calendar");
    let config_path = plugin_root
        .join("config/apps.json")
        .expect("app config URI");
    let plugin = resolved_plugin(&plugin_root, Some(config_path.clone()));
    let file_system = file_system(config_path.clone(), ReadOutcome::Contents("{not-json"));

    let err = load_from_file_system(&plugin, &plugin_root, &file_system)
        .await
        .expect_err("malformed app config should fail");

    let ExecutorPluginConnectorProviderError::ParseConfig {
        plugin_id,
        path,
        source: _,
    } = err
    else {
        panic!("expected parse error");
    };
    assert_eq!(
        (plugin_id, path),
        ("selected-root".to_string(), config_path.clone())
    );
    assert_eq!(reads(&file_system), vec![config_path]);
}

#[tokio::test]
async fn non_not_found_read_error_reports_declared_path() {
    let plugin_root = path_uri("file:///opt/plugins/calendar");
    let config_path = plugin_root
        .join("config/apps.json")
        .expect("app config URI");
    let plugin = resolved_plugin(&plugin_root, Some(config_path.clone()));
    let file_system = file_system(
        config_path.clone(),
        ReadOutcome::Error(io::ErrorKind::PermissionDenied),
    );

    let err = load_from_file_system(&plugin, &plugin_root, &file_system)
        .await
        .expect_err("executor read failure should be reported");

    let ExecutorPluginConnectorProviderError::ReadConfig {
        plugin_id,
        path,
        source: _,
    } = err
    else {
        panic!("expected read error");
    };
    assert_eq!(
        (plugin_id, path),
        ("selected-root".to_string(), config_path.clone())
    );
    assert_eq!(reads(&file_system), vec![config_path]);
}

fn resolved_plugin(plugin_root: &PathUri, apps: Option<PathUri>) -> ResolvedPlugin {
    ResolvedPlugin::from_environment(
        "selected-root".to_string(),
        "executor-test".to_string(),
        plugin_root.clone(),
        plugin_root
            .join(".claude-plugin/plugin.json")
            .expect("manifest URI"),
        PluginManifest {
            name: "calendar".to_string(),
            version: None,
            description: None,
            keywords: Vec::new(),
            paths: PluginManifestPaths {
                skills: Vec::new(),
                mcp_servers: None,
                apps,
                hooks: None,
            },
            interface: Some(PluginManifestInterface {
                display_name: Some("Executor Calendar".to_string()),
                ..PluginManifestInterface::default()
            }),
        },
    )
    .expect("valid executor plugin descriptor")
}

fn path_uri(uri: &str) -> PathUri {
    PathUri::parse(uri).expect("valid path URI")
}

fn file_system(config_path: PathUri, outcome: ReadOutcome) -> SyntheticExecutorFileSystem {
    SyntheticExecutorFileSystem {
        config_path,
        outcome,
        reads: Mutex::new(Vec::new()),
    }
}

fn reads(file_system: &SyntheticExecutorFileSystem) -> Vec<PathUri> {
    file_system
        .reads
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}
