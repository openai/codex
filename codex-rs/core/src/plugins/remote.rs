use crate::auth::CodexAuth;
use crate::config::Config;
use crate::default_client::build_reqwest_client;
use codex_protocol::protocol::Product;
use serde::Deserialize;
use std::time::Duration;
use url::Url;

const DEFAULT_REMOTE_MARKETPLACE_NAME: &str = "openai-curated";
const REMOTE_PLUGIN_FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const REMOTE_FEATURED_PLUGIN_FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const REMOTE_PLUGIN_MUTATION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RemotePluginStatusSummary {
    pub(crate) name: String,
    #[serde(default = "default_remote_marketplace_name")]
    pub(crate) marketplace_name: String,
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemotePluginMutationResponse {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RemotePluginMutationError {
    #[error("chatgpt authentication required for remote plugin mutation")]
    AuthRequired,

    #[error(
        "chatgpt authentication required for remote plugin mutation; api key auth is not supported"
    )]
    UnsupportedAuthMode,

    #[error("failed to read auth token for remote plugin mutation: {0}")]
    AuthToken(#[source] std::io::Error),

    #[error("invalid chatgpt base url for remote plugin mutation: {0}")]
    InvalidBaseUrl(#[source] url::ParseError),

    #[error("chatgpt base url cannot be used for plugin mutation")]
    InvalidBaseUrlPath,

    #[error("failed to send remote plugin mutation request to {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote plugin mutation failed with status {status} from {url}: {body}")]
    UnexpectedStatus {
        url: String,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("failed to parse remote plugin mutation response from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "remote plugin mutation returned unexpected plugin id: expected `{expected}`, got `{actual}`"
    )]
    UnexpectedPluginId { expected: String, actual: String },

    #[error(
        "remote plugin mutation returned unexpected enabled state for `{plugin_id}`: expected {expected_enabled}, got {actual_enabled}"
    )]
    UnexpectedEnabledState {
        plugin_id: String,
        expected_enabled: bool,
        actual_enabled: bool,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum RemotePluginFetchError {
    #[error("chatgpt authentication required to sync remote plugins")]
    AuthRequired,

    #[error(
        "chatgpt authentication required to sync remote plugins; api key auth is not supported"
    )]
    UnsupportedAuthMode,

    #[error("failed to read auth token for remote plugin sync: {0}")]
    AuthToken(#[source] std::io::Error),

    #[error("failed to send remote plugin sync request to {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote plugin sync request to {url} failed with status {status}: {body}")]
    UnexpectedStatus {
        url: String,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("failed to parse remote plugin sync response from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: serde_json::Error,
    },
}

pub(crate) async fn fetch_remote_plugin_status(
    config: &Config,
    auth: Option<&CodexAuth>,
) -> Result<Vec<RemotePluginStatusSummary>, RemotePluginFetchError> {
    let Some(auth) = auth else {
        return Err(RemotePluginFetchError::AuthRequired);
    };
    if !auth.is_chatgpt_auth() {
        return Err(RemotePluginFetchError::UnsupportedAuthMode);
    }

    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/plugins/list");
    let client = build_reqwest_client();
    let token = auth
        .get_token()
        .map_err(RemotePluginFetchError::AuthToken)?;
    let mut request = client
        .get(&url)
        .timeout(REMOTE_PLUGIN_FETCH_TIMEOUT)
        .bearer_auth(token);
    if let Some(account_id) = auth.get_account_id() {
        request = request.header("chatgpt-account-id", account_id);
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

pub async fn fetch_remote_featured_plugin_ids(
    config: &Config,
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

    if let Some(auth) = auth.filter(|auth| auth.is_chatgpt_auth()) {
        let token = auth
            .get_token()
            .map_err(RemotePluginFetchError::AuthToken)?;
        request = request.bearer_auth(token);
        if let Some(account_id) = auth.get_account_id() {
            request = request.header("chatgpt-account-id", account_id);
        }
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

pub(crate) async fn enable_remote_plugin(
    config: &Config,
    auth: Option<&CodexAuth>,
    plugin_id: &str,
) -> Result<(), RemotePluginMutationError> {
    post_remote_plugin_mutation(config, auth, plugin_id, "enable").await?;
    Ok(())
}

pub(crate) async fn uninstall_remote_plugin(
    config: &Config,
    auth: Option<&CodexAuth>,
    plugin_id: &str,
) -> Result<(), RemotePluginMutationError> {
    post_remote_plugin_mutation(config, auth, plugin_id, "uninstall").await?;
    Ok(())
}

fn ensure_chatgpt_auth(auth: Option<&CodexAuth>) -> Result<&CodexAuth, RemotePluginMutationError> {
    let Some(auth) = auth else {
        return Err(RemotePluginMutationError::AuthRequired);
    };
    if !auth.is_chatgpt_auth() {
        return Err(RemotePluginMutationError::UnsupportedAuthMode);
    }
    Ok(auth)
}

fn default_remote_marketplace_name() -> String {
    DEFAULT_REMOTE_MARKETPLACE_NAME.to_string()
}

async fn post_remote_plugin_mutation(
    config: &Config,
    auth: Option<&CodexAuth>,
    plugin_id: &str,
    action: &str,
) -> Result<RemotePluginMutationResponse, RemotePluginMutationError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let url = remote_plugin_mutation_url(config, plugin_id, action)?;
    let client = build_reqwest_client();
    let token = auth
        .get_token()
        .map_err(RemotePluginMutationError::AuthToken)?;
    let mut request = client
        .post(url.clone())
        .timeout(REMOTE_PLUGIN_MUTATION_TIMEOUT)
        .bearer_auth(token);
    if let Some(account_id) = auth.get_account_id() {
        request = request.header("chatgpt-account-id", account_id);
    }

    let response = request
        .send()
        .await
        .map_err(|source| RemotePluginMutationError::Request {
            url: url.clone(),
            source,
        })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(RemotePluginMutationError::UnexpectedStatus { url, status, body });
    }

    let parsed: RemotePluginMutationResponse =
        serde_json::from_str(&body).map_err(|source| RemotePluginMutationError::Decode {
            url: url.clone(),
            source,
        })?;
    let expected_enabled = action == "enable";
    if parsed.id != plugin_id {
        return Err(RemotePluginMutationError::UnexpectedPluginId {
            expected: plugin_id.to_string(),
            actual: parsed.id,
        });
    }
    if parsed.enabled != expected_enabled {
        return Err(RemotePluginMutationError::UnexpectedEnabledState {
            plugin_id: plugin_id.to_string(),
            expected_enabled,
            actual_enabled: parsed.enabled,
        });
    }

    Ok(parsed)
}

fn remote_plugin_mutation_url(
    config: &Config,
    plugin_id: &str,
    action: &str,
) -> Result<String, RemotePluginMutationError> {
    let mut url = Url::parse(config.chatgpt_base_url.trim_end_matches('/'))
        .map_err(RemotePluginMutationError::InvalidBaseUrl)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| RemotePluginMutationError::InvalidBaseUrlPath)?;
        segments.pop_if_empty();
        segments.push("plugins");
        segments.push(plugin_id);
        segments.push(action);
    }
    Ok(url.to_string())
}
