use codex_api::AuthProvider;
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

/// Auth provider for callers that already resolved the complete Authorization header value.
#[derive(Clone, Default)]
pub struct AuthorizationHeaderAuthProvider {
    pub authorization_header_value: Option<String>,
    pub account_id: Option<String>,
}

impl AuthorizationHeaderAuthProvider {
    pub fn new(authorization_header_value: Option<String>, account_id: Option<String>) -> Self {
        Self {
            authorization_header_value,
            account_id,
        }
    }

    pub fn for_test(authorization_header_value: Option<&str>, account_id: Option<&str>) -> Self {
        Self {
            authorization_header_value: authorization_header_value.map(str::to_string),
            account_id: account_id.map(str::to_string),
        }
    }
}

impl AuthProvider for AuthorizationHeaderAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        if let Some(authorization_header_value) = self.authorization_header_value.as_ref()
            && let Ok(header) = HeaderValue::from_str(authorization_header_value)
        {
            let _ = headers.insert(http::header::AUTHORIZATION, header);
        }
        if let Some(account_id) = self.account_id.as_ref()
            && let Ok(header) = HeaderValue::from_str(account_id)
        {
            let _ = headers.insert("ChatGPT-Account-ID", header);
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
    fn authorization_header_auth_provider_supports_non_bearer_authorization_headers() {
        let auth = AuthorizationHeaderAuthProvider::for_test(
            Some("AgentAssertion opaque-token"),
            Some("workspace-123"),
        );
        let mut headers = HeaderMap::new();

        auth.add_auth_headers(&mut headers);

        assert_eq!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("AgentAssertion opaque-token")
        );
        assert_eq!(
            headers
                .get("ChatGPT-Account-ID")
                .and_then(|value| value.to_str().ok()),
            Some("workspace-123")
        );
        assert_eq!(
            codex_api::auth_header_telemetry(&auth),
            codex_api::AuthHeaderTelemetry {
                attached: true,
                name: Some("authorization"),
            }
        );
    }
}
