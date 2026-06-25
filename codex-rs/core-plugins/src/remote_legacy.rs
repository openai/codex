use crate::remote::RemotePluginServiceConfig;
use codex_login::CodexAuth;
use codex_login::default_client::build_reqwest_client;
use codex_protocol::protocol::Product;
use std::time::Duration;

const REMOTE_FEATURED_PLUGIN_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum RemotePluginFetchError {
    #[error("failed to send remote featured plugin request to {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote featured plugin request to {url} failed with status {status}: {body}")]
    UnexpectedStatus {
        url: String,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("failed to parse remote featured plugin response from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: serde_json::Error,
    },
}

pub async fn fetch_remote_featured_plugin_ids(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    product: Option<Product>,
) -> Result<Vec<String>, RemotePluginFetchError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/plugins/featured");
    let client = build_reqwest_client();
    let mut request = client
        .get(&url)
        .query(&[(
            "platform",
            product.unwrap_or(Product::Codex).to_app_platform(),
        )])
        .timeout(REMOTE_FEATURED_PLUGIN_FETCH_TIMEOUT);

    if let Some(auth) = auth.filter(|auth| auth.uses_codex_backend()) {
        request =
            request.headers(codex_model_provider::auth_provider_from_auth(auth).to_auth_headers());
    }

    let response = request
        .send()
        .await
        .map_err(|source| RemotePluginFetchError::Request {
            url: url.clone(),
            source,
        })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(RemotePluginFetchError::UnexpectedStatus { url, status, body });
    }

    serde_json::from_str(&body).map_err(|source| RemotePluginFetchError::Decode {
        url: url.clone(),
        source,
    })
}
