//! Authentication for clients that reuse a connection across requests.

use crate::ResolvedProviderAuth;
use codex_api::AuthError;
use codex_api::SharedAuthProvider;
use codex_features::RuntimeAuthMode;
use codex_http_client::HttpClientFactory;
use codex_login::CodexAuth;
use codex_protocol::error::Result as CodexResult;
use codex_runtime_agent_instance::RuntimeAuthBinding;
use codex_runtime_agent_instance::RuntimeAuthManager;
use http::HeaderMap;
use std::sync::Arc;

/// Keeps a reusable client's credential source fixed while allowing token refresh.
/// Each request resolves fresh auth using the same source as the client transport.
#[derive(Clone)]
pub struct ClientAuth {
    source: ClientAuthSource,
}

#[derive(Clone)]
enum ClientAuthSource {
    Provider(SharedAuthProvider),
    Runtime {
        manager: Arc<RuntimeAuthBinding>,
        mode: RuntimeAuthMode,
        source_generation: u64,
    },
}

impl ClientAuth {
    pub fn new(provider: SharedAuthProvider) -> Self {
        Self {
            source: ClientAuthSource::Provider(provider),
        }
    }

    /// Select the auth source and matching transport for a reusable client's request headers.
    /// The caller selects runtime auth only for a supported OpenAI destination.
    pub async fn for_auth(
        auth: &CodexAuth,
        runtime: Option<&(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
        factory: HttpClientFactory,
        headers: &HeaderMap,
    ) -> Result<(Self, HttpClientFactory), AuthError> {
        let resolved = crate::auth_provider_for_request(auth, runtime, headers).await?;
        let factory = resolved.bind_http_client_factory(factory)?;
        let source = match (runtime, resolved.runtime) {
            (Some((_, mode)), Some(selected)) => ClientAuthSource::Runtime {
                manager: Arc::clone(selected.binding()),
                mode: *mode,
                source_generation: selected.change_state().source_generation,
            },
            _ => ClientAuthSource::Provider(resolved.auth),
        };
        Ok((Self { source }, factory))
    }

    /// Replace ordinary authentication; reject changes to a runtime-bound client.
    pub fn set_provider(&mut self, provider: SharedAuthProvider) -> Result<(), AuthError> {
        match &mut self.source {
            ClientAuthSource::Provider(current) => {
                *current = provider;
                Ok(())
            }
            ClientAuthSource::Runtime { .. } => Err(AuthError::Build(
                "cannot replace credentials on a runtime-bound client".to_string(),
            )),
        }
    }

    /// Resolve one request, preserving the source that authenticated the client.
    pub async fn resolve(&self, headers: &HeaderMap) -> Result<ResolvedProviderAuth, AuthError> {
        let mode = match &self.source {
            ClientAuthSource::Provider(_) => None,
            ClientAuthSource::Runtime { mode, .. } => Some(*mode),
        };
        crate::auth::observe_auth_selection(mode, "client_request", async {
            match &self.source {
                ClientAuthSource::Provider(provider) => {
                    Ok(ResolvedProviderAuth::new(Arc::clone(provider)))
                }
                ClientAuthSource::Runtime {
                    manager,
                    mode,
                    source_generation,
                } => {
                    let selected =
                        crate::select_runtime_auth_for_source(manager, *source_generation, headers)
                            .await?;
                    Ok(ResolvedProviderAuth::for_runtime(selected, *mode))
                }
            }
        })
        .await
    }

    /// Allow OpenAI requests and the original origin, never an unrelated provider.
    pub fn validate_destination(&self, base_url: &str, url: &str) -> CodexResult<()> {
        if matches!(&self.source, ClientAuthSource::Provider(_)) {
            return Ok(());
        }
        crate::validate_runtime_destination(Some(url))?;
        if codex_runtime_agent_instance::is_openai_api_url(url) {
            return Ok(());
        }
        let origin = |value| {
            url::Url::parse(value)
                .map(|url| url.origin())
                .map_err(std::io::Error::other)
        };
        if origin(url)? != origin(base_url)? {
            return Err(std::io::Error::other(
                "runtime-authenticated clients cannot send requests to another provider",
            )
            .into());
        }
        Ok(())
    }
}
