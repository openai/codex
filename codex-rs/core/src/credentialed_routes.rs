use anyhow::Result;
use codex_backend_client::Client as BackendClient;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_network_proxy::CredentialedRoute;
use codex_network_proxy::CredentialedRouteProxyHeader;
use codex_network_proxy::CredentialedRoutesConfig;
use codex_network_proxy::CredentialedRoutesSource;
use http::HeaderMap;
use std::sync::Arc;
use tracing::debug;
use tracing::warn;

pub(crate) async fn load_for_session(
    chatgpt_base_url: &str,
    auth: Option<&CodexAuth>,
) -> CredentialedRoutesConfig {
    match fetch(chatgpt_base_url, auth).await {
        Ok(credentialed_routes) => credentialed_routes,
        Err(err) => {
            warn!(error = %err, "failed to load credentialed routes for session");
            CredentialedRoutesConfig::default()
        }
    }
}

async fn fetch(
    chatgpt_base_url: &str,
    auth: Option<&CodexAuth>,
) -> Result<CredentialedRoutesConfig> {
    let Some(auth) = auth.filter(|auth| auth.uses_codex_backend()) else {
        return Ok(CredentialedRoutesConfig::default());
    };

    let client = BackendClient::from_auth(chatgpt_base_url.to_string(), auth)?;
    let response = client.list_credential_routes().await?;
    debug!(
        credentialed_routes = response.routes.len(),
        "loaded credentialed routes for session"
    );
    Ok(CredentialedRoutesConfig {
        routes: response
            .routes
            .into_iter()
            .map(|route| CredentialedRoute {
                connector_id: route.connector_id,
                link_id: route.link_id,
                base_url: route.base_url,
            })
            .collect(),
        proxy_headers: credentialed_route_proxy_headers(client.credential_routes_proxy_headers()),
        proxy_url: Some(client.credential_routes_proxy_url()),
    })
}

pub(crate) fn source(
    chatgpt_base_url: String,
    auth_manager: Arc<AuthManager>,
) -> Arc<dyn CredentialedRoutesSource> {
    Arc::new(move || {
        let chatgpt_base_url = chatgpt_base_url.clone();
        let auth_manager = Arc::clone(&auth_manager);
        async move {
            let auth = auth_manager.auth().await;
            fetch(&chatgpt_base_url, auth.as_ref()).await
        }
    })
}

fn credentialed_route_proxy_headers(headers: HeaderMap) -> Vec<CredentialedRouteProxyHeader> {
    headers
        .iter()
        .map(|(name, value)| CredentialedRouteProxyHeader {
            name: name.clone(),
            value: value.clone(),
        })
        .collect()
}
