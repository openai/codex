use std::sync::Arc;

use codex_api::AuthProvider;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;

use crate::bearer_auth_provider::BearerAuthProvider;

/// Returns the provider-scoped auth manager when this provider uses command-backed auth.
///
/// Providers without custom auth continue using the caller-supplied base manager.
pub(crate) fn auth_manager_for_provider(
    auth_manager: Arc<AuthManager>,
    provider: &ModelProviderInfo,
) -> Arc<AuthManager> {
    match provider.auth.clone() {
        Some(config) => AuthManager::external_bearer_only(config),
        None => auth_manager,
    }
}

fn bearer_auth_provider_from_auth(
    auth: Option<CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<BearerAuthProvider> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(BearerAuthProvider {
            token: Some(api_key),
            account_id: None,
            is_fedramp_account: false,
        });
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(BearerAuthProvider {
            token: Some(token),
            account_id: None,
            is_fedramp_account: false,
        });
    }

    if let Some(auth) = auth {
        let token = auth.get_token()?;
        Ok(BearerAuthProvider {
            token: Some(token),
            account_id: auth.get_account_id(),
            is_fedramp_account: auth.is_fedramp_account(),
        })
    } else {
        Ok(BearerAuthProvider {
            token: None,
            account_id: None,
            is_fedramp_account: false,
        })
    }
}

pub(crate) fn resolve_provider_auth(
    auth: Option<CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<Arc<dyn AuthProvider>> {
    let api_auth = bearer_auth_provider_from_auth(auth, provider)?;
    Ok(Arc::new(api_auth))
}
