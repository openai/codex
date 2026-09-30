//! Adapt the SDK enterprise exchange to Codex HTTP routing and credential lifecycle errors.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_exec_server::HttpClient;
use oauth2::RefreshToken;
use rmcp::transport::auth::enterprise::EmaAuthorizationServer;
use rmcp::transport::auth::enterprise::EmaError;
use rmcp::transport::auth::enterprise::EmaExchangeRequest;
use rmcp::transport::auth::enterprise::EmaExchangeStage;

use crate::ema_auth_policy::EmaAuthFailure;
use crate::ema_auth_policy::EmaInvalidGrantSource;
use crate::http_client_adapter::StreamableHttpRedirectMode;
use crate::oauth_http_client::OAuthHttpClientAdapter;
use crate::utils::build_default_headers;

pub(crate) const TOKEN_EXCHANGE_GRANT_TYPE: &str =
    "urn:ietf:params:oauth:grant-type:token-exchange";
pub(crate) const JWT_BEARER_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";

/// A resource-bound bearer and its server-reported lifetime, with redacted diagnostics.
#[derive(Clone, PartialEq, Eq)]
pub struct EmaAccessToken {
    pub access_token: String,
    pub expires_in: Option<Duration>,
}

impl std::fmt::Debug for EmaAccessToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EmaAccessToken")
            .field("access_token", &"[REDACTED]")
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// The caller supplies trusted authorization-server metadata and an IdP credential.
/// This primitive does not perform resource discovery or interactive login.
pub(crate) struct EmaIdJagExchangeRequest<'a> {
    pub resource: &'a str,
    pub scopes: &'a [String],
    pub mcp_client_id: &'a str,
    pub authorization_server_issuer: &'a str,
    pub authorization_server_token_endpoint: &'a str,
    pub idp_token_endpoint: &'a str,
    pub idp_issuer: &'a str,
    pub idp_client_id: &'a str,
    pub refresh_token: String,
    pub idp_http_client: Arc<dyn HttpClient>,
    pub resource_http_client: Arc<dyn HttpClient>,
}

/// Exchanges an enterprise IdP credential for a resource-bound MCP bearer token.
pub(crate) async fn exchange_id_jag(
    request: EmaIdJagExchangeRequest<'_>,
) -> Result<EmaAccessToken> {
    let idp_http = OAuthHttpClientAdapter::new_with_redirect_mode(
        request.idp_http_client,
        build_default_headers(/*http_headers*/ None, /*env_http_headers*/ None)?,
        request.idp_token_endpoint,
        /*has_configured_headers*/ false,
        StreamableHttpRedirectMode::Legacy,
    )?;
    let resource_http = OAuthHttpClientAdapter::new_with_redirect_mode(
        request.resource_http_client,
        build_default_headers(/*http_headers*/ None, /*env_http_headers*/ None)?,
        request.authorization_server_token_endpoint,
        /*has_configured_headers*/ false,
        StreamableHttpRedirectMode::Legacy,
    )?;
    let refresh_token = RefreshToken::new(request.refresh_token);
    // Discovery has already verified both public-client registrations. The SDK owns
    // token validation and the two exchanges; Codex retains routing and credential policy.
    let token = EmaExchangeRequest::new(
        EmaAuthorizationServer::new(
            request.idp_issuer,
            request.idp_token_endpoint,
            request.idp_client_id,
        ),
        EmaAuthorizationServer::new(
            request.authorization_server_issuer,
            request.authorization_server_token_endpoint,
            request.mcp_client_id,
        ),
        request.resource,
        &refresh_token,
    )
    .with_scopes(request.scopes.iter().cloned())
    .exchange(&idp_http, &resource_http)
    .await
    .map_err(|error| {
        let failure = match &error {
            EmaError::InvalidGrant(EmaExchangeStage::IdentityProvider) => {
                Some(EmaAuthFailure::InvalidGrant {
                    grant_source: EmaInvalidGrantSource::EnterpriseIdentity,
                })
            }
            EmaError::InvalidGrant(EmaExchangeStage::ResourceAuthorizationServer) => {
                Some(EmaAuthFailure::InvalidGrant {
                    grant_source: EmaInvalidGrantSource::ResourceAuthorization,
                })
            }
            EmaError::InsufficientUserAuthentication(_) => {
                Some(EmaAuthFailure::InsufficientUserAuthentication)
            }
            // Future SDK stages must not invalidate the shared enterprise credential.
            _ => None,
        };
        match failure {
            Some(failure) => anyhow::Error::new(failure).context(error),
            None => anyhow::Error::new(error),
        }
    })?;
    Ok(EmaAccessToken {
        access_token: token.access_token.secret().to_owned(),
        expires_in: token.expires_in,
    })
}

#[cfg(test)]
#[path = "ema_exchange_tests.rs"]
mod tests;
