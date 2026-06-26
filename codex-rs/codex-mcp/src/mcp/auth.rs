use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use codex_config::McpServerAuth;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::HttpClient;
use codex_login::CodexAuth;
use codex_rmcp_client::McpAuthState;
use codex_rmcp_client::OAuthProviderError;
use codex_rmcp_client::determine_streamable_http_auth_status;
use codex_rmcp_client::determine_streamable_http_auth_status_with_http_client;
use codex_rmcp_client::discover_streamable_http_oauth;
use codex_rmcp_client::discover_streamable_http_oauth_with_http_client;
use futures::FutureExt;
use futures::future::join_all;
use tracing::warn;

use crate::runtime::McpRuntimeContext;
use crate::server::EffectiveMcpServer;

#[derive(Debug, Clone)]
pub struct McpOAuthLoginConfig {
    pub url: String,
    pub http_headers: Option<HashMap<String, String>>,
    pub env_http_headers: Option<HashMap<String, String>>,
    pub discovered_scopes: Option<Vec<String>>,
}

#[derive(Debug)]
pub enum McpOAuthLoginSupport {
    Supported(McpOAuthLoginConfig),
    Unsupported,
    Unknown(anyhow::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpOAuthScopesSource {
    Explicit,
    Configured,
    Discovered,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMcpOAuthScopes {
    pub scopes: Vec<String>,
    pub source: McpOAuthScopesSource,
}

#[derive(Debug, Clone)]
pub struct McpAuthStatusEntry {
    pub config: Option<McpServerConfig>,
    pub auth_state: McpAuthState,
}

pub async fn oauth_login_support(transport: &McpServerTransportConfig) -> McpOAuthLoginSupport {
    let Some(mut config) = oauth_login_candidate(transport) else {
        return McpOAuthLoginSupport::Unsupported;
    };
    match discover_streamable_http_oauth(
        &config.url,
        config.http_headers.clone(),
        config.env_http_headers.clone(),
    )
    .await
    {
        Ok(Some(discovery)) => {
            config.discovered_scopes = discovery.scopes_supported;
            McpOAuthLoginSupport::Supported(config)
        }
        Ok(None) => McpOAuthLoginSupport::Unsupported,
        Err(err) => McpOAuthLoginSupport::Unknown(err),
    }
}

pub async fn oauth_login_support_with_http_client(
    transport: &McpServerTransportConfig,
    http_client: Arc<dyn HttpClient>,
) -> McpOAuthLoginSupport {
    let Some(mut config) = oauth_login_candidate(transport) else {
        return McpOAuthLoginSupport::Unsupported;
    };
    match discover_streamable_http_oauth_with_http_client(
        &config.url,
        config.http_headers.clone(),
        config.env_http_headers.clone(),
        http_client,
    )
    .await
    {
        Ok(Some(discovery)) => {
            config.discovered_scopes = discovery.scopes_supported;
            McpOAuthLoginSupport::Supported(config)
        }
        Ok(None) => McpOAuthLoginSupport::Unsupported,
        Err(err) => McpOAuthLoginSupport::Unknown(err),
    }
}

fn oauth_login_candidate(transport: &McpServerTransportConfig) -> Option<McpOAuthLoginConfig> {
    let McpServerTransportConfig::StreamableHttp {
        url,
        bearer_token_env_var,
        http_headers,
        env_http_headers,
    } = transport
    else {
        return None;
    };
    if bearer_token_env_var.is_some() {
        return None;
    }
    Some(McpOAuthLoginConfig {
        url: url.clone(),
        http_headers: http_headers.clone(),
        env_http_headers: env_http_headers.clone(),
        discovered_scopes: None,
    })
}

pub async fn discover_supported_scopes(
    transport: &McpServerTransportConfig,
) -> Option<Vec<String>> {
    match oauth_login_support(transport).await {
        McpOAuthLoginSupport::Supported(config) => config.discovered_scopes,
        McpOAuthLoginSupport::Unsupported | McpOAuthLoginSupport::Unknown(_) => None,
    }
}

pub async fn discover_supported_scopes_with_http_client(
    transport: &McpServerTransportConfig,
    http_client: Arc<dyn HttpClient>,
) -> Option<Vec<String>> {
    match oauth_login_support_with_http_client(transport, http_client).await {
        McpOAuthLoginSupport::Supported(config) => config.discovered_scopes,
        McpOAuthLoginSupport::Unsupported | McpOAuthLoginSupport::Unknown(_) => None,
    }
}

pub fn resolve_oauth_scopes(
    explicit_scopes: Option<Vec<String>>,
    configured_scopes: Option<Vec<String>>,
    discovered_scopes: Option<Vec<String>>,
) -> ResolvedMcpOAuthScopes {
    if let Some(scopes) = explicit_scopes {
        return ResolvedMcpOAuthScopes {
            scopes,
            source: McpOAuthScopesSource::Explicit,
        };
    }

    if let Some(scopes) = configured_scopes {
        return ResolvedMcpOAuthScopes {
            scopes,
            source: McpOAuthScopesSource::Configured,
        };
    }

    if let Some(scopes) = discovered_scopes
        && !scopes.is_empty()
    {
        return ResolvedMcpOAuthScopes {
            scopes,
            source: McpOAuthScopesSource::Discovered,
        };
    }

    ResolvedMcpOAuthScopes {
        scopes: Vec::new(),
        source: McpOAuthScopesSource::Empty,
    }
}

pub fn should_retry_without_scopes(scopes: &ResolvedMcpOAuthScopes, error: &anyhow::Error) -> bool {
    scopes.source == McpOAuthScopesSource::Discovered
        && error.downcast_ref::<OAuthProviderError>().is_some()
}

pub async fn compute_auth_statuses<'a, I>(
    servers: I,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    auth: Option<&CodexAuth>,
    runtime_context: &McpRuntimeContext,
) -> HashMap<String, McpAuthStatusEntry>
where
    I: IntoIterator<Item = (&'a String, &'a EffectiveMcpServer)>,
{
    let futures = servers.into_iter().map(|(name, server)| {
        let name = name.clone();
        let config = server.configured_config().cloned();
        let runtime_context = runtime_context.clone();
        let has_runtime_auth = config
            .as_ref()
            .is_some_and(|config| matches!(&config.auth, McpServerAuth::ChatGpt))
            && auth.is_some_and(CodexAuth::uses_codex_backend)
            && config.as_ref().is_some_and(|config| {
                matches!(
                    &config.transport,
                    McpServerTransportConfig::StreamableHttp {
                        bearer_token_env_var: None,
                        ..
                    }
                )
            });
        async move {
            let auth_state = match config.as_ref() {
                Some(config) => {
                    match compute_auth_status(
                        &name,
                        config,
                        store_mode,
                        keyring_backend_kind,
                        has_runtime_auth,
                        &runtime_context,
                    )
                    .await
                    {
                        Ok(status) => status,
                        Err(error) => {
                            warn!(
                                "failed to determine auth status for MCP server `{name}`: {error:?}"
                            );
                            McpAuthState::Unsupported
                        }
                    }
                }
                None => McpAuthState::Unsupported,
            };
            let entry = McpAuthStatusEntry { config, auth_state };
            (name, entry)
        }
    });

    join_all(futures).await.into_iter().collect()
}

async fn compute_auth_status(
    server_name: &str,
    config: &McpServerConfig,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    has_runtime_auth: bool,
    runtime_context: &McpRuntimeContext,
) -> Result<McpAuthState> {
    if !config.enabled {
        return Ok(McpAuthState::Unsupported);
    }

    if has_runtime_auth {
        return Ok(McpAuthState::BearerToken);
    }

    match &config.transport {
        McpServerTransportConfig::Stdio { .. } => Ok(McpAuthState::Unsupported),
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
        } => {
            if config.is_local_environment() {
                determine_streamable_http_auth_status(
                    server_name,
                    url,
                    bearer_token_env_var.as_deref(),
                    http_headers.clone(),
                    env_http_headers.clone(),
                    store_mode,
                    keyring_backend_kind,
                )
                .boxed()
                .await
            } else {
                let http_client = runtime_context
                    .resolve_http_client(server_name, config)
                    .map_err(anyhow::Error::msg)?;
                determine_streamable_http_auth_status_with_http_client(
                    server_name,
                    url,
                    bearer_token_env_var.as_deref(),
                    http_headers.clone(),
                    env_http_headers.clone(),
                    store_mode,
                    keyring_backend_kind,
                    http_client,
                )
                .boxed()
                .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use pretty_assertions::assert_eq;

    use super::McpOAuthScopesSource;
    use super::OAuthProviderError;
    use super::ResolvedMcpOAuthScopes;
    use super::resolve_oauth_scopes;
    use super::should_retry_without_scopes;

    #[test]
    fn resolve_oauth_scopes_prefers_explicit() {
        let resolved = resolve_oauth_scopes(
            Some(vec!["explicit".to_string()]),
            Some(vec!["configured".to_string()]),
            Some(vec!["discovered".to_string()]),
        );

        assert_eq!(
            resolved,
            ResolvedMcpOAuthScopes {
                scopes: vec!["explicit".to_string()],
                source: McpOAuthScopesSource::Explicit,
            }
        );
    }

    #[test]
    fn resolve_oauth_scopes_prefers_configured_over_discovered() {
        let resolved = resolve_oauth_scopes(
            /*explicit_scopes*/ None,
            Some(vec!["configured".to_string()]),
            Some(vec!["discovered".to_string()]),
        );

        assert_eq!(
            resolved,
            ResolvedMcpOAuthScopes {
                scopes: vec!["configured".to_string()],
                source: McpOAuthScopesSource::Configured,
            }
        );
    }

    #[test]
    fn resolve_oauth_scopes_uses_discovered_when_needed() {
        let resolved = resolve_oauth_scopes(
            /*explicit_scopes*/ None,
            /*configured_scopes*/ None,
            Some(vec!["discovered".to_string()]),
        );

        assert_eq!(
            resolved,
            ResolvedMcpOAuthScopes {
                scopes: vec!["discovered".to_string()],
                source: McpOAuthScopesSource::Discovered,
            }
        );
    }

    #[test]
    fn resolve_oauth_scopes_preserves_explicitly_empty_configured_scopes() {
        let resolved = resolve_oauth_scopes(
            /*explicit_scopes*/ None,
            Some(Vec::new()),
            Some(vec!["ignored".into()]),
        );

        assert_eq!(
            resolved,
            ResolvedMcpOAuthScopes {
                scopes: Vec::new(),
                source: McpOAuthScopesSource::Configured,
            }
        );
    }

    #[test]
    fn resolve_oauth_scopes_falls_back_to_empty() {
        let resolved = resolve_oauth_scopes(
            /*explicit_scopes*/ None, /*configured_scopes*/ None,
            /*discovered_scopes*/ None,
        );

        assert_eq!(
            resolved,
            ResolvedMcpOAuthScopes {
                scopes: Vec::new(),
                source: McpOAuthScopesSource::Empty,
            }
        );
    }

    #[test]
    fn should_retry_without_scopes_only_for_discovered_provider_errors() {
        let discovered = ResolvedMcpOAuthScopes {
            scopes: vec!["scope".to_string()],
            source: McpOAuthScopesSource::Discovered,
        };
        let provider_error = anyhow!(OAuthProviderError::new(
            Some("invalid_scope".to_string()),
            Some("scope rejected".to_string()),
        ));

        assert!(should_retry_without_scopes(&discovered, &provider_error));

        let configured = ResolvedMcpOAuthScopes {
            scopes: vec!["scope".to_string()],
            source: McpOAuthScopesSource::Configured,
        };
        assert!(!should_retry_without_scopes(&configured, &provider_error));
        assert!(!should_retry_without_scopes(
            &discovered,
            &anyhow!("timed out waiting for OAuth callback"),
        ));
    }
}
