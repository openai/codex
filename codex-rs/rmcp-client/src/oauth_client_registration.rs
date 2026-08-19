use std::sync::Arc;

use anyhow::Result;
use anyhow::bail;
use rmcp::transport::AuthorizationManager;
use rmcp::transport::AuthorizationRequest;
use rmcp::transport::AuthorizationSession;
use rmcp::transport::auth::OAuthHttpClient;
use rmcp::transport::auth::OAuthState;
use url::Url;

/// OAuth client-registration strategy for one interactive HTTP MCP login.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum McpOAuthClientRegistration {
    /// Prefer a supported native CIMD and otherwise use advertised DCR.
    #[default]
    Auto,
    /// Require a ChatGPT-hosted Codex public native Client ID Metadata Document.
    Cimd,
    /// Require the authorization server's Dynamic Client Registration endpoint.
    Dcr,
}

/// OAuth state prepared from one authorization-server metadata resolution.
pub(crate) struct PreparedOAuthLogin {
    pub(crate) oauth_state: OAuthState,
    pub(crate) authorization_server_issuer: Option<String>,
}

pub(crate) async fn start_authorization(
    server_url: &str,
    http_client: Arc<dyn OAuthHttpClient>,
    scopes: &[&str],
    redirect_uri: &str,
    callback_id: &str,
    client_registration: McpOAuthClientRegistration,
) -> Result<PreparedOAuthLogin> {
    let mut auth_manager =
        AuthorizationManager::new_with_oauth_http_client(server_url, http_client).await?;
    auth_manager.set_allow_missing_issuer(true);
    let metadata = auth_manager.resolve_metadata().await?.metadata;
    let authorization_server_issuer = metadata.issuer.clone();

    let cimd_advertised = metadata
        .additional_fields
        .get("client_id_metadata_document_supported")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let public_client_auth_supported = metadata
        .additional_fields
        .get("token_endpoint_auth_methods_supported")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|methods| methods.iter().any(|method| method.as_str() == Some("none")));

    let parsed_redirect_uri = Url::parse(redirect_uri)?;
    let native_redirect_supported = parsed_redirect_uri.scheme() == "http"
        && matches!(
            parsed_redirect_uri.host_str(),
            Some("127.0.0.1" | "localhost")
        )
        && parsed_redirect_uri.port().is_some_and(|port| port > 0)
        && parsed_redirect_uri.path() == format!("/callback/{callback_id}")
        && parsed_redirect_uri.query().is_none()
        && parsed_redirect_uri.fragment().is_none()
        && parsed_redirect_uri.username().is_empty()
        && parsed_redirect_uri.password().is_none();
    // MCP 2026-07-28 priority: pre-registered clients never reach this path; offer
    // advertised CIMD here and otherwise let rmcp fall back to DCR.
    // https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization/client-registration
    let offer_cimd = match client_registration {
        McpOAuthClientRegistration::Auto => {
            cimd_advertised && native_redirect_supported && public_client_auth_supported
        }
        McpOAuthClientRegistration::Cimd => {
            if !cimd_advertised || !public_client_auth_supported {
                bail!(
                    "MCP authorization server does not advertise CIMD with token endpoint auth method `none`"
                );
            }
            if !native_redirect_supported {
                bail!(
                    "MCP OAuth CIMD requires an ephemeral loopback callback at `/callback/{callback_id}`"
                );
            }
            true
        }
        McpOAuthClientRegistration::Dcr => false,
    };

    auth_manager.set_metadata(metadata);
    let mut request = AuthorizationRequest::new(redirect_uri)
        .with_scopes(scopes.iter().copied())
        .with_client_name("Codex");
    if offer_cimd {
        // CIMD is an active IETF Internet-Draft: this HTTPS client identifier resolves
        // to its self-referential JSON metadata document.
        // https://datatracker.ietf.org/doc/draft-ietf-oauth-client-id-metadata-document/
        request = request.with_client_metadata_url(format!(
            "https://chatgpt.com/oauth/codex/{callback_id}/client.json"
        ));
    }
    let session = AuthorizationSession::new(auth_manager, request)
        .await
        .map_err(|(_auth_manager, error)| error)?;

    Ok(PreparedOAuthLogin {
        oauth_state: OAuthState::Session(session),
        authorization_server_issuer,
    })
}

#[cfg(test)]
#[path = "oauth_client_registration_tests.rs"]
mod tests;
