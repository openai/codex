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

/// Runtime placement information used when starting MCP server transports.
///
/// `McpConfig` describes what servers exist. This value describes which
/// selected/default environment MCP servers should use for the current caller.
/// Keep it explicit at manager construction time so status/snapshot paths and
/// real sessions make the same placement decision. `fallback_cwd` is not a
/// per-server override; it is used when a stdio server omits `cwd` and the
/// launcher needs a concrete process working directory. `local_environment`
/// is separate because a remote selected/default environment can coexist with
/// an explicitly configured local environment that may launch local stdio MCPs.
#[derive(Clone)]
pub struct McpRuntimeEnvironment {
    environment: Option<Arc<Environment>>,
    local_environment: Option<Arc<Environment>>,
    fallback_cwd: PathBuf,
}

impl McpRuntimeEnvironment {
    pub fn new(
        environment: Option<Arc<Environment>>,
        local_environment: Option<Arc<Environment>>,
        fallback_cwd: PathBuf,
    ) -> Self {
        Self {
            environment,
            local_environment,
            fallback_cwd,
        }
    }

    pub(crate) fn environment(&self) -> Option<Arc<Environment>> {
        self.environment.as_ref().map(Arc::clone)
    }

    pub(crate) fn fallback_cwd(&self) -> PathBuf {
        self.fallback_cwd.clone()
    }

    pub(crate) fn startup_unavailable_reason(
        &self,
        server_name: &str,
        config: &codex_config::McpServerConfig,
    ) -> Option<String> {
        // This is intentionally narrower than "no env means no MCP": local
        // stdio needs a local process launcher, while local HTTP can still use
        // the ambient HTTP client with no local environment configured.
        match config.experimental_environment.as_deref() {
            None | Some("local") => {
                // Local stdio only needs an explicitly configured local
                // launcher. The selected/default MCP environment can be remote
                // when both local and remote environments are configured.
                if self.local_environment.is_none()
                    && matches!(
                        config.transport,
                        codex_config::McpServerTransportConfig::Stdio { .. }
                    )
                {
                    Some(format!(
                        "local stdio MCP server `{server_name}` requires a local environment"
                    ))
                } else {
                    None
                }
            }
            Some("remote") => match self.environment.as_ref() {
                Some(environment) if environment.is_remote() => None,
                _ => Some(format!(
                    "remote MCP server `{server_name}` requires a remote environment"
                )),
            },
            Some(environment) => Some(format!(
                "unsupported experimental_environment `{environment}` for MCP server `{server_name}`"
            )),
        }
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
    use pretty_assertions::assert_eq;

    use super::*;

    fn stdio_server(experimental_environment: Option<&str>) -> McpServerConfig {
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "echo".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            experimental_environment: experimental_environment.map(str::to_string),
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

    fn http_server(experimental_environment: Option<&str>) -> McpServerConfig {
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "http://127.0.0.1:1".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            experimental_environment: experimental_environment.map(str::to_string),
            ..stdio_server(/*experimental_environment*/ None)
        }
    }

    #[test]
    fn local_stdio_requires_local_stdio_availability() {
        let runtime_environment = McpRuntimeEnvironment::new(
            /*environment*/ None,
            /*local_environment*/ None,
            PathBuf::from("/tmp"),
        );

        assert_eq!(
            runtime_environment.startup_unavailable_reason(
                "stdio",
                &stdio_server(/*experimental_environment*/ None)
            ),
            Some("local stdio MCP server `stdio` requires a local environment".to_string())
        );
    }

    #[test]
    fn local_http_does_not_require_local_stdio_availability() {
        let runtime_environment = McpRuntimeEnvironment::new(
            /*environment*/ None,
            /*local_environment*/ None,
            PathBuf::from("/tmp"),
        );

        assert_eq!(
            runtime_environment.startup_unavailable_reason(
                "http",
                &http_server(/*experimental_environment*/ None)
            ),
            None
        );
    }

    #[test]
    fn remote_stdio_requires_remote_environment() {
        let runtime_environment = McpRuntimeEnvironment::new(
            /*environment*/ None,
            /*local_environment*/ None,
            PathBuf::from("/tmp"),
        );

        assert_eq!(
            runtime_environment.startup_unavailable_reason(
                "stdio",
                &stdio_server(/*experimental_environment*/ Some("remote")),
            ),
            Some("remote MCP server `stdio` requires a remote environment".to_string())
        );
    }

    #[test]
    fn remote_stdio_and_http_accept_remote_environment() {
        let environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("remote environment"),
        );
        let runtime_environment = McpRuntimeEnvironment::new(
            Some(environment),
            /*local_environment*/ None,
            PathBuf::from("/tmp"),
        );

        assert_eq!(
            runtime_environment.startup_unavailable_reason(
                "stdio",
                &stdio_server(/*experimental_environment*/ Some("remote")),
            ),
            None
        );
        assert_eq!(
            runtime_environment.startup_unavailable_reason(
                "http",
                &http_server(/*experimental_environment*/ Some("remote")),
            ),
            None
        );
    }

    #[tokio::test]
    async fn local_stdio_accepts_remote_runtime_when_local_environment_exists() {
        let remote_environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                .expect("remote environment"),
        );
        let local_environment = Arc::new(
            Environment::create_for_tests(/*exec_server_url*/ None).expect("local environment"),
        );
        let runtime_environment = McpRuntimeEnvironment::new(
            Some(remote_environment),
            Some(local_environment),
            PathBuf::from("/tmp"),
        );

        assert_eq!(
            runtime_environment.startup_unavailable_reason(
                "stdio",
                &stdio_server(/*experimental_environment*/ None),
            ),
            None
        );
    }
}
