use codex_core::config::Config;
use codex_http_state::HttpStateContext;
use codex_login::AuthManager;
use codex_login::default_client::create_client;

use anyhow::Context;
use serde::de::DeserializeOwned;
use std::time::Duration;

const OAI_PRODUCT_SKU_HEADER: &str = "OAI-Product-Sku";
const CODEX_PRODUCT_SKU: &str = "codex";

pub(crate) async fn chatgpt_get_request_with_http_state<T: DeserializeOwned>(
    config: &Config,
    path: String,
    http_state: HttpStateContext,
) -> anyhow::Result<T> {
    chatgpt_get_request_with_timeout_and_http_state(
        config,
        path,
        /*timeout*/ None,
        Some(http_state),
    )
    .await
}

pub(crate) async fn chatgpt_get_request_with_timeout_and_http_state<T: DeserializeOwned>(
    config: &Config,
    path: String,
    timeout: Option<Duration>,
    http_state: Option<HttpStateContext>,
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

    // Make direct HTTP request to ChatGPT backend API with the token
    let client = create_client();
    let url = format!(
        "{}/{}",
        chatgpt_base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

    let auth_provider = codex_model_provider::with_native_integrity_state(
        codex_model_provider::auth_provider_from_auth(&auth),
        Some(&auth),
        http_state,
    );
    let mut request_headers = Default::default();
    auth_provider.add_auth_headers_for_url(&url, &mut request_headers);
    let mut request = client
        .get(&url)
        .headers(request_headers.clone())
        .header(OAI_PRODUCT_SKU_HEADER, CODEX_PRODUCT_SKU)
        .header("Content-Type", "application/json");

    if let Some(timeout) = timeout {
        request = request.timeout(timeout);
    }

    let response = request.send().await.context("Failed to send request")?;
    auth_provider.observe_response_headers(&url, &request_headers, response.headers());

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
