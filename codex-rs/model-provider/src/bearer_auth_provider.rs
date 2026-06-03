use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_client::INTEGRITY_STATE_HEADER_NAME;
use codex_client::INTEGRITY_STATE_UPDATE_HEADER_NAME;
use codex_client::is_allowed_chatgpt_request_url;
use codex_http_state::HttpStateContext;
use codex_http_state::HttpStateSurface;
use http::HeaderMap;
use http::HeaderValue;

const OAI_CLIENT_TYPE_HEADER_NAME: &str = "oai-client-type";

/// Bearer-token auth provider for OpenAI-compatible model-provider requests.
#[derive(Clone, Default)]
pub struct BearerAuthProvider {
    pub token: Option<String>,
    pub account_id: Option<String>,
    pub is_fedramp_account: bool,
}

impl BearerAuthProvider {
    pub fn new(token: String) -> Self {
        Self {
            token: Some(token),
            account_id: None,
            is_fedramp_account: false,
        }
    }

    pub fn for_test(token: Option<&str>, account_id: Option<&str>) -> Self {
        Self {
            token: token.map(str::to_string),
            account_id: account_id.map(str::to_string),
            is_fedramp_account: false,
        }
    }
}

impl AuthProvider for BearerAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        if let Some(token) = self.token.as_ref()
            && let Ok(header) = HeaderValue::from_str(&format!("Bearer {token}"))
        {
            let _ = headers.insert(http::header::AUTHORIZATION, header);
        }
        if let Some(account_id) = self.account_id.as_ref()
            && let Ok(header) = HeaderValue::from_str(account_id)
        {
            let _ = headers.insert("ChatGPT-Account-ID", header);
        }
        if self.is_fedramp_account {
            let _ = headers.insert("X-OpenAI-Fedramp", HeaderValue::from_static("true"));
        }
    }
}

#[derive(Clone)]
pub(crate) struct NativeIntegrityAuthProvider {
    auth: SharedAuthProvider,
    state: HttpStateContext,
    surface: HttpStateSurface,
}

impl NativeIntegrityAuthProvider {
    pub(crate) fn new(auth: SharedAuthProvider, state: HttpStateContext) -> Self {
        let surface = state.surface();
        Self {
            auth,
            state,
            surface,
        }
    }
}

impl AuthProvider for NativeIntegrityAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        self.auth.add_auth_headers(headers);
    }

    fn add_auth_headers_for_url(&self, request_url: &str, headers: &mut HeaderMap) {
        self.auth.add_auth_headers_for_url(request_url, headers);
        if !is_allowed_chatgpt_request_url(request_url) {
            return;
        }

        if let Ok(header) = HeaderValue::from_str(self.surface.as_str()) {
            let _ = headers.insert(OAI_CLIENT_TYPE_HEADER_NAME, header);
        }
        match self.state.get_for_surface(self.surface) {
            Ok(Some(state)) => {
                if let Ok(header) = HeaderValue::from_str(&state) {
                    let _ = headers.insert(INTEGRITY_STATE_HEADER_NAME, header);
                }
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!("failed to load native integrity state: {error}");
            }
        }
    }

    fn observe_response_headers(
        &self,
        request_url: &str,
        request_headers: &HeaderMap,
        response_headers: &HeaderMap,
    ) {
        self.auth
            .observe_response_headers(request_url, request_headers, response_headers);
        if !is_allowed_chatgpt_request_url(request_url) {
            return;
        }

        let Some(next_state) = response_headers
            .get(INTEGRITY_STATE_UPDATE_HEADER_NAME)
            .and_then(|value| value.to_str().ok())
        else {
            return;
        };

        let expected_state = request_headers
            .get(INTEGRITY_STATE_HEADER_NAME)
            .and_then(|value| value.to_str().ok());
        let result = match expected_state {
            Some(expected_state) => self.state.compare_and_set_for_surface(
                self.surface,
                expected_state,
                next_state.to_string(),
            ),
            None => self
                .state
                .set_for_surface(self.surface, next_state.to_string())
                .map(|()| true),
        };
        if let Err(error) = result {
            tracing::warn!("failed to rotate native integrity state: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_http_state::HttpStateStore;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[test]
    fn bearer_auth_provider_reports_when_auth_header_will_attach() {
        let auth = BearerAuthProvider {
            token: Some("access-token".to_string()),
            account_id: None,
            is_fedramp_account: false,
        };

        assert_eq!(
            codex_api::auth_header_telemetry(&auth),
            codex_api::AuthHeaderTelemetry {
                attached: true,
                name: Some("authorization"),
            }
        );
    }

    #[test]
    fn bearer_auth_provider_adds_auth_headers() {
        let auth = BearerAuthProvider::for_test(Some("access-token"), Some("workspace-123"));
        let mut headers = HeaderMap::new();

        auth.add_auth_headers(&mut headers);

        assert_eq!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer access-token")
        );
        assert_eq!(
            headers
                .get("ChatGPT-Account-ID")
                .and_then(|value| value.to_str().ok()),
            Some("workspace-123")
        );
    }

    #[test]
    fn bearer_auth_provider_adds_fedramp_routing_header_for_fedramp_accounts() {
        let auth = BearerAuthProvider {
            token: Some("access-token".to_string()),
            account_id: Some("workspace-123".to_string()),
            is_fedramp_account: true,
        };
        let mut headers = HeaderMap::new();

        auth.add_auth_headers(&mut headers);

        assert_eq!(
            headers
                .get("X-OpenAI-Fedramp")
                .and_then(|value| value.to_str().ok()),
            Some("true")
        );
    }

    #[test]
    fn native_integrity_provider_scopes_and_rotates_surface_state() {
        let codex_home = TempDir::new().expect("tempdir");
        let context = HttpStateContext::new(
            codex_home.path().to_path_buf(),
            HttpStateSurface::CodexDesktop,
        );
        let store = HttpStateStore::new(codex_home.path().to_path_buf());
        store
            .set(
                HttpStateSurface::CodexDesktop,
                "ois1.initial.nonce.ciphertext".to_string(),
            )
            .expect("state should store");
        let provider = NativeIntegrityAuthProvider::new(
            Arc::new(BearerAuthProvider::new("access-token".to_string())),
            context.clone(),
        );
        context.set_surface(HttpStateSurface::CodexCli);
        store
            .set(
                HttpStateSurface::CodexCli,
                "ois1.cli.nonce.ciphertext".to_string(),
            )
            .expect("CLI state should store");

        let mut request_headers = HeaderMap::new();
        provider.add_auth_headers_for_url(
            "https://chatgpt.com/backend-api/codex/responses",
            &mut request_headers,
        );
        assert_eq!(
            request_headers
                .get(INTEGRITY_STATE_HEADER_NAME)
                .and_then(|value| value.to_str().ok()),
            Some("ois1.initial.nonce.ciphertext")
        );
        assert_eq!(
            request_headers
                .get(OAI_CLIENT_TYPE_HEADER_NAME)
                .and_then(|value| value.to_str().ok()),
            Some("codex_desktop")
        );

        let mut response_headers = HeaderMap::new();
        response_headers.insert(
            INTEGRITY_STATE_UPDATE_HEADER_NAME,
            HeaderValue::from_static("ois1.rotated.nonce.ciphertext"),
        );
        provider.observe_response_headers(
            "https://chatgpt.com/backend-api/codex/responses",
            &request_headers,
            &response_headers,
        );
        assert_eq!(
            store
                .get(HttpStateSurface::CodexDesktop)
                .expect("state should load")
                .expect("state should exist"),
            "ois1.rotated.nonce.ciphertext"
        );
        assert_eq!(
            store
                .get(HttpStateSurface::CodexCli)
                .expect("CLI state should load")
                .expect("CLI state should exist"),
            "ois1.cli.nonce.ciphertext"
        );

        let mut external_headers = HeaderMap::new();
        provider.add_auth_headers_for_url(
            "https://example.com/backend-api/codex/responses",
            &mut external_headers,
        );
        assert!(!external_headers.contains_key(INTEGRITY_STATE_HEADER_NAME));
        assert!(!external_headers.contains_key(OAI_CLIENT_TYPE_HEADER_NAME));
    }

    #[test]
    fn native_integrity_provider_seeds_empty_surface_from_response() {
        let codex_home = TempDir::new().expect("tempdir");
        let context = HttpStateContext::new(
            codex_home.path().to_path_buf(),
            HttpStateSurface::CodexDesktop,
        );
        let store = HttpStateStore::new(codex_home.path().to_path_buf());
        let provider = NativeIntegrityAuthProvider::new(
            Arc::new(BearerAuthProvider::new("access-token".to_string())),
            context,
        );
        let mut response_headers = HeaderMap::new();
        response_headers.insert(
            INTEGRITY_STATE_UPDATE_HEADER_NAME,
            HeaderValue::from_static("ois1.seeded.nonce.ciphertext"),
        );

        provider.observe_response_headers(
            "https://chatgpt.com/backend-api/codex/responses",
            &HeaderMap::new(),
            &response_headers,
        );

        assert_eq!(
            store
                .get(HttpStateSurface::CodexDesktop)
                .expect("state should load")
                .expect("state should exist"),
            "ois1.seeded.nonce.ciphertext"
        );
    }
}
