use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use codex_api::AuthProvider;
use codex_api::Provider;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;

use crate::provider_auth::auth_manager_for_provider;
use crate::provider_auth::resolve_provider_auth;

/// Runtime provider abstraction used by model execution.
///
/// Implementations own provider-specific behavior for a model backend. The
/// `ModelProviderInfo` returned by `info` is the serialized/configured provider
/// metadata used by the default OpenAI-compatible implementation.
pub trait ModelProvider: fmt::Debug + Send + Sync {
    /// Returns the configured provider metadata.
    fn info(&self) -> &ModelProviderInfo;

    /// Returns the provider-scoped auth manager, when this provider uses one.
    fn auth_manager(&self) -> Option<Arc<AuthManager>>;

    /// Resolves the auth and API-provider configuration for a request.
    fn resolve_auth(&self) -> ModelProviderAuthFuture<'_>;
}

/// Shared runtime model provider handle.
pub type SharedModelProvider = Arc<dyn ModelProvider>;

/// Future returned while resolving model-provider auth.
pub type ModelProviderAuthFuture<'a> =
    Pin<Box<dyn Future<Output = codex_protocol::error::Result<ResolvedProviderAuth>> + Send + 'a>>;

/// Auth and provider configuration resolved for a model-provider request.
pub struct ResolvedProviderAuth {
    /// The current Codex auth session, when one is available.
    pub auth: Option<CodexAuth>,
    /// Provider configuration adapted for the API client.
    pub api_provider: Provider,
    /// Auth provider used to attach request credentials.
    pub api_auth: Arc<dyn AuthProvider>,
}

/// Creates the default runtime model provider for configured provider metadata.
pub fn create_model_provider(
    info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
) -> SharedModelProvider {
    let auth_manager =
        auth_manager.map(|auth_manager| auth_manager_for_provider(auth_manager, &info));
    Arc::new(ConfiguredModelProvider { info, auth_manager })
}

/// Runtime model provider backed by configured `ModelProviderInfo`.
#[derive(Clone, Debug)]
struct ConfiguredModelProvider {
    info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
}

impl ModelProvider for ConfiguredModelProvider {
    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }

    fn resolve_auth(&self) -> ModelProviderAuthFuture<'_> {
        Box::pin(async {
            let auth = match self.auth_manager.as_ref() {
                Some(auth_manager) => auth_manager.auth().await,
                None => None,
            };
            let api_provider = self
                .info
                .to_api_provider(auth.as_ref().map(CodexAuth::auth_mode))?;
            let api_auth = resolve_provider_auth(auth.clone(), &self.info)?;
            Ok(ResolvedProviderAuth {
                auth,
                api_provider,
                api_auth,
            })
        })
    }
}
