use codex_api::AuthProvider;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use http::HeaderMap;
use http::HeaderValue;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct CoreAuthProvider {
    pub token: Option<String>,
    pub account_id: Option<String>,
    pub is_fedramp_account: bool,
}

impl CoreAuthProvider {
    pub fn for_test(token: Option<&str>, account_id: Option<&str>) -> Self {
        Self {
            token: token.map(str::to_string),
            account_id: account_id.map(str::to_string),
            is_fedramp_account: false,
        }
    }
}

impl AuthProvider for CoreAuthProvider {
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

    fn auth_header_attached(&self) -> bool {
        self.token
            .as_ref()
            .is_some_and(|token| HeaderValue::from_str(&format!("Bearer {token}")).is_ok())
    }

    fn auth_header_name(&self) -> Option<&'static str> {
        self.auth_header_attached().then_some("authorization")
    }
}

fn core_auth_provider_from_auth(
    auth: Option<CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<CoreAuthProvider> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(CoreAuthProvider {
            token: Some(api_key),
            account_id: None,
            is_fedramp_account: false,
        });
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(CoreAuthProvider {
            token: Some(token),
            account_id: None,
            is_fedramp_account: false,
        });
    }

    if let Some(auth) = auth {
        let token = auth.get_token()?;
        Ok(CoreAuthProvider {
            token: Some(token),
            account_id: auth.get_account_id(),
            is_fedramp_account: auth.is_fedramp_account(),
        })
    } else {
        Ok(CoreAuthProvider {
            token: None,
            account_id: None,
            is_fedramp_account: false,
        })
    }
}

pub fn resolve_provider_auth(
    auth: Option<CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<Arc<dyn AuthProvider>> {
    let api_auth = core_auth_provider_from_auth(auth, provider)?;
    Ok(Arc::new(api_auth))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn core_auth_provider_reports_when_auth_header_will_attach() {
        let auth = CoreAuthProvider {
            token: Some("access-token".to_string()),
            account_id: None,
            is_fedramp_account: false,
        };

        assert!(auth.auth_header_attached());
        assert_eq!(auth.auth_header_name(), Some("authorization"));
    }

    #[test]
    fn core_auth_provider_adds_auth_headers() {
        let auth = CoreAuthProvider::for_test(Some("access-token"), Some("workspace-123"));
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
    fn core_auth_provider_adds_fedramp_routing_header_for_fedramp_accounts() {
        let auth = CoreAuthProvider {
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
}
