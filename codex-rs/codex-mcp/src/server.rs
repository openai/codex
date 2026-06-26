use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use codex_config::McpServerConfig;
use codex_config::McpServerToolConfig;
use codex_config::McpServerTransportConfig;
use codex_config::McpToolApproval;
use codex_config::types::ApprovalsReviewer;
use thiserror::Error;

pub use crate::runtime_metadata::McpElicitationRuntimeMetadata;
pub use crate::runtime_metadata::McpSandboxStateSource;
pub use crate::runtime_metadata::McpServerRuntimeMetadata;
pub use crate::runtime_metadata::McpToolApprovalIdentity;
pub use crate::runtime_metadata::McpToolApprovalParameterLabel;
pub use crate::runtime_metadata::McpToolApprovalPersistence;
pub use crate::runtime_metadata::McpToolApprovalPresentation;
pub use crate::runtime_metadata::McpToolRuntimeMetadata;
pub use crate::runtime_metadata::McpToolTelemetryIdentity;

/// MCP server after runtime additions have been applied.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveMcpServer {
    config: Box<McpServerConfig>,
    runtime_bearer_token: Option<RuntimeBearerToken>,
    runtime_owner: Option<RuntimeOwnerGuard>,
    runtime_metadata: McpServerRuntimeMetadata,
}

#[derive(Clone, PartialEq)]
struct RuntimeBearerToken(String);

#[derive(Clone)]
pub(crate) struct RuntimeOwnerGuard(Arc<dyn Any + Send + Sync>);

impl fmt::Debug for RuntimeOwnerGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED RUNTIME OWNER]")
    }
}

impl PartialEq for RuntimeOwnerGuard {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeBearerTokenError {
    #[error("runtime bearer tokens require a streamable HTTP MCP server")]
    UnsupportedTransport,
    #[error("runtime bearer token must not be empty")]
    EmptyToken,
    #[error("runtime bearer token conflicts with configured HTTP authorization")]
    ConflictingAuthorization,
}

impl fmt::Debug for RuntimeBearerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl EffectiveMcpServer {
    pub fn configured(config: McpServerConfig) -> Self {
        Self {
            config: Box::new(config),
            runtime_bearer_token: None,
            runtime_owner: None,
            runtime_metadata: McpServerRuntimeMetadata::default(),
        }
    }

    /// Creates an HTTP MCP server with a process-owned bearer token that is
    /// intentionally absent from the serializable server configuration.
    pub fn configured_with_runtime_bearer_token(
        config: McpServerConfig,
        bearer_token: String,
    ) -> Result<Self, RuntimeBearerTokenError> {
        let McpServerTransportConfig::StreamableHttp {
            bearer_token_env_var,
            http_headers,
            env_http_headers,
            ..
        } = &config.transport
        else {
            return Err(RuntimeBearerTokenError::UnsupportedTransport);
        };
        if bearer_token.trim().is_empty() {
            return Err(RuntimeBearerTokenError::EmptyToken);
        }
        let has_authorization_header = |headers: &Option<HashMap<String, String>>| {
            headers.as_ref().is_some_and(|headers| {
                headers
                    .keys()
                    .any(|name| name.eq_ignore_ascii_case("authorization"))
            })
        };
        if bearer_token_env_var.is_some()
            || has_authorization_header(http_headers)
            || has_authorization_header(env_http_headers)
        {
            return Err(RuntimeBearerTokenError::ConflictingAuthorization);
        }
        Ok(Self {
            config: Box::new(config),
            runtime_bearer_token: Some(RuntimeBearerToken(bearer_token)),
            runtime_owner: None,
            runtime_metadata: McpServerRuntimeMetadata::default(),
        })
    }

    /// Retains a process-owned value for as long as this effective registration is alive.
    ///
    /// The value is type-erased, redacted from debug output, and absent from serializable config.
    pub fn with_runtime_owner<T>(mut self, owner: Arc<T>) -> Self
    where
        T: Any + Send + Sync,
    {
        self.runtime_owner = Some(RuntimeOwnerGuard(owner));
        self
    }

    /// Attaches host-provided metadata that must not enter serializable MCP config.
    pub fn with_runtime_metadata(mut self, runtime_metadata: McpServerRuntimeMetadata) -> Self {
        self.runtime_metadata = runtime_metadata;
        self
    }

    pub fn runtime_metadata(&self) -> &McpServerRuntimeMetadata {
        &self.runtime_metadata
    }

    pub(crate) fn runtime_bearer_token(&self) -> Option<&str> {
        self.runtime_bearer_token
            .as_ref()
            .map(|token| token.0.as_str())
    }

    pub(crate) fn has_same_launch_config(&self, other: &Self) -> bool {
        self.config == other.config && self.runtime_bearer_token == other.runtime_bearer_token
    }

    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    /// Applies the tool visibility and approval policy selected by the registration owner.
    pub fn with_tool_policy(
        mut self,
        enabled_tools: Vec<String>,
        tools: HashMap<String, McpServerToolConfig>,
    ) -> Self {
        self.config.enabled_tools = Some(enabled_tools);
        self.config.tools = tools;
        self
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    pub fn required(&self) -> bool {
        self.config.required
    }
}

/// Transport origin retained for metrics and diagnostics after server launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum McpServerOrigin {
    Stdio,
    StreamableHttp(String),
}

impl McpServerOrigin {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Stdio => "stdio",
            Self::StreamableHttp(origin) => origin,
        }
    }

    fn from_transport(transport: &McpServerTransportConfig) -> Option<Self> {
        match transport {
            McpServerTransportConfig::StreamableHttp { url, .. } => {
                let parsed = url::Url::parse(url).ok()?;
                Some(Self::StreamableHttp(parsed.origin().ascii_serialization()))
            }
            McpServerTransportConfig::Stdio { .. } => Some(Self::Stdio),
        }
    }
}

/// Semantic metadata that must survive after the server is launched.
#[derive(Debug, Clone)]
pub(crate) struct McpServerMetadata {
    pub environment_id: String,
    pub pollutes_memory: bool,
    pub origin: Option<McpServerOrigin>,
    pub supports_parallel_tool_calls: bool,
    pub default_tools_approval_mode: Option<McpToolApproval>,
    pub tool_approval_modes: HashMap<String, McpToolApproval>,
    pub tool_runtime_metadata: HashMap<String, McpToolRuntimeMetadata>,
    pub trusts_tool_input: bool,
    pub trusts_approval_context: bool,
    pub sandbox_state_source: McpSandboxStateSource,
    pub approvals_reviewer: Option<ApprovalsReviewer>,
    pub(crate) _runtime_owner: Option<RuntimeOwnerGuard>,
}

impl McpServerMetadata {
    pub fn tool_approval_mode(&self, tool_name: &str) -> McpToolApproval {
        self.tool_approval_modes
            .get(tool_name)
            .copied()
            .or(self.default_tools_approval_mode)
            .unwrap_or_default()
    }
}

impl From<&EffectiveMcpServer> for McpServerMetadata {
    fn from(server: &EffectiveMcpServer) -> Self {
        let config = server.config();
        Self {
            environment_id: config.environment_id.clone(),
            pollutes_memory: true,
            origin: server
                .runtime_metadata
                .telemetry_origin
                .clone()
                .map(McpServerOrigin::StreamableHttp)
                .or_else(|| McpServerOrigin::from_transport(&config.transport)),
            supports_parallel_tool_calls: config.supports_parallel_tool_calls,
            default_tools_approval_mode: config.default_tools_approval_mode,
            tool_approval_modes: config
                .tools
                .iter()
                .filter_map(|(name, config)| {
                    config
                        .approval_mode
                        .map(|approval_mode| (name.clone(), approval_mode))
                })
                .collect(),
            tool_runtime_metadata: server.runtime_metadata.tools.clone(),
            trusts_tool_input: server.runtime_metadata.trusts_tool_input,
            trusts_approval_context: server.runtime_metadata.trusts_approval_context,
            sandbox_state_source: server.runtime_metadata.sandbox_state_source,
            approvals_reviewer: server.runtime_metadata.approvals_reviewer,
            _runtime_owner: server.runtime_owner.clone(),
        }
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
