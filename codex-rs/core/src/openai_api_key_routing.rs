use codex_api::Provider as ApiProvider;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::Result as CoreResult;

pub(crate) fn resolve_api_provider(
    provider: &ModelProviderInfo,
    auth: Option<&CodexAuth>,
) -> CoreResult<ApiProvider> {
    let auth_mode = auth.map(CodexAuth::auth_mode);
    let mut api_provider = provider.to_api_provider(auth_mode)?;

    if provider.base_url.is_some() || !provider.is_openai() {
        return Ok(api_provider);
    }

    if let Some(CodexAuth::ApiKey(api_key_auth)) = auth
        && let Some(api_base_url) = api_key_auth.api_base_url()
    {
        api_provider.base_url = api_base_url.to_string();
    }

    Ok(api_provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_login::OPENAI_GOV_API_BASE_URL;
    use codex_model_provider_info::WireApi;
    use pretty_assertions::assert_eq;

    const COMMERCIAL_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

    #[test]
    fn resolve_api_provider_uses_saved_gov_override_for_openai_api_key_auth() {
        let provider = ModelProviderInfo::create_openai_provider(None);
        let auth = CodexAuth::from_api_key_and_base_url(
            "sk-test",
            Some(OPENAI_GOV_API_BASE_URL.to_string()),
        );

        let resolved =
            resolve_api_provider(&provider, Some(&auth)).expect("expected provider resolution");

        assert_eq!(resolved.base_url, OPENAI_GOV_API_BASE_URL);
    }

    #[test]
    fn resolve_api_provider_keeps_default_base_url_without_saved_override() {
        let provider = ModelProviderInfo::create_openai_provider(None);
        let auth = CodexAuth::from_api_key("sk-test");

        let resolved =
            resolve_api_provider(&provider, Some(&auth)).expect("expected provider resolution");

        assert_eq!(resolved.base_url, COMMERCIAL_OPENAI_BASE_URL);
    }

    #[test]
    fn resolve_api_provider_preserves_explicit_provider_base_url() {
        let provider = ModelProviderInfo {
            name: "OpenAI".to_string(),
            base_url: Some("https://example.test/v1".to_string()),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            auth: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            websocket_connect_timeout_ms: None,
            requires_openai_auth: true,
            supports_websockets: true,
        };
        let auth = CodexAuth::from_api_key_and_base_url(
            "sk-test",
            Some(OPENAI_GOV_API_BASE_URL.to_string()),
        );

        let resolved =
            resolve_api_provider(&provider, Some(&auth)).expect("expected provider resolution");

        assert_eq!(resolved.base_url, "https://example.test/v1");
    }
}
