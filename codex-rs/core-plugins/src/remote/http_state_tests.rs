use super::*;
use codex_http_state::HttpStateContext;
use codex_http_state::HttpStateSurface;
use pretty_assertions::assert_eq;
use reqwest::header::HeaderValue;
use tempfile::TempDir;

const REQUEST_URL: &str = "https://chatgpt.com/backend-api/ps/plugins/list";

#[test]
fn plugin_request_headers_attach_and_rotate_native_integrity_state() {
    let codex_home = TempDir::new().expect("create temp dir");
    let state = HttpStateContext::new(codex_home.path().to_path_buf(), HttpStateSurface::CodexCli);
    state
        .set_for_surface(HttpStateSurface::CodexCli, "state-before".to_string())
        .expect("seed state");
    let config = RemotePluginServiceConfig::new("https://chatgpt.com/backend-api".to_string())
        .with_http_state(state.clone());
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let (auth_provider, request_headers) = request_headers(&config, &auth, REQUEST_URL);
    assert_eq!(
        request_headers.get("X-OAI-IS"),
        Some(&HeaderValue::from_static("state-before"))
    );

    let mut response_headers = HeaderMap::new();
    response_headers.insert("X-OAI-IS-Update", HeaderValue::from_static("state-after"));
    observe_response_headers(
        &auth_provider,
        REQUEST_URL,
        &request_headers,
        &response_headers,
    );

    assert_eq!(
        state
            .get_for_surface(HttpStateSurface::CodexCli)
            .expect("read state"),
        Some("state-after".to_string())
    );
}
