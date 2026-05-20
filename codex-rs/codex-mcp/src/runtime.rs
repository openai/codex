//! Runtime support for Model Context Protocol (MCP) servers.
//!
//! This module contains data that describes the runtime environment in which MCP
//! servers execute, plus the sandbox state payload sent to capable servers and a
//! tiny shared metrics helper. Transport startup and orchestration live in
//! [`crate::rmcp_client`] and [`crate::connection_manager`].

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_exec_server::Environment;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::HttpClient;
use codex_exec_server::ReqwestHttpClient;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::SandboxPolicy;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<PermissionProfile>,
    pub sandbox_policy: SandboxPolicy,
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub sandbox_cwd: PathBuf,
    #[serde(default)]
    pub use_legacy_landlock: bool,
}

/// Effective runtime placement for one MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServerRuntimePlacement {
    /// The orchestrator owns the transport directly.
    Orchestrator,
    /// The selected named environment owns the transport.
    Environment { environment_id: String },
}

impl McpServerRuntimePlacement {
    pub fn from_config(config: &codex_config::McpServerConfig) -> Self {
        if config.is_local_environment() {
            Self::Orchestrator
        } else {
            Self::Environment {
                environment_id: config.environment_id.clone(),
            }
        }
    }
}

/// Resolved runtime handle for one MCP server startup.
#[derive(Clone)]
pub(crate) enum ResolvedMcpServerRuntime {
    Orchestrator,
    Environment(Arc<Environment>),
}

impl ResolvedMcpServerRuntime {
    pub(crate) fn http_client(&self) -> Arc<dyn HttpClient> {
        match self {
            Self::Orchestrator => Arc::new(ReqwestHttpClient) as Arc<dyn HttpClient>,
            Self::Environment(environment) => environment.get_http_client(),
        }
    }
}

/// Runtime context used when resolving per-server MCP environment placement.
///
/// `McpConfig` describes what servers exist. This value carries the canonical
/// environment registry plus the local stdio fallback cwd used when a local
/// stdio server omits its own working directory.
#[derive(Clone)]
pub struct McpRuntimeContext {
    environment_manager: Arc<EnvironmentManager>,
    fallback_cwd: PathBuf,
}

impl McpRuntimeContext {
    pub fn new(environment_manager: Arc<EnvironmentManager>, fallback_cwd: PathBuf) -> Self {
        Self {
            environment_manager,
            fallback_cwd,
        }
    }

    pub(crate) fn fallback_cwd(&self) -> PathBuf {
        self.fallback_cwd.clone()
    }

    pub(crate) fn resolve_server_runtime(
        &self,
        server_name: &str,
        config: &codex_config::McpServerConfig,
    ) -> Result<ResolvedMcpServerRuntime, String> {
        match McpServerRuntimePlacement::from_config(config) {
            McpServerRuntimePlacement::Orchestrator => {
                if self.environment_manager.try_local_environment().is_none()
                    && matches!(
                        config.transport,
                        codex_config::McpServerTransportConfig::Stdio { .. }
                    )
                {
                    return Err(format!(
                        "local stdio MCP server `{server_name}` requires a local environment"
                    ));
                }
                Ok(ResolvedMcpServerRuntime::Orchestrator)
            }
            McpServerRuntimePlacement::Environment { environment_id } => {
                let environment = self
                    .environment_manager
                    .get_environment(&environment_id)
                    .ok_or_else(|| {
                        format!(
                            "MCP server `{server_name}` references unknown environment id `{environment_id}`"
                        )
                    })?;
                ensure_remote_stdio_cwd(server_name, config)?;
                Ok(ResolvedMcpServerRuntime::Environment(environment))
            }
        }
    }
}

fn ensure_remote_stdio_cwd(
    server_name: &str,
    config: &codex_config::McpServerConfig,
) -> Result<(), String> {
    let codex_config::McpServerTransportConfig::Stdio { cwd, .. } = &config.transport else {
        return Ok(());
    };
    let Some(cwd) = cwd else {
        return Err(format!(
            "remote stdio MCP server `{server_name}` requires an absolute cwd"
        ));
    };
    if cwd.is_absolute() {
        return Ok(());
    }
    Err(format!(
        "remote stdio MCP server `{server_name}` requires an absolute cwd, got `{}`",
        cwd.display()
    ))
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
    use pretty_assertions::assert_eq;

    use super::*;

    fn stdio_server(environment_id: &str) -> McpServerConfig {
        McpServerConfig {
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
            .resolve_server_runtime("stdio", &stdio_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID))
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

        let resolved_runtime = runtime_context
            .resolve_server_runtime("http", &http_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID))
            .expect("local HTTP MCP should resolve");
        assert!(matches!(
            resolved_runtime,
            ResolvedMcpServerRuntime::Orchestrator
        ));
    }

    #[test]
    fn unknown_explicit_environment_is_rejected() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::without_environments()),
            PathBuf::from("/tmp"),
        );

        let error = match runtime_context.resolve_server_runtime("stdio", &stdio_server("remote")) {
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
        *cwd = Some(std::env::temp_dir());
        for resolved_runtime in [
            runtime_context.resolve_server_runtime("stdio", &remote_stdio),
            runtime_context.resolve_server_runtime("http", &http_server("remote")),
        ] {
            let resolved_runtime = resolved_runtime.expect("remote MCP should resolve");
            assert!(matches!(
                resolved_runtime,
                ResolvedMcpServerRuntime::Environment(_)
            ));
        }
    }

    #[tokio::test]
    async fn local_stdio_accepts_local_environment_when_available() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::default_for_tests()),
            PathBuf::from("/tmp"),
        );

        let resolved_runtime = runtime_context
            .resolve_server_runtime("stdio", &stdio_server(DEFAULT_MCP_SERVER_ENVIRONMENT_ID))
            .expect("local stdio MCP should resolve");
        assert!(matches!(
            resolved_runtime,
            ResolvedMcpServerRuntime::Orchestrator
        ));
    }

    #[tokio::test]
    async fn remote_stdio_requires_absolute_cwd() {
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
        *cwd = Some(PathBuf::from("relative"));

        let error = match runtime_context.resolve_server_runtime("stdio", &remote_stdio) {
            Ok(_) => panic!("remote stdio MCP should require absolute cwd"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            "remote stdio MCP server `stdio` requires an absolute cwd, got `relative`"
        );
    }
}
