use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use codex_api::Provider;
use codex_api::SharedAuthProvider;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;

use crate::auth::auth_manager_for_provider;
use crate::auth::resolve_provider_auth;

/// Runtime provider abstraction used by model execution.
///
/// Implementations own provider-specific behavior for a model backend. The
/// `ModelProviderInfo` returned by `info` is the serialized/configured provider
/// metadata used by the default OpenAI-compatible implementation.
#[async_trait]
pub trait ModelProvider: fmt::Debug + Send + Sync {
    /// Returns the configured provider metadata.
    fn info(&self) -> &ModelProviderInfo;

    /// Returns the provider-scoped auth manager, when this provider uses one.
    fn auth_manager(&self) -> Option<Arc<AuthManager>>;

    /// Resolves the auth and API-provider configuration for a request.
    async fn resolve_auth(&self) -> codex_protocol::error::Result<ResolvedProviderAuth>;
}

/// Shared runtime model provider handle.
pub type SharedModelProvider = Arc<dyn ModelProvider>;

/// Auth and provider configuration resolved for a model-provider request.
pub struct ResolvedProviderAuth {
    /// The current Codex auth session, when one is available.
    pub auth: Option<CodexAuth>,
    /// Provider configuration adapted for the API client.
    pub api_provider: Provider,
    /// Auth provider used to attach request credentials.
    pub api_auth: SharedAuthProvider,
}

/// Creates the default runtime model provider for configured provider metadata.
pub fn create_model_provider(
    provider_info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
) -> SharedModelProvider {
    let auth_manager = auth_manager_for_provider(auth_manager, &provider_info);
    Arc::new(ConfiguredModelProvider {
        info: provider_info,
        auth_manager,
    })
}

/// Runtime model provider backed by configured `ModelProviderInfo`.
#[derive(Clone, Debug)]
struct ConfiguredModelProvider {
    info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
}

#[async_trait]
impl ModelProvider for ConfiguredModelProvider {
    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }

    async fn resolve_auth(&self) -> codex_protocol::error::Result<ResolvedProviderAuth> {
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
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use codex_protocol::config_types::ModelProviderAuthInfo;

    use super::*;

    fn provider_info_with_command_auth() -> ModelProviderInfo {
        ModelProviderInfo {
            auth: Some(ModelProviderAuthInfo {
                command: "print-token".to_string(),
                args: Vec::new(),
                timeout_ms: NonZeroU64::new(5_000).expect("timeout should be non-zero"),
                refresh_interval_ms: 300_000,
                cwd: std::env::current_dir()
                    .expect("current dir should be available")
                    .try_into()
                    .expect("current dir should be absolute"),
            }),
            requires_openai_auth: false,
            ..ModelProviderInfo::create_openai_provider(/*base_url*/ None)
        }
    }

    #[test]
    fn create_model_provider_builds_command_auth_manager_without_base_manager() {
        let provider = create_model_provider(provider_info_with_command_auth(), None);

        let auth_manager = provider
            .auth_manager()
            .expect("command auth provider should have an auth manager");

        assert!(auth_manager.has_external_auth());
    }
}
