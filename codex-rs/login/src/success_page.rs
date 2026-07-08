use base64::Engine;
use serde_json::Value as JsonValue;
use url::Url;

use crate::server::DEFAULT_ISSUER;

pub const CODEX_OPEN_APP_URL: &str = "https://chatgpt.com/codex/open-app";

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub enum LoginSuccessPage {
    #[default]
    Local,
    Hosted {
        url: Url,
        app_brand: LoginSuccessPageBrand,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LoginSuccessPageBrand {
    Codex,
    Chatgpt,
}

impl LoginSuccessPageBrand {
    fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Chatgpt => "chatgpt",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum LoginSuccessRedirect {
    Local(String),
    Hosted(String),
}

pub(crate) fn compose_success_url(
    port: u16,
    issuer: &str,
    id_token: &str,
    access_token: &str,
    codex_streamlined_login: bool,
    login_success_page: &LoginSuccessPage,
) -> LoginSuccessRedirect {
    let token_claims = jwt_auth_claims(id_token);

    let org_id = token_claims
        .get("organization_id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let project_id = token_claims
        .get("project_id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let completed_onboarding = token_claims
        .get("completed_platform_onboarding")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let is_org_owner = token_claims
        .get("is_org_owner")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let needs_setup = !completed_onboarding && is_org_owner;
    if !needs_setup && let LoginSuccessPage::Hosted { url, app_brand } = login_success_page {
        let mut success_url = url.clone();
        success_url.set_query(None);
        success_url
            .query_pairs_mut()
            .append_pair("source", "login")
            .append_pair("app_brand", app_brand.as_str());
        return LoginSuccessRedirect::Hosted(success_url.into());
    }

    let access_claims = jwt_auth_claims(access_token);
    let plan_type = access_claims
        .get("chatgpt_plan_type")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let platform_url = if issuer == DEFAULT_ISSUER {
        "https://platform.openai.com"
    } else {
        "https://platform.api.openai.org"
    };
    let mut params = vec![
        ("id_token", id_token.to_string()),
        ("needs_setup", needs_setup.to_string()),
        ("org_id", org_id.to_string()),
        ("project_id", project_id.to_string()),
        ("plan_type", plan_type.to_string()),
        ("platform_url", platform_url.to_string()),
    ];
    if codex_streamlined_login {
        params.push(("codex_streamlined_login", "true".to_string()));
    }
    let query = params
        .into_iter()
        .map(|(key, value)| format!("{key}={}", urlencoding::encode(&value)))
        .collect::<Vec<_>>()
        .join("&");
    LoginSuccessRedirect::Local(format!("http://localhost:{port}/success?{query}"))
}

pub(crate) fn jwt_auth_claims(jwt: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut parts = jwt.split('.');
    let (_header, payload, _signature) = match (parts.next(), parts.next(), parts.next()) {
        (Some(header), Some(payload), Some(signature))
            if !header.is_empty() && !payload.is_empty() && !signature.is_empty() =>
        {
            (header, payload, signature)
        }
        _ => {
            eprintln!("Invalid JWT format while extracting claims");
            return serde_json::Map::new();
        }
    };
    match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(mut value) => {
                if let Some(claims) = value
                    .get_mut("https://api.openai.com/auth")
                    .and_then(JsonValue::as_object_mut)
                {
                    return claims.clone();
                }
                eprintln!("JWT payload missing expected 'https://api.openai.com/auth' object");
            }
            Err(error) => {
                eprintln!("Failed to parse JWT JSON payload: {error}");
            }
        },
        Err(error) => {
            eprintln!("Failed to base64url-decode JWT payload: {error}");
        }
    }
    serde_json::Map::new()
}

#[cfg(test)]
#[path = "success_page_tests.rs"]
mod tests;
