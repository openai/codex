use super::RemotePluginServiceConfig;
use codex_api::SharedAuthProvider;
use codex_login::CodexAuth;
use reqwest::header::HeaderMap;

pub(crate) fn request_headers(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    url: &str,
) -> (SharedAuthProvider, HeaderMap) {
    let auth_provider = codex_model_provider::with_native_integrity_state(
        codex_model_provider::auth_provider_from_auth(auth),
        Some(auth),
        config.http_state.clone(),
    );
    let mut request_headers = HeaderMap::new();
    auth_provider.add_auth_headers_for_url(url, &mut request_headers);
    (auth_provider, request_headers)
}

pub(crate) fn observe_response_headers(
    auth_provider: &SharedAuthProvider,
    url: &str,
    request_headers: &HeaderMap,
    response_headers: &HeaderMap,
) {
    auth_provider.observe_response_headers(url, request_headers, response_headers);
}

#[cfg(test)]
#[path = "http_state_tests.rs"]
mod tests;
