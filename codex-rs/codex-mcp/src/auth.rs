//! Authentication shared by requests on an MCP connection.

use std::sync::Arc;

use codex_api::AuthError;
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
