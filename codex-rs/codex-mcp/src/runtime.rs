//! Runtime support for Model Context Protocol (MCP) servers.
//!
//! This module contains data that describes the runtime environment in which MCP
//! servers execute, plus the sandbox state payload sent to capable servers and a
//! tiny shared metrics helper. Transport startup and orchestration live in
//! [`crate::rmcp_client`] and [`crate::connection_manager`].

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
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
/// `McpConfig` describes what servers exist. This value describes where those
/// servers should run for the current caller. Keep it explicit at manager
/// construction time so status/snapshot paths and real sessions make the same
/// local-vs-remote decision. `fallback_cwd` is not a per-server override; it is
/// used when a stdio server omits `cwd` and the launcher needs a concrete
/// process working directory.
#[derive(Clone)]
pub struct McpRuntimeEnvironment {
    environment: Option<Arc<Environment>>,
    local_environment_available: bool,
    fallback_cwd: PathBuf,
}

impl McpRuntimeEnvironment {
    pub fn new(
        environment: Option<Arc<Environment>>,
        local_environment_available: bool,
        fallback_cwd: PathBuf,
    ) -> Self {
        Self {
            environment,
            local_environment_available,
            fallback_cwd,
        }
    }

    pub(crate) fn environment(&self) -> Option<Arc<Environment>> {
        self.environment.as_ref().map(Arc::clone)
    }

    pub(crate) fn fallback_cwd(&self) -> PathBuf {
        self.fallback_cwd.clone()
    }

    pub(crate) fn unavailable_reason(&self, config: &McpServerConfig) -> Option<&'static str> {
        let requires_remote_environment =
            matches!(config.experimental_environment.as_deref(), Some("remote"));
        if requires_remote_environment && self.environment.is_none() {
            return Some("remote MCP server requires a configured runtime environment");
        }

        if !self.local_environment_available
            && matches!(
                config.experimental_environment.as_deref(),
                None | Some("local")
            )
            && matches!(config.transport, McpServerTransportConfig::Stdio { .. })
        {
            return Some("local stdio MCP server requires a local environment");
        }

        None
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

    use super::*;

    fn mcp_server_config(transport: McpServerTransportConfig) -> McpServerConfig {
        McpServerConfig {
            transport,
            experimental_environment: None,
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

    #[test]
    fn no_environment_skips_local_stdio_but_keeps_local_http() {
        let runtime_environment = McpRuntimeEnvironment::new(None, false, PathBuf::from("/tmp"));
        let local_stdio = mcp_server_config(McpServerTransportConfig::Stdio {
            command: "echo".to_string(),
            args: Vec::new(),
            env: None,
            env_vars: Vec::new(),
            cwd: None,
        });
        let local_http = mcp_server_config(McpServerTransportConfig::StreamableHttp {
            url: "http://127.0.0.1:1234".to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        });

        assert_eq!(
            runtime_environment.unavailable_reason(&local_stdio),
            Some("local stdio MCP server requires a local environment")
        );
        assert_eq!(runtime_environment.unavailable_reason(&local_http), None);
    }

    #[test]
    fn no_environment_skips_remote_mcp_server() {
        let runtime_environment = McpRuntimeEnvironment::new(None, false, PathBuf::from("/tmp"));
        let mut remote_http = mcp_server_config(McpServerTransportConfig::StreamableHttp {
            url: "http://127.0.0.1:1234".to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        });
        remote_http.experimental_environment = Some("remote".to_string());

        assert_eq!(
            runtime_environment.unavailable_reason(&remote_http),
            Some("remote MCP server requires a configured runtime environment")
        );
    }
}
