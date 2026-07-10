//! Runtime support for Model Context Protocol (MCP) servers.
//!
//! This module contains data that describes the runtime environment in which MCP
//! servers execute, plus the sandbox state payload sent to capable servers and a
//! tiny shared metrics helper. Transport startup and orchestration live in
//! [`crate::rmcp_client`] and [`crate::connection_manager`].

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_exec_server::Environment;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::HttpClient;
use codex_exec_server::ReqwestHttpClient;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de::Error as _;

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxState {
    pub permission_profile: PermissionProfile<PathUri>,
    pub codex_linux_sandbox_exe: Option<PathUri>,
    pub sandbox_cwd: PathUri,
    #[serde(default)]
    pub use_legacy_landlock: bool,
}

/// Historical mixed native-path/URI payload for `codex/sandbox-state-meta`.
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacySandboxState {
    pub permission_profile: PermissionProfile<LegacyAppPathString>,
    pub codex_linux_sandbox_exe: Option<LegacyAppPathString>,
    pub sandbox_cwd: PathUri,
    pub use_legacy_landlock: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySandboxStateWire {
    permission_profile: PermissionProfile<LegacyAppPathString>,
    codex_linux_sandbox_exe: Option<LegacyAppPathString>,
    sandbox_cwd: PathUri,
    #[serde(default)]
    use_legacy_landlock: bool,
}

impl LegacySandboxState {
    pub fn from_native_paths(
        permission_profile: PermissionProfile,
        codex_linux_sandbox_exe: Option<&Path>,
        sandbox_cwd: PathUri,
        use_legacy_landlock: bool,
    ) -> anyhow::Result<Self> {
        let permission_profile = try_map_permission_profile_paths(permission_profile, |path| {
            let path = path.as_path().to_str().ok_or_else(|| {
                anyhow::anyhow!("legacy sandbox permission path is not valid UTF-8")
            })?;
            Ok::<_, anyhow::Error>(LegacyAppPathString::from_path(Path::new(path)))
        })?;
        let codex_linux_sandbox_exe = codex_linux_sandbox_exe
            .map(|path| {
                let path = path.to_str().ok_or_else(|| {
                    anyhow::anyhow!("legacy sandbox helper path is not valid UTF-8")
                })?;
                Ok::<_, anyhow::Error>(LegacyAppPathString::from_path(Path::new(path)))
            })
            .transpose()?;
        Ok(Self {
            permission_profile,
            codex_linux_sandbox_exe,
            sandbox_cwd,
            use_legacy_landlock,
        })
    }
}

impl<'de> Deserialize<'de> for SandboxState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = LegacySandboxStateWire::deserialize(deserializer)?;
        let permission_profile =
            try_map_permission_profile_paths(wire.permission_profile, sandbox_wire_path_to_uri)
                .map_err(D::Error::custom)?;
        let codex_linux_sandbox_exe = wire
            .codex_linux_sandbox_exe
            .map(|path| sandbox_wire_helper_path_to_uri(path, &wire.sandbox_cwd))
            .transpose()
            .map_err(D::Error::custom)?;
        Ok(Self {
            permission_profile,
            codex_linux_sandbox_exe,
            sandbox_cwd: wire.sandbox_cwd,
            use_legacy_landlock: wire.use_legacy_landlock,
        })
    }
}

fn try_map_permission_profile_paths<InputPath, OutputPath, E>(
    profile: PermissionProfile<InputPath>,
    mut map: impl FnMut(InputPath) -> Result<OutputPath, E>,
) -> Result<PermissionProfile<OutputPath>, E> {
    Ok(match profile {
        PermissionProfile::Managed {
            file_system,
            network,
        } => {
            let file_system = match file_system {
                ManagedFileSystemPermissions::Restricted {
                    entries,
                    glob_scan_max_depth,
                } => ManagedFileSystemPermissions::Restricted {
                    entries: entries
                        .into_iter()
                        .map(|entry| {
                            let path = match entry.path {
                                FileSystemPath::Path { path } => {
                                    FileSystemPath::Path { path: map(path)? }
                                }
                                FileSystemPath::GlobPattern { pattern } => {
                                    FileSystemPath::GlobPattern { pattern }
                                }
                                FileSystemPath::Special { value } => {
                                    FileSystemPath::Special { value }
                                }
                            };
                            Ok(FileSystemSandboxEntry {
                                path,
                                access: entry.access,
                            })
                        })
                        .collect::<Result<Vec<_>, E>>()?,
                    glob_scan_max_depth,
                },
                ManagedFileSystemPermissions::Unrestricted => {
                    ManagedFileSystemPermissions::Unrestricted
                }
            };
            PermissionProfile::Managed {
                file_system,
                network,
            }
        }
        PermissionProfile::Disabled => PermissionProfile::Disabled,
        PermissionProfile::External { network } => PermissionProfile::External { network },
    })
}

fn sandbox_wire_path_to_uri(path: LegacyAppPathString) -> Result<PathUri, String> {
    if let Ok(uri) = PathUri::parse(path.as_str()) {
        return Ok(uri);
    }
    path.to_inferred_path_uri().ok_or_else(|| {
        format!("sandbox path `{path}` is neither a path URI nor an absolute native path")
    })
}

fn sandbox_wire_helper_path_to_uri(
    path: LegacyAppPathString,
    sandbox_cwd: &PathUri,
) -> Result<PathUri, String> {
    if let Ok(uri) = PathUri::parse(path.as_str()) {
        return Ok(uri);
    }
    if let Some(uri) = path.to_inferred_path_uri() {
        return Ok(uri);
    }
    sandbox_cwd.join(path.as_str()).map_err(|err| {
        format!(
            "sandbox helper path `{path}` cannot be resolved against sandbox cwd URI `{sandbox_cwd}`: {err}"
        )
    })
}

/// Runtime context used when resolving per-server MCP environments.
///
/// `McpConfig` describes what servers exist. This value carries the canonical
/// environment registry plus the local stdio fallback cwd used when a local
/// stdio server omits its own working directory.
#[derive(Clone)]
pub struct McpRuntimeContext {
    environment_manager: Arc<EnvironmentManager>,
    local_stdio_fallback_cwd: PathBuf,
}

impl McpRuntimeContext {
    pub fn new(
        environment_manager: Arc<EnvironmentManager>,
        local_stdio_fallback_cwd: PathBuf,
    ) -> Self {
        Self {
            environment_manager,
            local_stdio_fallback_cwd,
        }
    }

    pub(crate) fn local_stdio_fallback_cwd(&self) -> PathBuf {
        self.local_stdio_fallback_cwd.clone()
    }

    pub(crate) fn resolve_server_environment(
        &self,
        server_name: &str,
        config: &codex_config::McpServerConfig,
    ) -> Result<Option<Arc<Environment>>, String> {
        // Resolve `"local"` through the shared registry when available. Local
        // HTTP is the one current exception: it can use the ambient HTTP client
        // even when no local Environment is configured.
        if let Some(environment) = self
            .environment_manager
            .get_environment(&config.environment_id)
        {
            return Ok(Some(environment));
        }

        if config.is_local_environment() {
            return match config.transport {
                codex_config::McpServerTransportConfig::Stdio { .. } => Err(format!(
                    "local stdio MCP server `{server_name}` requires a local environment"
                )),
                codex_config::McpServerTransportConfig::StreamableHttp { .. } => Ok(None),
            };
        }

        Err(format!(
            "MCP server `{server_name}` references unknown environment id `{}`",
            config.environment_id
        ))
    }

    /// Resolves the HTTP capability owned by the server's configured environment.
    pub fn resolve_http_client(
        &self,
        server_name: &str,
        config: &codex_config::McpServerConfig,
    ) -> Result<Arc<dyn HttpClient>, String> {
        Ok(self
            .resolve_server_environment(server_name, config)?
            .map_or_else(
                || Arc::new(ReqwestHttpClient) as Arc<dyn HttpClient>,
                |environment| environment.get_http_client(),
            ))
    }
}

pub(crate) fn emit_duration(metric: &str, duration: Duration, tags: &[(&str, &str)]) {
    if let Some(metrics) = codex_otel::global() {
        let _ = metrics.record_duration(metric, duration, tags);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
    use codex_config::McpServerConfig;
    use codex_config::McpServerTransportConfig;
    use codex_exec_server::EnvironmentManager;
    use codex_utils_path_uri::LegacyAppPathString;
    use pretty_assertions::assert_eq;

    use super::*;

    fn stdio_server(environment_id: &str) -> McpServerConfig {
        McpServerConfig {
            auth: Default::default(),
            transport: McpServerTransportConfig::Stdio {
                command: "echo".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            environment_id: environment_id.to_string(),
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
        }
    }

    fn http_server(environment_id: &str) -> McpServerConfig {
        McpServerConfig {
            auth: Default::default(),
            transport: McpServerTransportConfig::StreamableHttp {
                url: "http://127.0.0.1:1".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            environment_id: environment_id.to_string(),
            ..stdio_server(environment_id)
        }
    }

    #[test]
    fn local_stdio_requires_local_stdio_availability() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::without_environments()),
            PathBuf::from("/tmp"),
        );

        let error = match runtime_context
            .resolve_server_environment("stdio", &stdio_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID))
        {
            Ok(_) => panic!("local stdio MCP should require a local environment"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            "local stdio MCP server `stdio` requires a local environment"
        );
    }

    #[test]
    fn local_http_does_not_require_local_stdio_availability() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::without_environments()),
            PathBuf::from("/tmp"),
        );

        let resolved_runtime = match runtime_context
            .resolve_server_environment("http", &http_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID))
        {
            Ok(resolved_runtime) => resolved_runtime,
            Err(error) => panic!("local HTTP MCP should resolve: {error}"),
        };
        assert!(resolved_runtime.is_none());
    }

    #[test]
    fn unknown_explicit_environment_is_rejected() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::without_environments()),
            PathBuf::from("/tmp"),
        );

        let error =
            match runtime_context.resolve_server_environment("stdio", &stdio_server("remote")) {
                Ok(_) => panic!("unknown MCP environment should fail"),
                Err(error) => error,
            };
        assert_eq!(
            error,
            "MCP server `stdio` references unknown environment id `remote`"
        );
    }

    #[tokio::test]
    async fn explicit_remote_stdio_and_http_accept_named_environment() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(
                EnvironmentManager::create_for_tests(
                    Some("ws://127.0.0.1:8765".to_string()),
                    /*local_runtime_paths*/ None,
                )
                .await,
            ),
            PathBuf::from("/tmp"),
        );

        let mut remote_stdio = stdio_server("remote");
        let McpServerTransportConfig::Stdio { cwd, .. } = &mut remote_stdio.transport else {
            unreachable!("stdio helper should build stdio transport");
        };
        *cwd = Some(LegacyAppPathString::from_path(&std::env::temp_dir()));
        for resolved_runtime in [
            runtime_context.resolve_server_environment("stdio", &remote_stdio),
            runtime_context.resolve_server_environment("http", &http_server("remote")),
        ] {
            let resolved_runtime = match resolved_runtime {
                Ok(resolved_runtime) => resolved_runtime,
                Err(error) => panic!("remote MCP should resolve: {error}"),
            };
            assert!(resolved_runtime.is_some());
        }
    }

    #[tokio::test]
    async fn remote_stdio_accepts_foreign_absolute_cwd() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(
                EnvironmentManager::create_for_tests(
                    Some("ws://127.0.0.1:8765".to_string()),
                    /*local_runtime_paths*/ None,
                )
                .await,
            ),
            PathBuf::from("/tmp"),
        );
        let mut remote_stdio = stdio_server("remote");
        let McpServerTransportConfig::Stdio { cwd, .. } = &mut remote_stdio.transport else {
            unreachable!("stdio helper should build stdio transport");
        };
        *cwd = Some(
            PathUri::parse("file:///C:/plugins/demo")
                .expect("foreign cwd URI")
                .into(),
        );

        let resolved_runtime =
            match runtime_context.resolve_server_environment("stdio", &remote_stdio) {
                Ok(resolved_runtime) => resolved_runtime,
                Err(error) => panic!("foreign cwd should resolve: {error}"),
            };
        assert!(resolved_runtime.is_some());
    }

    #[tokio::test]
    async fn local_stdio_accepts_local_environment_when_available() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::default_for_tests()),
            PathBuf::from("/tmp"),
        );

        let resolved_runtime = match runtime_context
            .resolve_server_environment("stdio", &stdio_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID))
        {
            Ok(resolved_runtime) => resolved_runtime,
            Err(error) => panic!("local stdio MCP should resolve: {error}"),
        };
        assert!(resolved_runtime.is_some());
    }
}
