//! Applies configured OAuth client secrets to login and refresh without persisting them.
//! A confidential client must never reuse tokens issued to a different client ID.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_config::McpServerOAuthConfig;
use oauth2::ClientSecret;
use rmcp::transport::AuthorizationManager;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::OAuthClientConfig;

#[derive(Debug, Default)]
pub(crate) struct OAuthClientCredentials {
    pub(crate) client_id: Option<String>,
    pub(crate) client_secret: Option<ClientSecret>,
}

impl OAuthClientCredentials {
    pub(crate) fn resolve(config: Option<&McpServerOAuthConfig>) -> Result<Self> {
        let client_id = config
            .and_then(|config| config.client_id.clone())
            .filter(|id| !id.trim().is_empty());
        let client_secret = config
            .and_then(|config| config.client_secret.as_ref())
            .map(|secret| ClientSecret::new(secret.to_string()));
        if client_secret.is_some() && client_id.is_none() {
            bail!("MCP OAuth client_secret requires a nonempty client_id");
        }
        if client_secret
            .as_ref()
            .is_some_and(|secret| secret.secret().trim().is_empty())
        {
            bail!("MCP OAuth client_secret must not be empty");
        }
        Ok(Self {
            client_id,
            client_secret,
        })
    }

    pub(crate) fn validate_stored_client_id(&self, stored_client_id: &str) -> Result<()> {
        if self.client_secret.is_some() && self.client_id.as_deref() != Some(stored_client_id) {
            return Err(AuthError::AuthorizationRequired).context(
                "Configured MCP OAuth client ID differs from stored credentials; log in again",
            );
        }
        Ok(())
    }

    pub(crate) fn configure_for_refresh(
        &self,
        manager: &mut AuthorizationManager,
        server_url: &str,
        stored_client_id: &str,
    ) -> Result<()> {
        self.validate_stored_client_id(stored_client_id)?;
        if let Some(secret) = &self.client_secret {
            // Preserve the manager's application type, as configure_client_id does.
            // Refresh does not send the redirect URI.
            let mut config = OAuthClientConfig::new(stored_client_id, server_url)
                .with_client_secret(secret.secret());
            config.application_type = None;
            manager.configure_client(config)?;
        }
        Ok(())
    }
}
