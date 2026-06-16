use codex_core::config::Config;
use codex_login::AuthManager;
use codex_login::default_client::chatgpt_cloudflare_cookie_header;
use codex_login::default_client::create_client;
use codex_utils_plugins::plugin_service_routing::plugin_service_preview_enabled;
use codex_utils_plugins::plugin_service_routing::plugin_service_routing_cookie;

use anyhow::Context;
use serde::de::DeserializeOwned;
use std::time::Duration;

const OAI_PRODUCT_SKU_HEADER: &str = "OAI-Product-Sku";
const CODEX_PRODUCT_SKU: &str = "codex";

#[derive(Clone, Copy)]
enum ChatgptRequestRouting {
    Default,
    PluginService,
}

/// Make a GET request to the ChatGPT backend API.
pub(crate) async fn chatgpt_get_request<T: DeserializeOwned>(
    config: &Config,
    path: String,
) -> anyhow::Result<T> {
    chatgpt_get_request_with_timeout_inner(
        config,
        path,
        /*timeout*/ None,
        ChatgptRequestRouting::Default,
    )
    .await
}

pub(crate) async fn chatgpt_get_request_with_timeout<T: DeserializeOwned>(
    config: &Config,
    path: String,
    timeout: Option<Duration>,
) -> anyhow::Result<T> {
    chatgpt_get_request_with_timeout_inner(config, path, timeout, ChatgptRequestRouting::Default)
        .await
}

pub(crate) async fn chatgpt_get_plugin_service_request_with_timeout<T: DeserializeOwned>(
    config: &Config,
    path: String,
    timeout: Option<Duration>,
) -> anyhow::Result<T> {
    chatgpt_get_request_with_timeout_inner(
        config,
        path,
        timeout,
        ChatgptRequestRouting::PluginService,
    )
    .await
}

async fn chatgpt_get_request_with_timeout_inner<T: DeserializeOwned>(
    config: &Config,
    path: String,
    timeout: Option<Duration>,
    routing: ChatgptRequestRouting,
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

    let mut request = client
        .get(&url)
        .headers(codex_model_provider::auth_provider_from_auth(&auth).to_auth_headers())
        .header(OAI_PRODUCT_SKU_HEADER, CODEX_PRODUCT_SKU)
        .header("Content-Type", "application/json");

    if matches!(routing, ChatgptRequestRouting::PluginService) && plugin_service_preview_enabled() {
        let cloudflare_cookie = chatgpt_cloudflare_cookie_header(&url);
        let existing_cookie_headers = cloudflare_cookie.as_deref().into_iter().collect::<Vec<_>>();
        if let Some(routing_cookie) =
            plugin_service_routing_cookie(&existing_cookie_headers, /*preview_enabled*/ true)
        {
            request = request.header("Cookie", routing_cookie);
        }
    }

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
