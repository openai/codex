//! Adapt the SDK enterprise exchange to Codex HTTP routing and credential lifecycle errors.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_exec_server::HttpClient;
use oauth2::RefreshToken;
use rmcp::transport::auth::enterprise::EmaAuthorizationServer;
use rmcp::transport::auth::enterprise::EmaClientAuthentication;
use rmcp::transport::auth::enterprise::EmaError;
use rmcp::transport::auth::enterprise::EmaExchangeRequest;
use rmcp::transport::auth::enterprise::EmaExchangeStage;
use tokio::time::Instant;

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
    /// Exchange completion time, before any asynchronous credential validation.
    pub received_at: Instant,
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
    pub idp_client_authentication: EmaClientAuthentication,
    pub resource_client_authentication: EmaClientAuthentication,
    pub refresh_token: String,
    pub idp_http_client: Arc<dyn HttpClient>,
    pub resource_http_client: Arc<dyn HttpClient>,
}

/// Exchanges an enterprise IdP credential for a resource-bound MCP bearer token.
pub(crate) async fn exchange_id_jag<L: Send, R: Send>(
    request: EmaIdJagExchangeRequest<'_>,
    credentials: L,
    finish_idp: impl FnOnce(L) -> R + Send,
) -> Result<(EmaAccessToken, R)> {
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
    // Discovery has already verified each registration's authentication method. The SDK owns
    // token validation and the two exchanges; Codex retains routing and credential policy.
    let assertion = EmaExchangeRequest::new(
        EmaAuthorizationServer::new(
            request.idp_issuer,
            request.idp_token_endpoint,
            request.idp_client_id,
        )
        .with_client_authentication(request.idp_client_authentication),
        EmaAuthorizationServer::new(
            request.authorization_server_issuer,
            request.authorization_server_token_endpoint,
            request.mcp_client_id,
        )
        .with_client_authentication(request.resource_client_authentication),
        request.resource,
        &refresh_token,
    )
    .with_scopes(request.scopes.iter().cloned())
    .exchange_id_jag(&idp_http)
    .await
    .map_err(exchange_error)?;
    drop(refresh_token);
    // Only the IdP uses the refresh token. Release its lease before resource-AS I/O.
    let receipt = finish_idp(credentials);
    let token = assertion
        .exchange(&resource_http)
        .await
        .map_err(exchange_error)?;
    Ok((
        EmaAccessToken {
            access_token: token.access_token.secret().to_owned(),
            expires_in: token.expires_in,
            received_at: Instant::now(),
        },
        receipt,
    ))
}

fn exchange_error(error: EmaError) -> anyhow::Error {
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
        EmaError::InsufficientUserAuthentication(EmaExchangeStage::IdentityProvider) => {
            Some(EmaAuthFailure::InsufficientUserAuthentication {
                grant_source: EmaInvalidGrantSource::EnterpriseIdentity,
            })
        }
        EmaError::InsufficientUserAuthentication(EmaExchangeStage::ResourceAuthorizationServer) => {
            Some(EmaAuthFailure::InsufficientUserAuthentication {
                grant_source: EmaInvalidGrantSource::ResourceAuthorization,
            })
        }
        EmaError::OAuthRejected {
            stage: EmaExchangeStage::IdentityProvider,
            code: "unauthorized_client" | "invalid_client",
            ..
        } => Some(EmaAuthFailure::IdentityClientRejected),
        // Future SDK stages must not invalidate the shared enterprise credential.
        _ => None,
    };
    match failure {
        Some(failure) => anyhow::Error::new(failure).context(error),
        None => anyhow::Error::new(error),
    }
}

#[cfg(test)]
#[path = "ema_exchange_tests.rs"]
mod tests;
