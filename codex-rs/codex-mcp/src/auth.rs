//! Authentication shared by requests on an MCP connection.

use std::fmt;
use std::sync::Arc;

use codex_api::AuthError;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_exec_server::RouteAwareHttpClient;
use codex_features::RuntimeAuthMode;
use codex_http_client::HttpClientFactory;
use codex_login::CodexAuth;
use codex_model_provider::ClientAuth;
use codex_rmcp_client::AuthProviderResolver;
use codex_runtime_agent_instance::RuntimeAuthSlot;
use futures::future::BoxFuture;
use http::HeaderMap;
use tokio::sync::OnceCell;

use crate::McpRuntimeContext;

/// Supplies credentials for host-local MCP HTTP requests.
pub trait McpRequestAuth: Send + Sync {
    /// Returns the active owner without building request headers.
    /// The key must distinguish owners within this provider and stay stable
    /// across credential refreshes. Return an error if there is no active owner.
    fn owner_key(&self) -> Result<String, AuthError>;

    /// Adds auth headers and returns the owner from the same identity snapshot.
    /// Core checks this key before sending on the MCP session.
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<String, AuthError>;
}

/// Identifies an in-process MCP credential source without exposing credential values.
#[derive(Clone)]
pub struct McpRequestAuthProvider(Arc<dyn McpRequestAuth>);

impl McpRequestAuthProvider {
    pub fn new(provider: Arc<dyn McpRequestAuth>) -> Self {
        Self(provider)
    }

    pub(crate) fn bind(self) -> McpRequestAuthSession {
        McpRequestAuthSession {
            // Preserve a failed lookup so startup can report it without using fallback auth.
            owner: self.0.owner_key(),
            provider: self,
        }
    }
}

impl fmt::Debug for McpRequestAuthProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("McpRequestAuthProvider")
    }
}

impl PartialEq for McpRequestAuthProvider {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Captures the owner lookup for a connection attempt.
#[derive(Clone)]
pub(crate) struct McpRequestAuthSession {
    provider: McpRequestAuthProvider,
    owner: Result<String, AuthError>,
}

impl PartialEq for McpRequestAuthSession {
    fn eq(&self, other: &Self) -> bool {
        self.provider == other.provider && self.owner.as_ref().ok() == other.owner.as_ref().ok()
    }
}

impl fmt::Debug for McpRequestAuthSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpRequestAuthSession")
            .field("provider", &self.provider)
            .finish_non_exhaustive()
    }
}

impl McpRequestAuthSession {
    pub(crate) fn needs_owner_refresh(&self) -> bool {
        // Repeated failures leave this server unavailable. A change in availability
        // or owner needs a new connection attempt.
        self.provider.0.owner_key().as_ref().ok() != self.owner.as_ref().ok()
    }
}

impl AuthProvider for McpRequestAuthSession {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), AuthError> {
        let session_owner = self.owner.as_ref().map_err(Clone::clone)?;
        let request_owner = self.provider.0.add_auth_headers(headers)?;
        if &request_owner != session_owner {
            return Err(AuthError::Build(
                "MCP request auth owner changed".to_string(),
            ));
        }
        Ok(())
    }
}

/// A connection keeps its credential source across retries and reconnects. Initial runtime
/// selection happens inside the startup deadline; each HTTP request then selects fresh auth.
#[derive(Clone)]
pub(crate) enum McpAuthProvider {
    Credentials(SharedAuthProvider),
    Runtime(Arc<RuntimeMcpAuth>),
}

pub(crate) struct RuntimeMcpAuth {
    slot: RuntimeAuthSlot,
    mode: RuntimeAuthMode,
    auth: CodexAuth,
    client_auth: OnceCell<ClientAuth>,
}

struct RuntimeMcpRequestAuth {
    auth: ClientAuth,
    http_client_factory: HttpClientFactory,
}

impl McpAuthProvider {
    pub(crate) fn runtime(
        (slot, mode): (RuntimeAuthSlot, RuntimeAuthMode),
        auth: CodexAuth,
    ) -> Self {
        Self::Runtime(Arc::new(RuntimeMcpAuth {
            slot,
            mode,
            auth,
            client_auth: OnceCell::new(),
        }))
    }

    pub(crate) async fn resolve(
        &self,
        mut context: McpRuntimeContext,
        url: &str,
    ) -> anyhow::Result<(Arc<dyn AuthProviderResolver>, McpRuntimeContext)> {
        match self {
            Self::Credentials(provider) => Ok((Arc::new(Arc::clone(provider)), context)),
            Self::Runtime(runtime) => {
                codex_model_provider::validate_runtime_destination(Some(url))?;
                let manager = runtime.slot.get().ok_or_else(|| {
                    AuthError::Build("runtime authentication is not initialized".to_string())
                })?;
                let auth = runtime
                    .client_auth
                    .get_or_try_init(|| async {
                        ClientAuth::for_auth(
                            &runtime.auth,
                            Some(&(Arc::clone(manager), runtime.mode)),
                            context.http_client_factory(),
                            &HeaderMap::new(),
                        )
                        .await
                        .map(|(auth, _)| auth)
                    })
                    .await?;
                let selected = auth.resolve(&HeaderMap::new()).await?;
                let factory = selected.bind_http_client_factory(context.http_client_factory())?;
                context.set_local_http_client(Arc::new(
                    RouteAwareHttpClient::new(factory.clone()).with_tls_backend_fallback(),
                ));
                Ok((
                    Arc::new(RuntimeMcpRequestAuth {
                        auth: auth.clone(),
                        http_client_factory: factory,
                    }),
                    context,
                ))
            }
        }
    }
}

impl AuthProviderResolver for RuntimeMcpRequestAuth {
    fn resolve<'a>(
        &'a self,
        headers: &'a HeaderMap,
    ) -> BoxFuture<'a, Result<SharedAuthProvider, AuthError>> {
        Box::pin(async move {
            let selected = self.auth.resolve(headers).await?;
            let factory = selected.bind_http_client_factory(self.http_client_factory.clone())?;
            // Reuse the connection's transport, including its original account-policy binding.
            // A newly selected policy must not revive a connection that was already revoked.
            if factory != self.http_client_factory {
                return Err(AuthError::Build(
                    "runtime authentication changed the MCP connection's network policy"
                        .to_string(),
                ));
            }
            Ok(selected.auth)
        })
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
