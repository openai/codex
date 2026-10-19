//! Obtains fresh GitHub Actions OIDC subject tokens for the federated exchange.
//! The stdin request credential and minted tokens must never appear in diagnostics.

use anyhow::Result;
use anyhow::anyhow;
use http::header::AUTHORIZATION;
use http::header::HeaderValue;
use serde::Deserialize;
use url::Url;
use zeroize::Zeroizing;

use crate::auth::MAX_TOKEN_BYTES;
use crate::auth::SubjectTokenFuture;
use crate::auth::SubjectTokenSource;
use crate::auth::TOKEN_REQUEST_TIMEOUT;
use crate::auth::TokenClient;
use crate::auth::read_response;
use crate::read_api_key::AUTH_HEADER_PREFIX;
use crate::read_api_key::read_auth_header_with;
use crate::read_api_key::read_from_stdin;

// Allow space for "Bearer " and a trailing CRLF.
const REQUEST_AUTH_BUFFER_SIZE: usize = AUTH_HEADER_PREFIX.len() + MAX_TOKEN_BYTES + 2;

// Intentionally no Debug: request_auth contains a credential.
pub(crate) struct GithubOidc {
    request_url: Url,
    request_auth: HeaderValue,
}

impl GithubOidc {
    pub(crate) fn from_stdin(request_url: &str, audience: &str) -> Result<Self> {
        // Avoid echoing caller-supplied URL parameters in parsing errors.
        let mut request_url =
            Url::parse(request_url).map_err(|_| anyhow!("invalid GitHub OIDC request URL"))?;
        if request_url.scheme() != "https"
            || request_url.host_str().is_none()
            || !request_url.username().is_empty()
            || request_url.password().is_some()
            || request_url.fragment().is_some()
        {
            return Err(anyhow!(
                "GitHub OIDC request URL must be HTTPS without user info or fragment"
            ));
        }
        // Replace any existing audience; preserve GitHub's other query parameters.
        let pairs: Vec<(String, String)> = request_url
            .query_pairs()
            .filter(|(key, _)| key != "audience")
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        request_url
            .query_pairs_mut()
            .clear()
            .extend_pairs(pairs)
            .append_pair("audience", audience);
        Ok(Self {
            request_url,
            request_auth: read_request_auth(read_from_stdin)?,
        })
    }
}

impl SubjectTokenSource for GithubOidc {
    fn subject_token<'a>(&'a mut self, client: &'a TokenClient) -> SubjectTokenFuture<'a> {
        Box::pin(async move {
            let response = client
                .http()
                .get(self.request_url.clone())
                .timeout(TOKEN_REQUEST_TIMEOUT)
                .header(AUTHORIZATION, self.request_auth.clone())
                .send()
                .await
                .map_err(|_| anyhow!("GitHub OIDC token request failed"))?;
            let response: GithubResponse = read_response(response, "GitHub OIDC").await?;
            Ok(response.value)
        })
    }
}

#[derive(Deserialize)]
struct GithubResponse {
    value: Zeroizing<String>,
}

// The launcher must remove ACTIONS_ID_TOKEN_REQUEST_TOKEN from its environment
// before exec. This header is retained for the lifetime of the proxy.
fn read_request_auth<F>(mut read: F) -> Result<HeaderValue>
where
    F: FnMut(&mut [u8]) -> std::io::Result<usize>,
{
    let read_credential = |buf: &mut [u8]| {
        read(buf).map_err(|_| {
            std::io::Error::other("could not read GitHub OIDC request credential from stdin")
        })
    };
    let value = read_auth_header_with::<REQUEST_AUTH_BUFFER_SIZE>(
        read_credential,
        validate_github_request_token,
        || anyhow!("GitHub OIDC request credential too large"),
    )?;
    let mut header = HeaderValue::from_static(value);
    header.set_sensitive(true);
    Ok(header)
}

fn validate_github_request_token(token: &[u8]) -> Result<usize> {
    let token = token.strip_suffix(b"\n").unwrap_or(token);
    let token = token.strip_suffix(b"\r").unwrap_or(token);
    // RFC 6750 permits JWT periods; reject whitespace and control bytes.
    if token.is_empty()
        || token.len() > MAX_TOKEN_BYTES
        || !token.iter().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=')
        })
    {
        return Err(anyhow!("invalid workload identity bearer credential"));
    }
    Ok(token.len())
}

#[cfg(test)]
#[path = "github_oidc_tests.rs"]
mod tests;
