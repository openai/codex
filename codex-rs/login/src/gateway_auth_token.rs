//! Validates gateway token lifetimes and renders bounded diagnostics from shared OAuth errors.

use std::io;

use crate::oauth::OAuthError;
use crate::oauth::sanitize_url_for_logging;
use chrono::Utc;
use codex_secrets::redact_secrets;
use serde::Deserialize;
use serde::Serialize;

use super::GatewayAuthConfig;

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct StoredToken {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

#[derive(Deserialize)]
pub(super) struct TokenResponse {
    access_token: String,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

impl TokenResponse {
    pub(super) fn into_stored(
        self,
        previous_refresh_token: Option<&str>,
    ) -> io::Result<StoredToken> {
        if self.access_token.trim().is_empty() {
            return Err(io::Error::other(
                "provider OAuth token response omitted its access token",
            ));
        }
        if self
            .token_type
            .as_deref()
            .is_some_and(|value| !value.eq_ignore_ascii_case("bearer"))
        {
            return Err(io::Error::other(
                "provider OAuth token response returned an unsupported token type",
            ));
        }
        if self.expires_in == Some(0) {
            return Err(io::Error::other(
                "provider OAuth token response returned a zero lifetime",
            ));
        }
        let expires_at = self
            .expires_in
            .map(|expires_in| {
                let expires_in = i64::try_from(expires_in)
                    .map_err(|_| io::Error::other("provider OAuth token lifetime is too large"))?;
                Utc::now()
                    .timestamp()
                    .checked_add(expires_in)
                    .ok_or_else(|| io::Error::other("provider OAuth token expiry overflows"))
            })
            .transpose()?;
        Ok(StoredToken {
            access_token: self.access_token,
            refresh_token: self
                .refresh_token
                .or_else(|| previous_refresh_token.map(str::to_string)),
            expires_at,
        })
    }
}

pub(super) fn endpoint_error(
    error: OAuthError,
    config: &GatewayAuthConfig,
    grant_type: &str,
    redirect_uri: Option<&str>,
) -> io::Error {
    let endpoint = diagnostic_url(&config.token_url);
    match error {
        OAuthError::Rejected(rejection) => {
            // Issuers can echo custom URL credentials that shared grant redaction does not know.
            // Request IDs are already truncated, so even replacing complete values is unsafe.
            let has_query = std::iter::once(config.token_url.as_str())
                .chain(config.resource.as_deref())
                .any(|value| value.contains('?'));
            let (detail, request_id) = if has_query {
                (
                    "provider response details omitted".to_string(),
                    String::new(),
                )
            } else {
                // The shared layer redacts complete grant credentials before display limits.
                let detail: String = redact_secrets(rejection.detail.to_string())
                    .chars()
                    .take(/*n*/ 512)
                    .collect();
                let request_id = rejection
                    .request_id
                    .map(|value| format!(" (request id: {value})"))
                    .unwrap_or_default();
                (detail, request_id)
            };
            let client_id = &config.client_id;
            let resource = config
                .resource
                .as_deref()
                .map(diagnostic_url)
                .unwrap_or_else(|| "<not sent>".to_string());
            let redirect_uri = redirect_uri.unwrap_or("<not sent>");
            let scopes = if config.scopes.is_empty() {
                "<not sent>".to_string()
            } else {
                config.scopes.join(" ")
            };
            let pkce = if grant_type == "authorization_code" {
                "S256"
            } else {
                "not applicable"
            };
            io::Error::other(format!(
                "provider OAuth token endpoint {endpoint} returned HTTP {} - {detail}{request_id}. Request: grant_type={grant_type}, client_id={client_id}, client_auth=none (public client), pkce={pkce}, redirect_uri={redirect_uri}, resource={resource}, authorization_scopes={scopes}",
                rejection.status.as_u16(),
            ))
        }
        OAuthError::Transport(_) => io::Error::other(format!(
            "provider OAuth token exchange failed for {endpoint}"
        )),
        OAuthError::InvalidResponse => io::Error::other("provider OAuth token response is invalid"),
    }
}

fn diagnostic_url(value: &str) -> String {
    // Issuers may use custom query keys for credentials that the shared allowlist cannot know.
    let sanitized = sanitize_url_for_logging(value);
    sanitized
        .split('?')
        .next()
        .unwrap_or("<invalid-url>")
        .to_string()
}
