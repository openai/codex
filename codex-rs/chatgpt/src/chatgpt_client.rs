use codex_core::config::Config;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::default_client::RESIDENCY_HEADER_NAME;
use codex_login::default_client::create_client;
use codex_login::default_client::create_client_with_chatgpt_cookies;
use codex_login::default_client::default_headers;

use anyhow::Context;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;

const OAI_PRODUCT_SKU_HEADER: &str = "OAI-Product-Sku";
const CODEX_PRODUCT_SKU: &str = "codex";

struct CachedChatGptClient {
    factory: HttpClientFactory,
    residency: Option<Vec<u8>>,
    client: HttpClient,
}

static PSP_CHATGPT_CLIENT: LazyLock<Mutex<Option<CachedChatGptClient>>> =
    LazyLock::new(|| Mutex::new(None));

/// Reuse the default client while retaining its configured ChatGPT cookies.
fn psp_chatgpt_client(config: &Config) -> HttpClient {
    let factory = config.http_client_factory();
    let residency = default_headers()
        .get(RESIDENCY_HEADER_NAME)
        .map(|value| value.as_bytes().to_vec());
    let mut cached = PSP_CHATGPT_CLIENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(cached_client) = cached.as_ref()
        && cached_client.factory == factory
        && cached_client.residency == residency
    {
        return cached_client.client.clone();
    }

    let client = create_client_with_chatgpt_cookies(&factory);
    *cached = Some(CachedChatGptClient {
        factory,
        residency,
        client: client.clone(),
    });
    client
}

/// Make a GET request to the ChatGPT backend API.
pub(crate) async fn chatgpt_get_request<T: DeserializeOwned>(
    config: &Config,
    path: String,
) -> anyhow::Result<T> {
    chatgpt_get_request_with_timeout(config, path, /*timeout*/ None).await
}

pub(crate) async fn chatgpt_get_request_with_timeout<T: DeserializeOwned>(
    config: &Config,
    path: String,
    timeout: Option<Duration>,
) -> anyhow::Result<T> {
    let chatgpt_base_url = &config.chatgpt_base_url;
    let auth_manager =
        AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ false).await;
    let auth = auth_manager
        .auth()
        .await
        .ok_or_else(|| anyhow::anyhow!("ChatGPT auth not available"))?;
    anyhow::ensure!(
        auth.uses_codex_backend(),
        "ChatGPT backend requests require Codex backend auth"
    );
    anyhow::ensure!(
        auth.get_account_id().is_some(),
        "ChatGPT account ID not available, please re-run `codex login`"
    );

    let url = format!(
        "{}/{}",
        chatgpt_base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

    let client = if config.psp {
        psp_chatgpt_client(config)
    } else {
        create_client()
    };
    let mut request = client
        .get(&url)
        .headers(default_headers())
        .headers(codex_model_provider::auth_provider_from_auth(&auth).to_auth_headers())
        .header(OAI_PRODUCT_SKU_HEADER, CODEX_PRODUCT_SKU)
        .header("Content-Type", "application/json");
    if let Some(timeout) = timeout {
        request = request.timeout(timeout);
    }
    let response = request.send().await.context("Failed to send request")?;

    if response.status().is_success() {
        let result: T = response
            .json()
            .await
            .context("Failed to parse JSON response")?;
        Ok(result)
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Request failed with status {status}: {body}")
    }
}

/// Make a POST request to the ChatGPT backend API with an already-captured auth identity.
///
/// Callers that bind other state to the auth snapshot should pass that same snapshot here rather
/// than reacquiring auth while the request is in flight.
pub(crate) async fn chatgpt_post_request_with_timeout<
    TResponse: DeserializeOwned,
    TRequest: Serialize + ?Sized,
>(
    config: &Config,
    auth: &CodexAuth,
    path: String,
    body: &TRequest,
    timeout: Duration,
    product_sku: &str,
) -> anyhow::Result<TResponse> {
    anyhow::ensure!(
        auth.uses_codex_backend(),
        "ChatGPT backend requests require Codex backend auth"
    );
    anyhow::ensure!(
        auth.get_account_id().is_some(),
        "ChatGPT account ID not available, please re-run codex login"
    );

    let url = format!(
        "{}/{}",
        config.chatgpt_base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let client = if config.psp {
        psp_chatgpt_client(config)
    } else {
        create_client()
    };
    let response = client
        .post(&url)
        .headers(default_headers())
        .headers(codex_model_provider::auth_provider_from_auth(auth).to_auth_headers())
        .header(OAI_PRODUCT_SKU_HEADER, product_sku)
        .header("Content-Type", "application/json")
        .timeout(timeout)
        .json(body)
        .send()
        .await
        .context("Failed to send request")?;

    if response.status().is_success() {
        response
            .json()
            .await
            .context("Failed to parse JSON response")
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Request failed with status {status}: {body}")
    }
}
