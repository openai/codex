use codex_api::AuthProvider;
use codex_client::is_allowed_chatgpt_request_url;
use codex_login::CodexAuth;
use codex_login::integrity_state::INTEGRITY_STATE_HEADER_NAME;
use codex_login::integrity_state::INTEGRITY_STATE_UPDATE_HEADER_NAME;
use http::HeaderMap;
use http::HeaderValue;

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
pub(crate) struct ChatgptBearerAuthProvider {
    bearer: BearerAuthProvider,
    auth: CodexAuth,
}

impl ChatgptBearerAuthProvider {
    pub(crate) fn new(bearer: BearerAuthProvider, auth: CodexAuth) -> Self {
        Self { bearer, auth }
    }
}

impl AuthProvider for ChatgptBearerAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        self.bearer.add_auth_headers(headers);
    }

    fn add_auth_headers_for_url(&self, request_url: &str, headers: &mut HeaderMap) {
        self.bearer.add_auth_headers(headers);
        if !is_allowed_chatgpt_request_url(request_url) {
            return;
        }
        if let Some(integrity_state) = self.auth.integrity_state()
            && let Ok(header) = HeaderValue::from_str(&integrity_state)
        {
            let _ = headers.insert(INTEGRITY_STATE_HEADER_NAME, header);
        }
    }

    fn observe_response_headers(&self, request_url: &str, headers: &HeaderMap) {
        if !is_allowed_chatgpt_request_url(request_url) {
            return;
        }
        let Some(integrity_state) = headers
            .get(INTEGRITY_STATE_UPDATE_HEADER_NAME)
            .and_then(|value| value.to_str().ok())
        else {
            return;
        };

        if let Err(err) = self.auth.update_integrity_state(integrity_state) {
            tracing::warn!("failed to persist ChatGPT integrity state rotation: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
    fn bearer_auth_provider_rotates_integrity_state_for_chatgpt_hosts() {
        let auth = CodexAuth::create_dummy_chatgpt_auth_tokens_for_testing();
        assert!(
            auth.update_integrity_state("ois1.initial.nonce.ciphertext")
                .expect("initial integrity state should persist")
        );
        let provider = ChatgptBearerAuthProvider::new(
            BearerAuthProvider {
                token: Some("access-token".to_string()),
                account_id: Some("workspace-123".to_string()),
                is_fedramp_account: false,
            },
            auth.clone(),
        );

        let mut generic_headers = HeaderMap::new();
        provider.add_auth_headers(&mut generic_headers);
        assert!(!generic_headers.contains_key(INTEGRITY_STATE_HEADER_NAME));

        let mut request_headers = HeaderMap::new();
        provider.add_auth_headers_for_url(
            "https://chatgpt.com/backend-api/codex/tasks",
            &mut request_headers,
        );
        assert_eq!(
            request_headers
                .get(INTEGRITY_STATE_HEADER_NAME)
                .and_then(|value| value.to_str().ok()),
            Some("ois1.initial.nonce.ciphertext")
        );
        let mut external_headers = HeaderMap::new();
        provider.add_auth_headers_for_url("https://example.com/backend-api", &mut external_headers);
        assert!(!external_headers.contains_key(INTEGRITY_STATE_HEADER_NAME));

        let mut response_headers = HeaderMap::new();
        response_headers.insert(
            INTEGRITY_STATE_UPDATE_HEADER_NAME,
            HeaderValue::from_static("ois1.rotated.nonce.ciphertext"),
        );
        provider.observe_response_headers(
            "https://chatgpt.com/backend-api/codex/tasks",
            &response_headers,
        );
        assert_eq!(
            auth.integrity_state().as_deref(),
            Some("ois1.rotated.nonce.ciphertext")
        );

        response_headers.insert(
            INTEGRITY_STATE_UPDATE_HEADER_NAME,
            HeaderValue::from_static("ois1.external.nonce.ciphertext"),
        );
        provider.observe_response_headers("https://example.com/backend-api", &response_headers);
        assert_eq!(
            auth.integrity_state().as_deref(),
            Some("ois1.rotated.nonce.ciphertext")
        );
    }
}
