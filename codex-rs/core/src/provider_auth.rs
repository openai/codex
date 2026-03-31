use std::sync::Arc;

use crate::AuthManager;
use crate::model_provider_info::ModelProviderInfo;

/// Returns the provider-scoped auth manager when this provider uses command-backed auth.
///
/// Providers without custom auth continue using the caller-supplied base manager.
pub(crate) fn auth_manager_for_provider(
    auth_manager: Option<Arc<AuthManager>>,
    provider: &ModelProviderInfo,
) -> Option<Arc<AuthManager>> {
    match provider.auth.clone() {
        Some(config) => Some(AuthManager::external_bearer_only(config)),
        None => auth_manager,
    }
}

/// Returns an auth manager for request paths that always require authentication.
///
/// Providers with command-backed auth get a bearer-only manager; otherwise the caller's manager
/// is reused unchanged.
pub(crate) fn required_auth_manager_for_provider(
    auth_manager: Arc<AuthManager>,
    provider: &ModelProviderInfo,
) -> Arc<AuthManager> {
    match provider.auth.clone() {
        Some(config) => AuthManager::external_bearer_only(config),
        None => auth_manager,
    }
}
