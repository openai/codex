//! Executes OAuth grants without owning credential state or choosing a recovery policy.

use std::collections::BTreeMap;
use std::time::Duration;

use codex_http_client::HttpClient;
use serde::de::DeserializeOwned;

use crate::oauth::ErrorBodyLimit;
use crate::oauth::OAuthError;
use crate::oauth::PkceCodes;
use crate::oauth::TokenRejection;
use crate::oauth::diagnostics::redact_error_url;

/// ChatGPT refresh uses JSON; authorization-code and gateway grants use form encoding.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TokenEncoding {
    Form,
    Json,
}

/// Per-endpoint transport settings. The HTTP client already owns routing and CA policy.
pub(crate) struct TokenEndpoint<'a> {
    pub url: &'a str,
    pub client_id: &'a str,
    pub encoding: TokenEncoding,
    pub timeout: Option<Duration>,
    pub error_body_limit: ErrorBodyLimit,
}

/// Parameters bound to an authorization attempt. Secrets deliberately have no Debug output.
pub(crate) struct AuthorizationCodeGrant<'a> {
    pub code: &'a str,
    pub redirect_uri: &'a str,
    pub pkce: &'a PkceCodes,
    pub resource: Option<&'a str>,
}

pub(crate) struct RefreshTokenGrant<'a> {
    pub refresh_token: &'a str,
    pub resource: Option<&'a str>,
}

/// Shared OAuth protocol implementation; callers retain tokens, storage, and recovery decisions.
pub(crate) struct OAuthClient<'a> {
    http_client: &'a HttpClient,
    endpoint: TokenEndpoint<'a>,
}

impl<'a> OAuthClient<'a> {
    pub(crate) fn new(http_client: &'a HttpClient, endpoint: TokenEndpoint<'a>) -> Self {
        Self {
            http_client,
            endpoint,
        }
    }

    pub(crate) async fn exchange_code<T: DeserializeOwned>(
        &self,
        grant: AuthorizationCodeGrant<'_>,
    ) -> Result<T, OAuthError> {
        let mut parameters = vec![
            ("grant_type", "authorization_code"),
            ("client_id", self.endpoint.client_id),
            ("code", grant.code),
            ("redirect_uri", grant.redirect_uri),
            ("code_verifier", grant.pkce.code_verifier.as_str()),
        ];
        if let Some(resource) = grant.resource {
            parameters.push(("resource", resource));
        }
        self.exchange(&parameters, &[grant.code, &grant.pkce.code_verifier])
            .await
    }

    pub(crate) async fn refresh<T: DeserializeOwned>(
        &self,
        grant: RefreshTokenGrant<'_>,
    ) -> Result<T, OAuthError> {
        let mut parameters = vec![
            ("grant_type", "refresh_token"),
            ("client_id", self.endpoint.client_id),
            ("refresh_token", grant.refresh_token),
        ];
        if let Some(resource) = grant.resource {
            parameters.push(("resource", resource));
        }
        self.exchange(&parameters, &[grant.refresh_token]).await
    }

    async fn exchange<T: DeserializeOwned>(
        &self,
        parameters: &[(&str, &str)],
        secrets: &[&str],
    ) -> Result<T, OAuthError> {
        let mut request = self.http_client.post(self.endpoint.url);
        if let Some(timeout) = self.endpoint.timeout {
            request = request.timeout(timeout);
        }
        request = match self.endpoint.encoding {
            TokenEncoding::Form => {
                let mut form = url::form_urlencoded::Serializer::new(String::new());
                form.extend_pairs(parameters.iter().copied());
                request
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(form.finish())
            }
            TokenEncoding::Json => request
                .header("Content-Type", "application/json")
                .json(&parameters.iter().copied().collect::<BTreeMap<_, _>>()),
        };
        let response = request
            .send()
            .await
            .map_err(|error| OAuthError::Transport(redact_error_url(error)))?;
        if !response.status().is_success() {
            return Err(OAuthError::Rejected(Box::new(
                TokenRejection::from_response(response, self.endpoint.error_body_limit, secrets)
                    .await,
            )));
        }
        response
            .json()
            .await
            .map_err(|_| OAuthError::InvalidResponse)
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
