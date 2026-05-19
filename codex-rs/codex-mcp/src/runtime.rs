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

    pub(crate) fn startup_unavailable_reason(
        &self,
        server_name: &str,
        config: &codex_config::McpServerConfig,
    ) -> Option<String> {
        match config.experimental_environment.as_deref() {
            None | Some("local") => {
                if !self.local_environment_available
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
            Some("remote") => match self.environment() {
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
