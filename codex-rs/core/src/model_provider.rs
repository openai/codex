use std::fmt;
use std::sync::Arc;

use codex_api::AuthProvider;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;

/// Runtime provider abstraction used by turn execution.
///
/// Implementations own provider-specific behavior for a model backend. The
/// `ModelProviderInfo` returned by `info` is the serialized/configured provider
/// metadata used by the generic OpenAI-compatible implementation.
pub(crate) trait ModelProvider: fmt::Debug + Send + Sync {
    fn info(&self) -> &ModelProviderInfo;

    fn auth_manager(&self) -> Option<&AuthManager>;

    fn auth_provider(
        &self,
        auth: Option<CodexAuth>,
    ) -> codex_protocol::error::Result<Arc<dyn AuthProvider>>;
}

impl dyn ModelProvider {
    pub(crate) fn new(
        info: ModelProviderInfo,
        auth_manager: Option<Arc<AuthManager>>,
    ) -> Arc<Self> {
        Arc::new(GenericModelProvider { info, auth_manager })
    }
}

/// Generic OpenAI-compatible model provider backed by a `ModelProviderInfo`.
#[derive(Clone, Debug)]
struct GenericModelProvider {
    info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
}

impl ModelProvider for GenericModelProvider {
    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn auth_manager(&self) -> Option<&AuthManager> {
        self.auth_manager.as_deref()
    }

    fn auth_provider(
        &self,
        auth: Option<CodexAuth>,
    ) -> codex_protocol::error::Result<Arc<dyn AuthProvider>> {
        codex_provider_auth::resolve_provider_auth(auth, &self.info)
    }
}
