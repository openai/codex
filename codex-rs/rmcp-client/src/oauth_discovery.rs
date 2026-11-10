//! Resolves published OAuth metadata through RMCP and Codex's existing HTTP adapter.
//! The caller supplies network policy and an overall discovery deadline.

use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::HttpClient;
use http::HeaderMap;
use rmcp::transport::AuthorizationManager;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationMetadata;

use crate::http_client_adapter::StreamableHttpRedirectMode;
use crate::oauth_http_client::OAuthHttpClientAdapter;

/// Discovers published authorization-server metadata with browser and token endpoints.
///
/// Returns `Ok(None)` when RMCP finds no usable document, for example when a
/// direct issuer publishes only device and token endpoints. Discards synthesized
/// legacy endpoints and propagates discovery errors.
pub async fn discover_authorization_metadata(
    url: &str,
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<AuthorizationMetadata>> {
    let client = OAuthHttpClientAdapter::new_with_redirect_mode(
        http_client,
        HeaderMap::new(),
        url,
        /*has_configured_headers*/ false,
        StreamableHttpRedirectMode::Legacy,
    )?;
    let manager = AuthorizationManager::new_with_oauth_http_client(url, Arc::new(client)).await?;
    match manager.resolve_metadata().await {
        Ok(resolution) if !resolution.source.is_discovered() => Ok(None),
        Ok(resolution) => Ok(Some(resolution.metadata)),
        Err(AuthError::NoAuthorizationSupport) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
#[path = "oauth_discovery_tests.rs"]
mod tests;
