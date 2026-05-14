use codex_backend_client::Client as BackendClient;
use codex_backend_client::ResolvedCredentialRoute;
use codex_login::CodexAuth;
use tracing::debug;
use tracing::warn;

pub(crate) async fn load_for_session(
    chatgpt_base_url: &str,
    auth: Option<&CodexAuth>,
) -> Vec<ResolvedCredentialRoute> {
    let Some(auth) = auth.filter(|auth| auth.uses_codex_backend()) else {
        return Vec::new();
    };

    let client = match BackendClient::from_auth(chatgpt_base_url.to_string(), auth) {
        Ok(client) => client,
        Err(err) => {
            warn!(error = %err, "failed to initialize credentialed routes client");
            return Vec::new();
        }
    };

    match client.list_credential_routes().await {
        Ok(response) => {
            debug!(
                credentialed_routes = response.routes.len(),
                "loaded credentialed routes for session"
            );
            response.routes
        }
        Err(err) => {
            warn!(error = %err, "failed to load credentialed routes for session");
            Vec::new()
        }
    }
}
