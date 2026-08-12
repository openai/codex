use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use crate::runtime::McpRuntimeContext;
use codex_api::SharedAuthProvider;
use codex_config::AppToolApproval;
use codex_config::McpServerAuth;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_connectors::ConnectorRuntimeContextKey;
use codex_exec_server::Environment;
use codex_login::CodexAuth;
use codex_protocol::mcp::ClientMcpExtensions;
use codex_rmcp_client::StoredOAuthCredentialSnapshot;
use codex_rmcp_client::StoredOAuthTokens;
use rmcp::model::ElicitationCapability;
use tracing::warn;

/// MCP server after runtime additions have been applied.
#[derive(Debug, Clone)]
pub struct EffectiveMcpServer {
    config: McpServerConfig,
    agent_plugin: bool,
}

impl EffectiveMcpServer {
    pub fn configured(config: McpServerConfig) -> Self {
        Self {
            config,
            agent_plugin: false,
        }
    }

    pub fn with_agent_plugin(mut self, agent_plugin: bool) -> Self {
        self.agent_plugin = agent_plugin;
        self
    }

    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn required(&self) -> bool {
        self.config.required
    }

    pub fn is_agent_plugin(&self) -> bool {
        self.agent_plugin
    }
}

pub(crate) fn has_explicit_http_authorization(config: &McpServerConfig) -> bool {
    let McpServerTransportConfig::StreamableHttp {
        bearer_token_env_var,
        http_headers,
        env_http_headers,
        ..
    } = &config.transport
    else {
        return false;
    };

    if bearer_token_env_var.is_some()
        || env_http_headers
            .as_ref()
            .is_some_and(|headers| !headers.is_empty())
    {
        return false;
    }

    http_headers.as_ref().is_some_and(|headers| {
        headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("authorization")
                && !value.trim().is_empty()
                && value
                    .bytes()
                    .all(|byte| byte == b'\t' || (byte >= b' ' && byte != 0x7f))
        })
    })
}

/// Inputs that determine the identity of a live MCP connection.
///
/// Tool policy and presentation metadata intentionally do not appear here:
/// those belong to a publication and can change without reconnecting.
#[derive(Clone)]
pub(crate) struct McpServerConnectionIdentity {
    transport: McpServerTransportConfig,
    environment_id: String,
    oauth_store: Option<(OAuthCredentialsStoreMode, AuthKeyringBackendKind)>,
    oauth_credentials: Result<Option<StoredOAuthCredentialSnapshot>, String>,
    pub(crate) oauth_store_was_contended: bool,
    resolved_environment: Result<Option<Arc<Environment>>, String>,
    local_stdio_fallback_cwd: Option<PathBuf>,
    referenced_environment_variables: Vec<(String, Option<OsString>)>,
    runtime_auth: Option<CodexAuth>,
    runtime_auth_token: Option<String>,
    codex_apps_cache_identity: Option<(PathBuf, ConnectorRuntimeContextKey)>,
    client_elicitation_capability: ElicitationCapability,
    client_mcp_extensions: ClientMcpExtensions,
    agent_plugin: bool,
}

impl McpServerConnectionIdentity {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        server_name: &str,
        server: &EffectiveMcpServer,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        resolved_environment: &Result<Option<Arc<Environment>>, String>,
        runtime_context: &McpRuntimeContext,
        runtime_auth_provider: Option<&SharedAuthProvider>,
        auth: Option<&CodexAuth>,
        codex_apps_cache_identity: Option<(PathBuf, ConnectorRuntimeContextKey)>,
        client_elicitation_capability: ElicitationCapability,
        client_mcp_extensions: ClientMcpExtensions,
        previous_identity: Option<&Self>,
    ) -> Self {
        let config = server.config();
        let valid_http_header_value = |value: &str| {
            value
                .bytes()
                .all(|byte| byte == b'\t' || (byte >= b' ' && byte != 0x7f))
        };
        let stored_oauth_url = if runtime_auth_provider.is_none()
            && (!matches!(config.auth, McpServerAuth::ChatGpt) || config.is_local_environment())
        {
            match &config.transport {
                McpServerTransportConfig::StreamableHttp {
                    url,
                    bearer_token_env_var: None,
                    http_headers,
                    env_http_headers,
                    http_headers_helper: _,
                } if !http_headers.as_ref().is_some_and(|headers| {
                    headers.iter().any(|(name, value)| {
                        name.eq_ignore_ascii_case("authorization") && valid_http_header_value(value)
                    })
                }) && !env_http_headers.as_ref().is_some_and(|headers| {
                    headers.iter().any(|(name, env_var)| {
                        name.eq_ignore_ascii_case("authorization")
                            && std::env::var(env_var).is_ok_and(|value| {
                                !value.trim().is_empty() && valid_http_header_value(&value)
                            })
                    })
                }) =>
                {
                    Some(url)
                }
                McpServerTransportConfig::StreamableHttp { .. }
                | McpServerTransportConfig::Stdio { .. } => None,
            }
        } else {
            None
        };
        let oauth_credentials = stored_oauth_url.map_or(Ok(None), |url| {
            let credential_name = config.oauth_credential_name(server_name);
            StoredOAuthCredentialSnapshot::for_runtime_refresh(
                previous_identity.and_then(|previous_identity| {
                    previous_identity
                        .oauth_credentials
                        .as_ref()
                        .ok()
                        .and_then(Option::as_ref)
                        .filter(|_| {
                            previous_identity.oauth_store
                                == Some((store_mode, keyring_backend_kind))
                        })
                }),
                credential_name.as_ref(),
                url,
                store_mode,
                keyring_backend_kind,
            )
            .map_err(|error| {
                warn!(server_name, %error, "failed to read stored MCP OAuth credentials");
                error.to_string()
            })
        });
        let local_stdio_fallback_cwd = (config.is_local_environment()
            && matches!(
                config.transport,
                McpServerTransportConfig::Stdio { cwd: None, .. }
                    | McpServerTransportConfig::StreamableHttp {
                        http_headers_helper: Some(_),
                        ..
                    }
            ))
        .then(|| runtime_context.local_process_cwd());
        let referenced_environment_variables = referenced_environment_variables(config);
        let runtime_auth = runtime_auth_provider.and(auth).cloned();
        let runtime_auth_token = runtime_auth.as_ref().and_then(|auth| auth.get_token().ok());
        let oauth_store_was_contended = oauth_credentials
            .as_ref()
            .ok()
            .and_then(Option::as_ref)
            .is_some_and(StoredOAuthCredentialSnapshot::store_was_contended);

        Self {
            transport: config.transport.clone(),
            environment_id: config.environment_id.clone(),
            oauth_store: stored_oauth_url
                .is_some()
                .then_some((store_mode, keyring_backend_kind)),
            oauth_credentials,
            oauth_store_was_contended,
            resolved_environment: resolved_environment.clone(),
            local_stdio_fallback_cwd,
            referenced_environment_variables,
            runtime_auth,
            runtime_auth_token,
            codex_apps_cache_identity,
            client_elicitation_capability,
            client_mcp_extensions,
            agent_plugin: server.is_agent_plugin(),
        }
    }

    pub(crate) fn has_same_connection_config(&self, other: &Self) -> bool {
        let same_runtime_auth = match (&self.runtime_auth, &other.runtime_auth) {
            (Some(CodexAuth::AgentIdentity(left)), Some(CodexAuth::AgentIdentity(right))) => {
                left.record() == right.record()
            }
            (Some(left), Some(right)) => {
                left == right
                    && left.get_account_id() == right.get_account_id()
                    && left.get_chatgpt_user_id() == right.get_chatgpt_user_id()
                    && left.is_fedramp_account() == right.is_fedramp_account()
            }
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        };
        self.transport == other.transport
            && self.environment_id == other.environment_id
            && self.oauth_store == other.oauth_store
            && same_resolved_environment(&self.resolved_environment, &other.resolved_environment)
            && self.local_stdio_fallback_cwd == other.local_stdio_fallback_cwd
            && self.referenced_environment_variables == other.referenced_environment_variables
            && same_runtime_auth
            && self.runtime_auth_token == other.runtime_auth_token
            && self.codex_apps_cache_identity == other.codex_apps_cache_identity
            && self.client_elicitation_capability == other.client_elicitation_capability
            && self.client_mcp_extensions == other.client_mcp_extensions
            && self.agent_plugin == other.agent_plugin
    }

    pub(crate) fn oauth_credentials(&self) -> Result<Option<&StoredOAuthTokens>, &String> {
        self.oauth_credentials.as_ref().map(|credentials| {
            credentials
                .as_ref()
                .map(StoredOAuthCredentialSnapshot::credentials)
        })
    }

    pub(crate) fn oauth_credentials_changed(
        &self,
        server_name: &str,
        config: &McpServerConfig,
    ) -> bool {
        let Some((store_mode, keyring_backend_kind)) = self.oauth_store else {
            return false;
        };
        let McpServerTransportConfig::StreamableHttp { url, .. } = &self.transport else {
            return false;
        };

        let credential_name = config.oauth_credential_name(server_name);
        let current_credentials = match self.oauth_credentials.as_ref() {
            Ok(Some(credentials)) => credentials.reload(
                credential_name.as_ref(),
                url,
                store_mode,
                keyring_backend_kind,
            ),
            Ok(None) | Err(_) => StoredOAuthCredentialSnapshot::for_runtime_refresh(
                /*previous*/ None,
                credential_name.as_ref(),
                url,
                store_mode,
                keyring_backend_kind,
            )
            .map(|snapshot| snapshot.map(|snapshot| snapshot.credentials().clone())),
        };

        match current_credentials {
            Ok(Some(current_credentials)) => {
                self.oauth_credentials() != Ok(Some(&current_credentials))
            }
            Ok(None) => false,
            Err(error) => {
                warn!(server_name, %error, "failed to read stored MCP OAuth credentials");
                false
            }
        }
    }
}

impl PartialEq for McpServerConnectionIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.has_same_connection_config(other) && self.oauth_credentials == other.oauth_credentials
    }
}

fn same_resolved_environment(
    left: &Result<Option<Arc<Environment>>, String>,
    right: &Result<Option<Arc<Environment>>, String>,
) -> bool {
    match (left, right) {
        (Ok(Some(left)), Ok(Some(right))) => Arc::ptr_eq(left, right),
        (Ok(None), Ok(None)) => true,
        (Err(left), Err(right)) => left == right,
        (Ok(_), Ok(_)) | (Ok(_), Err(_)) | (Err(_), Ok(_)) => false,
    }
}

fn referenced_environment_variables(config: &McpServerConfig) -> Vec<(String, Option<OsString>)> {
    let mut names = match &config.transport {
        McpServerTransportConfig::Stdio { env_vars, .. } => env_vars
            .iter()
            .filter(|env_var| !env_var.is_remote_source())
            .map(|env_var| env_var.name().to_string())
            .collect::<Vec<_>>(),
        McpServerTransportConfig::StreamableHttp {
            bearer_token_env_var,
            env_http_headers,
            ..
        } => bearer_token_env_var
            .iter()
            .chain(env_http_headers.iter().flat_map(|headers| headers.values()))
            .cloned()
            .collect(),
    };
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|name| {
            let value = std::env::var_os(&name);
            (name, value)
        })
        .collect()
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
    pub default_tools_approval_mode: Option<AppToolApproval>,
    pub tool_approval_modes: HashMap<String, AppToolApproval>,
}

impl McpServerMetadata {
    pub fn tool_approval_mode(&self, tool_name: &str) -> AppToolApproval {
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
            origin: McpServerOrigin::from_transport(&config.transport),
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
        }
    }
}
