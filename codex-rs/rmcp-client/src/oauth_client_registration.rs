use std::sync::Arc;

use anyhow::Result;
use rmcp::transport::AuthorizationManager;
use rmcp::transport::AuthorizationRequest;
use rmcp::transport::auth::OAuthHttpClient;
use rmcp::transport::auth::OAuthState;

/// OAuth client-registration strategy for one interactive HTTP MCP login.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum McpOAuthClientRegistration {
    /// Preserve the current automatic Dynamic Client Registration flow.
    #[default]
    Auto,
    /// Require the authorization server's Dynamic Client Registration endpoint.
    Dcr,
}

pub(crate) async fn start_authorization(
    server_url: &str,
    http_client: Arc<dyn OAuthHttpClient>,
    scopes: &[&str],
    redirect_uri: &str,
    client_registration: McpOAuthClientRegistration,
) -> Result<OAuthState> {
    let request = match client_registration {
        McpOAuthClientRegistration::Auto | McpOAuthClientRegistration::Dcr => {
            AuthorizationRequest::new(redirect_uri)
                .with_scopes(scopes.iter().copied())
                .with_client_name("Codex")
        }
    };

    let mut auth_manager =
        AuthorizationManager::new_with_oauth_http_client(server_url, http_client).await?;
    auth_manager.set_allow_missing_issuer(true);
    let mut oauth_state = OAuthState::Unauthorized(auth_manager);
    oauth_state.start_authorization(request).await?;
    Ok(oauth_state)
}
