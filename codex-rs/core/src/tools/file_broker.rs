use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo;
use codex_tools::FileRef;
use codex_tools::FileScheme;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

/// Minimal broker for moving bytes across Code Mode file refs.
///
/// This POC intentionally implements only the workspace environment provider.
/// Connector, Library, and remote-environment adapters can plug in behind this
/// boundary without changing model-facing tool contracts.
#[derive(Debug)]
pub(crate) struct CodeModeFileBroker {
    current_root: PathBuf,
}

const MAX_DATA_URI_EXPORT_BYTES: usize = 8 * 1024 * 1024;

const CONFIGURED_BROKER_ROUTES: &[ConfiguredBrokerRoute] = &[ConfiguredBrokerRoute {
    provider: "google_drive",
    operation: "upload",
    required_server: CODEX_APPS_MCP_SERVER_NAME,
    required_namespace: "google_drive",
    required_tool: "upload_file",
}];

impl CodeModeFileBroker {
    pub(crate) fn new(current_root: impl Into<PathBuf>) -> Self {
        Self {
            current_root: current_root.into(),
        }
    }

    pub(crate) fn read_to_bytes(&self, source: &FileRef) -> Result<Vec<u8>, FileBrokerError> {
        let source_path = self.resolve_env_path(source)?;
        fs::read(&source_path).map_err(|source| FileBrokerError::Io {
            action: "read",
            source,
        })
    }

    pub(crate) fn write_bytes(
        &self,
        target: &FileRef,
        bytes: &[u8],
    ) -> Result<FileBrokerWriteResult, FileBrokerError> {
        let target_path = self.resolve_env_path(target)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| FileBrokerError::Io {
                action: "create target directory",
                source,
            })?;
        }
        fs::write(&target_path, bytes).map_err(|source| FileBrokerError::Io {
            action: "write",
            source,
        })?;
        Ok(FileBrokerWriteResult {
            file_ref: target.raw().to_string(),
            byte_count: bytes.len() as u64,
        })
    }

    pub(crate) fn copy(
        &self,
        source: &FileRef,
        target: &FileRef,
    ) -> Result<FileBrokerCopyResult, FileBrokerError> {
        let bytes = self.read_to_bytes(source)?;
        let write_result = self.write_bytes(target, &bytes)?;
        Ok(FileBrokerCopyResult {
            source_ref: source.raw().to_string(),
            target_ref: write_result.file_ref,
            byte_count: write_result.byte_count,
        })
    }

    pub(crate) fn export_data_uri(
        &self,
        source: &FileRef,
        mime_type: &str,
    ) -> Result<FileBrokerDataUriResult, FileBrokerError> {
        let bytes = self.read_to_bytes(source)?;
        if bytes.len() > MAX_DATA_URI_EXPORT_BYTES {
            return Err(FileBrokerError::DataUriTooLarge {
                file_ref: source.raw().to_string(),
                byte_count: bytes.len() as u64,
                max_byte_count: MAX_DATA_URI_EXPORT_BYTES as u64,
            });
        }

        let data_uri = format!(
            "data:{mime_type};base64,{}",
            BASE64_STANDARD.encode(bytes.as_slice())
        );
        Ok(FileBrokerDataUriResult {
            source_ref: source.raw().to_string(),
            mime_type: mime_type.to_string(),
            data_uri,
            byte_count: bytes.len() as u64,
        })
    }

    pub(crate) fn active_provider_registry(mcp_tools: &[ToolInfo]) -> ActiveProviderRegistry {
        ActiveProviderRegistry::from_routes(CONFIGURED_BROKER_ROUTES, mcp_tools)
    }

    fn resolve_env_path(&self, file_ref: &FileRef) -> Result<PathBuf, FileBrokerError> {
        if file_ref.scheme() != FileScheme::Env {
            return Err(FileBrokerError::UnsupportedProvider {
                file_ref: file_ref.raw().to_string(),
            });
        }

        let Some(path) = file_ref.body().strip_prefix("current/") else {
            return Err(FileBrokerError::UnsupportedEnvironment {
                file_ref: file_ref.raw().to_string(),
            });
        };
        if path.is_empty() {
            return Err(FileBrokerError::InvalidEnvPath {
                file_ref: file_ref.raw().to_string(),
            });
        }

        let relative_path =
            clean_relative_path(path).ok_or_else(|| FileBrokerError::InvalidEnvPath {
                file_ref: file_ref.raw().to_string(),
            })?;
        Ok(self.current_root.join(relative_path))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ConfiguredBrokerRoute {
    provider: &'static str,
    operation: &'static str,
    required_server: &'static str,
    required_namespace: &'static str,
    required_tool: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ActiveProviderRegistry {
    pub(crate) providers: Vec<ActiveBrokerProvider>,
    pub(crate) unavailable_routes: Vec<UnavailableBrokerRoute>,
}

impl ActiveProviderRegistry {
    fn from_routes(routes: &[ConfiguredBrokerRoute], mcp_tools: &[ToolInfo]) -> Self {
        let mut active_operations = BTreeMap::<String, Vec<String>>::new();
        let mut unavailable_routes = Vec::new();

        for route in routes {
            if route_is_available(route, mcp_tools) {
                active_operations
                    .entry(route.provider.to_string())
                    .or_default()
                    .push(route.operation.to_string());
            } else {
                unavailable_routes.push(UnavailableBrokerRoute {
                    provider: route.provider.to_string(),
                    operation: route.operation.to_string(),
                    required_server: route.required_server.to_string(),
                    required_namespace: route.required_namespace.to_string(),
                    required_tool: route.required_tool.to_string(),
                });
            }
        }

        let providers = active_operations
            .into_iter()
            .map(|(provider, mut operations)| {
                operations.sort();
                ActiveBrokerProvider {
                    provider,
                    operations,
                }
            })
            .collect();

        Self {
            providers,
            unavailable_routes,
        }
    }

    pub(crate) fn summary(&self) -> String {
        if self.providers.is_empty() {
            return "none".to_string();
        }

        self.providers
            .iter()
            .map(|provider| format!("{}: {}", provider.provider, provider.operations.join(", ")))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ActiveBrokerProvider {
    pub(crate) provider: String,
    pub(crate) operations: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct UnavailableBrokerRoute {
    pub(crate) provider: String,
    pub(crate) operation: String,
    pub(crate) required_server: String,
    pub(crate) required_namespace: String,
    pub(crate) required_tool: String,
}

fn route_is_available(route: &ConfiguredBrokerRoute, mcp_tools: &[ToolInfo]) -> bool {
    mcp_tools.iter().any(|tool| {
        tool.server_name == route.required_server
            && tool.callable_namespace == route.required_namespace
            && (tool.callable_name == route.required_tool
                || tool.tool.name.as_ref() == route.required_tool)
    })
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FileBrokerWriteResult {
    pub(crate) file_ref: String,
    pub(crate) byte_count: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FileBrokerCopyResult {
    pub(crate) source_ref: String,
    pub(crate) target_ref: String,
    pub(crate) byte_count: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FileBrokerDataUriResult {
    pub(crate) source_ref: String,
    pub(crate) mime_type: String,
    pub(crate) data_uri: String,
    pub(crate) byte_count: u64,
}

#[derive(Debug)]
pub(crate) enum FileBrokerError {
    UnsupportedProvider {
        file_ref: String,
    },
    UnsupportedEnvironment {
        file_ref: String,
    },
    InvalidEnvPath {
        file_ref: String,
    },
    Io {
        action: &'static str,
        source: std::io::Error,
    },
    DataUriTooLarge {
        file_ref: String,
        byte_count: u64,
        max_byte_count: u64,
    },
}

impl fmt::Display for FileBrokerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProvider { file_ref } => {
                write!(f, "file provider for `{file_ref}` is not available")
            }
            Self::UnsupportedEnvironment { file_ref } => {
                write!(f, "`{file_ref}` must use env://current/... in this runtime")
            }
            Self::InvalidEnvPath { file_ref } => {
                write!(f, "`{file_ref}` must resolve to a relative workspace path")
            }
            Self::Io { action, source } => write!(f, "failed to {action} file: {source}"),
            Self::DataUriTooLarge {
                file_ref,
                byte_count,
                max_byte_count,
            } => write!(
                f,
                "`{file_ref}` is {byte_count} bytes; max data URI export is {max_byte_count} bytes"
            ),
        }
    }
}

impl std::error::Error for FileBrokerError {}

impl FileBrokerError {
    pub(crate) fn should_include_active_provider_status(&self) -> bool {
        matches!(
            self,
            Self::UnsupportedProvider { .. } | Self::UnsupportedEnvironment { .. }
        )
    }
}

fn clean_relative_path(path: &str) -> Option<PathBuf> {
    let mut clean = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!clean.as_os_str().is_empty()).then_some(clean)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rmcp::model::Tool;
    use serde_json::Map;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn file_ref(raw: &str) -> FileRef {
        FileRef::parse(raw).expect("file ref should parse")
    }

    fn mcp_tool(server_name: &str, namespace: &str, name: &str) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            supports_parallel_tool_calls: false,
            server_origin: None,
            callable_name: name.to_string(),
            callable_namespace: namespace.to_string(),
            namespace_description: None,
            tool: Tool {
                name: name.to_string().into(),
                title: None,
                description: None,
                input_schema: Arc::new(Map::new()),
                output_schema: None,
                annotations: None,
                execution: None,
                icons: None,
                meta: None,
            },
            connector_id: None,
            connector_name: None,
            plugin_display_names: Vec::new(),
        }
    }

    #[test]
    fn writes_and_reads_env_current_refs() {
        let temp_dir = TempDir::new().expect("temp dir");
        let broker = CodeModeFileBroker::new(temp_dir.path());

        let target = file_ref("env://current/out/report.txt");
        let write_result = broker
            .write_bytes(&target, b"hello")
            .expect("write should succeed");

        assert_eq!(
            write_result,
            FileBrokerWriteResult {
                file_ref: "env://current/out/report.txt".to_string(),
                byte_count: 5,
            }
        );
        assert_eq!(
            broker.read_to_bytes(&target).expect("read should succeed"),
            b"hello"
        );
    }

    #[test]
    fn copies_between_env_current_refs() {
        let temp_dir = TempDir::new().expect("temp dir");
        let broker = CodeModeFileBroker::new(temp_dir.path());
        let source = file_ref("env://current/source.bin");
        let target = file_ref("env://current/nested/target.bin");
        broker
            .write_bytes(&source, b"payload")
            .expect("write should succeed");

        let copy_result = broker.copy(&source, &target).expect("copy should succeed");

        assert_eq!(
            copy_result,
            FileBrokerCopyResult {
                source_ref: "env://current/source.bin".to_string(),
                target_ref: "env://current/nested/target.bin".to_string(),
                byte_count: 7,
            }
        );
        assert_eq!(
            broker
                .read_to_bytes(&target)
                .expect("copied target should exist"),
            b"payload"
        );
    }

    #[test]
    fn rejects_env_path_traversal() {
        let temp_dir = TempDir::new().expect("temp dir");
        let broker = CodeModeFileBroker::new(temp_dir.path());
        let source = file_ref("env://current/../secret.txt");

        assert!(matches!(
            broker.read_to_bytes(&source),
            Err(FileBrokerError::InvalidEnvPath { .. })
        ));
    }

    #[test]
    fn rejects_provider_refs_without_adapter() {
        let temp_dir = TempDir::new().expect("temp dir");
        let broker = CodeModeFileBroker::new(temp_dir.path());
        let source = file_ref("oai_library://file_123");

        assert!(matches!(
            broker.read_to_bytes(&source),
            Err(FileBrokerError::UnsupportedProvider { .. })
        ));
    }

    #[test]
    fn exports_env_current_ref_as_data_uri() {
        let temp_dir = TempDir::new().expect("temp dir");
        let broker = CodeModeFileBroker::new(temp_dir.path());
        let source = file_ref("env://current/image.png");
        broker
            .write_bytes(&source, b"png")
            .expect("write should succeed");

        let result = broker
            .export_data_uri(&source, "image/png")
            .expect("export should succeed");

        assert_eq!(
            result,
            FileBrokerDataUriResult {
                source_ref: "env://current/image.png".to_string(),
                mime_type: "image/png".to_string(),
                data_uri: "data:image/png;base64,cG5n".to_string(),
                byte_count: 3,
            }
        );
    }

    #[test]
    fn active_provider_registry_uses_live_mcp_inventory() {
        let registry = CodeModeFileBroker::active_provider_registry(&[mcp_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "google_drive",
            "upload_file",
        )]);

        assert_eq!(
            registry.providers,
            vec![ActiveBrokerProvider {
                provider: "google_drive".to_string(),
                operations: vec!["upload".to_string()],
            }]
        );
        assert!(registry.unavailable_routes.is_empty());
        assert_eq!(registry.summary(), "google_drive: upload");
    }

    #[test]
    fn active_provider_registry_reports_missing_route_dependency() {
        let registry = CodeModeFileBroker::active_provider_registry(&[]);

        assert!(registry.providers.is_empty());
        assert_eq!(
            registry.unavailable_routes,
            vec![UnavailableBrokerRoute {
                provider: "google_drive".to_string(),
                operation: "upload".to_string(),
                required_server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                required_namespace: "google_drive".to_string(),
                required_tool: "upload_file".to_string(),
            }]
        );
        assert_eq!(registry.summary(), "none");
    }
}
