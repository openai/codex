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
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
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

/// Resolved environment placement for one MCP server startup.
#[derive(Clone)]
pub struct ResolvedMcpEnvironment {
    pub environment_id: String,
    pub environment: Option<Arc<Environment>>,
}

/// Runtime context used when resolving per-server MCP environment placement.
///
/// `McpConfig` describes what servers exist. This value carries the canonical
/// environment registry plus the fallback cwd used when a stdio server omits
/// its own working directory.
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

    pub(crate) fn resolve_server_environment(
        &self,
        server_name: &str,
        config: &codex_config::McpServerConfig,
    ) -> Result<ResolvedMcpEnvironment, String> {
        // MCP config parsing materializes an omitted environment id as `local`,
        // so runtime resolution always starts from one explicit effective id.
        let environment_id = config.environment_id.clone();
        let environment = self.environment_manager.get_environment(&environment_id);
        if environment.is_none() {
            if environment_id == LOCAL_ENVIRONMENT_ID
                && matches!(
                    config.transport,
                    codex_config::McpServerTransportConfig::Stdio { .. }
                )
            {
                return Err(format!(
                    "local stdio MCP server `{server_name}` requires a local environment"
                ));
            }
            if environment_id != LOCAL_ENVIRONMENT_ID {
                return Err(format!(
                    "MCP server `{server_name}` references unknown environment id `{environment_id}`"
                ));
            }
        }
        Ok(ResolvedMcpEnvironment {
            environment_id,
            environment,
        })
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
            .resolve_server_environment("stdio", &stdio_server(LOCAL_ENVIRONMENT_ID))
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

        let resolved_environment = runtime_context
            .resolve_server_environment("http", &http_server(LOCAL_ENVIRONMENT_ID))
            .expect("local HTTP MCP should resolve");
        assert_eq!(resolved_environment.environment_id, LOCAL_ENVIRONMENT_ID);
        assert!(resolved_environment.environment.is_none());
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

        for resolved_environment in [
            runtime_context.resolve_server_environment("stdio", &stdio_server("remote")),
            runtime_context.resolve_server_environment("http", &http_server("remote")),
        ] {
            let resolved_environment = resolved_environment.expect("remote MCP should resolve");
            assert_eq!(resolved_environment.environment_id, "remote");
            assert!(resolved_environment.environment.is_some());
        }
    }

    #[tokio::test]
    async fn local_stdio_accepts_local_environment_when_available() {
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::default_for_tests()),
            PathBuf::from("/tmp"),
        );

        let resolved_environment = runtime_context
            .resolve_server_environment("stdio", &stdio_server(LOCAL_ENVIRONMENT_ID))
            .expect("local stdio MCP should resolve");
        assert_eq!(resolved_environment.environment_id, LOCAL_ENVIRONMENT_ID);
        assert!(resolved_environment.environment.is_some());
    }
}
