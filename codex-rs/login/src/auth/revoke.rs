use serde::Serialize;

use codex_app_server_protocol::AuthMode as ApiAuthMode;
use codex_client::CodexHttpClient;

use super::manager::CLIENT_ID;
use super::manager::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
use super::storage::AuthDotJson;
use super::util::try_parse_error_message;
use crate::default_client::create_client;
use crate::token_data::TokenData;

const REVOKE_TOKEN_URL: &str = "https://auth.openai.com/oauth/revoke";
pub const REVOKE_TOKEN_URL_OVERRIDE_ENV_VAR: &str = "CODEX_REVOKE_TOKEN_URL_OVERRIDE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogoutResult {
    pub removed: bool,
    pub revoke_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RevokeTokenKind {
    Access,
    Refresh,
}

impl RevokeTokenKind {
    fn hint(self) -> &'static str {
        match self {
            Self::Access => "access_token",
            Self::Refresh => "refresh_token",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Access => "access token",
            Self::Refresh => "refresh token",
        }
    }

    fn client_id(self) -> Option<&'static str> {
        match self {
            Self::Access => None,
            Self::Refresh => Some(CLIENT_ID),
        }
    }
}

#[derive(Serialize)]
struct RevokeTokenRequest<'a> {
    token: &'a str,
    token_type_hint: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_id: Option<&'static str>,
}

pub(super) async fn revoke_auth_tokens(
    auth_dot_json: Option<&AuthDotJson>,
) -> Result<(), std::io::Error> {
    let Some(tokens) = auth_dot_json.and_then(managed_chatgpt_tokens) else {
        return Ok(());
    };

    let client = create_client();
    if !tokens.refresh_token.is_empty() {
        revoke_oauth_token(
            &client,
            tokens.refresh_token.as_str(),
            RevokeTokenKind::Refresh,
        )
        .await
    } else if !tokens.access_token.is_empty() {
        revoke_oauth_token(
            &client,
            tokens.access_token.as_str(),
            RevokeTokenKind::Access,
        )
        .await
    } else {
        Ok(())
    }
}

fn managed_chatgpt_tokens(auth_dot_json: &AuthDotJson) -> Option<&TokenData> {
    if resolved_auth_mode(auth_dot_json) == ApiAuthMode::Chatgpt {
        auth_dot_json.tokens.as_ref()
    } else {
        None
    }
}

fn resolved_auth_mode(auth_dot_json: &AuthDotJson) -> ApiAuthMode {
    if let Some(mode) = auth_dot_json.auth_mode {
        return mode;
    }
    if auth_dot_json.openai_api_key.is_some() {
        return ApiAuthMode::ApiKey;
    }
    ApiAuthMode::Chatgpt
}

async fn revoke_oauth_token(
    client: &CodexHttpClient,
    token: &str,
    kind: RevokeTokenKind,
) -> Result<(), std::io::Error> {
    let request = RevokeTokenRequest {
        token,
        token_type_hint: kind.hint(),
        client_id: kind.client_id(),
    };

    let response = client
        .post(revoke_token_endpoint().as_str())
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(std::io::Error::other)?;

    let status = response.status();
    if status.is_success() {
        return Ok(());
    }

    let body = response.text().await.unwrap_or_default();
    let message = try_parse_error_message(&body);
    Err(std::io::Error::other(format!(
        "failed to revoke {}: {}: {}",
        kind.label(),
        status,
        message
    )))
}

fn revoke_token_endpoint() -> String {
    if let Ok(endpoint) = std::env::var(REVOKE_TOKEN_URL_OVERRIDE_ENV_VAR) {
        return endpoint;
    }

    if let Ok(refresh_endpoint) = std::env::var(REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR)
        && let Some(endpoint) = derive_revoke_token_endpoint(&refresh_endpoint)
    {
        return endpoint;
    }

    REVOKE_TOKEN_URL.to_string()
}

fn derive_revoke_token_endpoint(refresh_endpoint: &str) -> Option<String> {
    let mut url = url::Url::parse(refresh_endpoint).ok()?;
    url.set_path("/oauth/revoke");
    url.set_query(None);
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_revoke_url_from_refresh_token_override() {
        assert_eq!(
            derive_revoke_token_endpoint("http://127.0.0.1:1234/oauth/token?unified=true"),
            Some("http://127.0.0.1:1234/oauth/revoke".to_string())
        );
    }
}
