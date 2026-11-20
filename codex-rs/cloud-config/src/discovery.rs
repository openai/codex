//! Discovers OAuth endpoints for the config server through RMCP.
//! Explicit OAuth settings take precedence, and discovered settings stay in memory.

use codex_config::ConfigServer;
use codex_exec_server::RouteAwareHttpClient;
use codex_login::AuthRouteConfig;
use codex_login::OAuthSessionConfig;
use codex_rmcp_client::discover_authorization_metadata;
use serde::Deserialize;
use std::io;
use std::sync::Arc;
use std::time::Duration;

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn resolve_config_server(
    config: &ConfigServer,
    routes: &AuthRouteConfig,
) -> io::Result<ConfigServer> {
    config.validated_base_url().map_err(io::Error::other)?;
    let mut resolved = config.clone();

    if config.oauth.is_none()
        && let Some(oauth) = discover_oauth(&config.base_url, routes).await?
    {
        resolved.oauth = Some(Box::new(oauth));
        resolved.validated_base_url().map_err(io::Error::other)?;
    }

    Ok(resolved)
}

async fn discover_oauth(
    issuer: &str,
    routes: &AuthRouteConfig,
) -> io::Result<Option<OAuthSessionConfig>> {
    let client = Arc::new(RouteAwareHttpClient::new(
        routes.bootstrap_http_client_factory().clone(),
    ));
    let metadata = tokio::time::timeout(
        DISCOVERY_TIMEOUT,
        discover_authorization_metadata(issuer, client),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "OAuth discovery timed out"))?
    .map_err(|_| io::Error::other("OAuth discovery failed"))?;
    let Some(metadata) = metadata else {
        return Ok(None);
    };

    let device_authorization_url = metadata
        .additional_fields
        .get("device_authorization_endpoint")
        .map(Option::<String>::deserialize)
        .transpose()
        .map_err(|_| io::Error::other("OAuth discovery device endpoint is invalid"))?
        .flatten();

    Ok(Some(OAuthSessionConfig {
        authorization_url: metadata.authorization_endpoint,
        token_url: metadata.token_endpoint,
        device_authorization_url,
        client_id: String::from("codex"),
        resource: None,
        scopes: Vec::new(),
        redirect_port: None,
        credentials_store: Default::default(),
    }))
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
