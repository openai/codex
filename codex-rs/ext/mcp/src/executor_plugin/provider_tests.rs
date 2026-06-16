use super::DEFAULT_MCP_CONFIG_FILE;
use super::ExecutorPluginMcpProviderError;
use super::load_from_file_system;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemResult;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_plugin::ResolvedPlugin;
use codex_plugin::manifest::PluginManifest;
use codex_plugin::manifest::PluginManifestMcpServers;
use codex_plugin::manifest::PluginManifestPaths;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::io;
use std::sync::Mutex;

const MCP_CONFIG_CONTENTS: &str = r#"{
  "mcpServers": {
    "demo": {"command": "demo-mcp", "environment_id": "local"},
    "hosted": {"url": "https://example.com/mcp"}
  }
}"#;

struct SyntheticExecutorFileSystem {
    config_path: AbsolutePathBuf,
    config_contents: Option<&'static str>,
    reads: Mutex<Vec<AbsolutePathBuf>>,
}

impl SyntheticExecutorFileSystem {
    fn unsupported<T>() -> FileSystemResult<T> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "operation is not used by executor MCP provider tests",
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
            let path = path.to_abs_path()?;
            self.reads
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(path.clone());
            if path != self.config_path {
                return Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
            }
            self.config_contents
                .map(|contents| contents.as_bytes().to_vec())
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not found"))
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
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let plugin_root =
        AbsolutePathBuf::from_absolute_path_checked(temp_dir.path().join("executor-only-plugin"))
            .expect("absolute plugin root");
    assert!(!plugin_root.as_path().exists());
    let config_path = plugin_root.join("config/mcp.json");
    let plugin = resolved_plugin(
        &plugin_root,
        Some(PluginManifestMcpServers::Path(config_path.clone())),
    );
    let file_system = SyntheticExecutorFileSystem {
        config_path: config_path.clone(),
        config_contents: Some(MCP_CONFIG_CONTENTS),
        reads: Mutex::new(Vec::new()),
    };

    let servers = load_from_file_system(&plugin, &plugin_root, &file_system)
        .await
        .expect("load executor MCP config");

    assert_eq!(
        servers,
        vec![(
            "demo".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: "demo-mcp".to_string(),
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: Some(plugin_root.to_path_buf()),
                },
                environment_id: "executor-test".to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            },
        )]
    );
    assert_eq!(reads(&file_system), vec![config_path]);
}

#[tokio::test]
async fn reads_manifest_object_config_without_executor_file_system_access() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let plugin_root = AbsolutePathBuf::from_absolute_path_checked(temp_dir.path().join("plugin"))
        .expect("absolute plugin root");
    let config_path = plugin_root.join(DEFAULT_MCP_CONFIG_FILE);
    let plugin = resolved_plugin(
        &plugin_root,
        Some(PluginManifestMcpServers::Object(
            r#"{"counter":{"command":"counter-mcp","environment_id":"local"}}"#.to_string(),
        )),
    );
    let file_system = SyntheticExecutorFileSystem {
        config_path,
        config_contents: None,
        reads: Mutex::new(Vec::new()),
    };

    let servers = load_from_file_system(&plugin, &plugin_root, &file_system)
        .await
        .expect("load manifest object executor MCP config");

    assert_eq!(
        servers,
        vec![(
            "counter".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: "counter-mcp".to_string(),
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: Some(plugin_root.to_path_buf()),
                },
                environment_id: "executor-test".to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            },
        )]
    );
    assert_eq!(reads(&file_system), Vec::new());
}

#[tokio::test]
async fn missing_default_config_is_empty() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let plugin_root = AbsolutePathBuf::from_absolute_path_checked(temp_dir.path().join("plugin"))
        .expect("absolute plugin root");
    let config_path = plugin_root.join(DEFAULT_MCP_CONFIG_FILE);
    let plugin = resolved_plugin(&plugin_root, /*mcp_servers*/ None);
    let file_system = SyntheticExecutorFileSystem {
        config_path: config_path.clone(),
        config_contents: None,
        reads: Mutex::new(Vec::new()),
    };

    let servers = load_from_file_system(&plugin, &plugin_root, &file_system)
        .await
        .expect("missing default config should be ignored");

    assert_eq!(servers, Vec::new());
    assert_eq!(reads(&file_system), vec![config_path]);
}

#[tokio::test]
async fn malformed_declared_config_is_an_error() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let plugin_root = AbsolutePathBuf::from_absolute_path_checked(temp_dir.path().join("plugin"))
        .expect("absolute plugin root");
    let config_path = plugin_root.join("mcp.json");
    let plugin = resolved_plugin(
        &plugin_root,
        Some(PluginManifestMcpServers::Path(config_path.clone())),
    );
    let file_system = SyntheticExecutorFileSystem {
        config_path: config_path.clone(),
        config_contents: Some("{not-json"),
        reads: Mutex::new(Vec::new()),
    };

    let err = load_from_file_system(&plugin, &plugin_root, &file_system)
        .await
        .expect_err("malformed declared config should fail");

    let ExecutorPluginMcpProviderError::ParseConfig {
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

fn resolved_plugin(
    plugin_root: &AbsolutePathBuf,
    mcp_servers: Option<PluginManifestMcpServers<AbsolutePathBuf>>,
) -> ResolvedPlugin {
    ResolvedPlugin::from_environment(
        "selected-root".to_string(),
        "executor-test".to_string(),
        plugin_root.clone(),
        plugin_root.join(".codex-plugin/plugin.json"),
        PluginManifest {
            name: "demo-plugin".to_string(),
            version: None,
            description: None,
            keywords: Vec::new(),
            paths: PluginManifestPaths {
                skills: None,
                mcp_servers,
                apps: None,
                hooks: None,
            },
            interface: None,
        },
    )
    .expect("valid plugin descriptor")
}

fn reads(file_system: &SyntheticExecutorFileSystem) -> Vec<AbsolutePathBuf> {
    file_system
        .reads
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}
